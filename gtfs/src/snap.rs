use std::collections::BTreeMap;

use abstutil::Timer;
use anyhow::Result;
use geom::{Distance, FindClosest, GPSBounds, Line, LonLat, PolyLine, Polygon, Ring};
use osm2streets::{Direction, DrivingSide, LaneType, RoadID, StreetNetwork, Transformation};

use crate::GTFS;

// To manually get input.osm: ./target/x86_64-unknown-linux-gnu/release/cli one-step-import --geojson-path foo.geojson --map-name sjc

pub fn snap_routes<R: std::io::Read>(
    gtfs: &mut GTFS,
    mut reader: R,
    gps_bounds: &GPSBounds,
    timer: &mut Timer,
) -> Result<()> {
    timer.start("snap routes to street network");

    let mut osm_xml_input = String::new();
    reader.read_to_string(&mut osm_xml_input)?;
    let streets = import_streets(&osm_xml_input, gps_bounds.get_rectangle(), timer)?;

    // For the left side of the road, should we start from src_i or dst_i?
    let left_side_src_i = if streets.config.driving_side == DrivingSide::Right {
        false
    } else {
        true
    };

    // Snap the start/end of each route shape to the nearest side of a road. Matching to
    // intersections doesn't work well; many routes have endpoints at a stop in the middle of a
    // long road.
    //
    // TODO We should really do this for every pair of stops and glue those together. Variant 196
    // skips a bunch of stops!
    let mut closest: FindClosest<(RoadID, bool)> = FindClosest::new(&gps_bounds.to_bounds());
    for (id, r) in &streets.roads {
        if let Ok(pl) = r.center_line.shift_left(r.half_width()) {
            closest.add((*id, left_side_src_i), pl.points());
        }
        if let Ok(pl) = r.center_line.shift_right(r.half_width()) {
            closest.add((*id, !left_side_src_i), pl.points());
        }
    }

    let mut all_paths = Vec::new();
    for (id, path) in timer
        .parallelize(
            "snap route shapes",
            gtfs.shapes.iter().map(|(id, pl)| (id, pl)).collect(),
            |(id, pl)| {
                let threshold = Distance::meters(50.0);
                let mut result = None;
                if let Some(((from_r, from_src_i), _)) =
                    closest.closest_pt(pl.first_pt(), threshold)
                {
                    if let Some(((to_r, to_src_i), _)) = closest.closest_pt(pl.last_pt(), threshold)
                    {
                        // Pathfind from the intersections
                        // TODO Consider using RoadWithEndpoints
                        let from = if from_src_i {
                            streets.roads[&from_r].src_i
                        } else {
                            streets.roads[&from_r].dst_i
                        };
                        let to = if to_src_i {
                            streets.roads[&to_r].src_i
                        } else {
                            streets.roads[&to_r].dst_i
                        };

                        if let Some(path) =
                            streets.simple_path(from, to, &[LaneType::Driving, LaneType::Bus])
                        {
                            result = Some((id, path));
                        }
                    }
                }
                result
            },
        )
        .into_iter()
        .flatten()
    {
        if let Ok(pl) = make_snapped_shape(&streets, &path) {
            gtfs.snapped_shapes.insert(id.clone(), pl);
        }
        all_paths.push((id.clone(), path));
    }

    timer.start("render overlapping paths");
    for (shape_id, polygon) in render_overlapping_paths(&streets, all_paths, timer) {
        gtfs.nonoverlapping_shapes.insert(shape_id, polygon);
    }
    timer.stop("render overlapping paths");

    // For debugging, convert to the drawable form of StreetNetwork and stash that.
    for r in streets.roads.into_values() {
        gtfs.road_geometry
            .push(r.center_line.make_polygons(2.0 * r.half_width()));
    }
    for i in streets.intersections.into_values() {
        gtfs.intersection_geometry.push(i.polygon);
    }

    timer.stop("snap routes to street network");
    Ok(())
}

fn import_streets(
    osm_xml_input: &str,
    clip_pts: Vec<LonLat>,
    timer: &mut Timer,
) -> Result<StreetNetwork> {
    let (mut street_network, _) = streets_reader::osm_to_street_network(
        osm_xml_input,
        Some(clip_pts),
        osm2streets::MapConfig::default(),
        timer,
    )?;
    // We don't care about most transformations, especially since some of them are slow to run.
    street_network.apply_transformations(vec![Transformation::RemoveDisconnectedRoads], timer);
    Ok(street_network)
}

fn make_snapped_shape(
    streets: &StreetNetwork,
    path: &Vec<(RoadID, Direction)>,
) -> Result<PolyLine> {
    let mut pts = Vec::new();
    for (r, dir) in path {
        let mut append = streets.roads[r].center_line.clone().into_points();
        if *dir == Direction::Back {
            append.reverse();
        }
        pts.extend(append);
    }
    PolyLine::new(pts)
}

// Per input path, return a polygon covering it.
//
// Lots of logic shared with map_gui's draw_overlapping_paths, but also kind of experimenting with
// gluing one polygon together.
fn render_overlapping_paths<ID: Clone + PartialEq + Send + Sync>(
    streets: &StreetNetwork,
    paths: Vec<(ID, Vec<(RoadID, Direction)>)>,
    timer: &mut Timer,
) -> Vec<(ID, Polygon)> {
    let road_width_multiplier = 1.0;

    // Per road, just figure out what objects we need
    let mut objects_per_road: BTreeMap<RoadID, Vec<ID>> = BTreeMap::new();
    for (id, path) in &paths {
        for (road, _) in path {
            objects_per_road
                .entry(road.clone())
                .or_insert_with(Vec::new)
                .push(id.clone());
        }
    }

    let roads = &streets.roads;
    let get_sides = |road_id: &RoadID, id: &ID| {
        let road = &roads[road_id];
        let total_width = road_width_multiplier * 2.0 * road.half_width();
        let objects = &objects_per_road[road_id];
        let width_per_piece = total_width / (objects.len() as f64);
        let piece_idx = objects.iter().position(|x| x == id).unwrap();

        let width_from_left_side = (piece_idx as f64) * width_per_piece;
        // This logic is shift_from_left_side
        let left_pl = road
            .center_line
            .shift_from_center(total_width, width_from_left_side)
            .unwrap();
        let right_pl = road
            .center_line
            .shift_from_center(total_width, width_from_left_side + width_per_piece)
            .unwrap();
        (left_pl.into_points(), right_pl.into_points())
    };

    timer
        .parallelize("render path", paths, |(id, path)| {
            let mut left_side_pts = Vec::new();
            let mut right_side_pts = Vec::new();

            for (road, dir) in path {
                let (mut left, mut right) = get_sides(&road, &id);
                if dir == Direction::Back {
                    left.reverse();
                    right.reverse();
                }

                // The relative position along the pair of roads may change dramatically, causing the
                // left and right side to effectively swap. Just test if line segments overlap...
                if !left_side_pts.is_empty() {
                    if let Ok(l1) = Line::new(*left_side_pts.last().unwrap(), left[0]) {
                        if let Ok(l2) = Line::new(*right_side_pts.last().unwrap(), right[0]) {
                            if l1.intersection(&l2).is_some() {
                                std::mem::swap(&mut left, &mut right);
                            }
                        }
                    }
                }

                left_side_pts.extend(left);
                right_side_pts.extend(right);
            }

            // Glue both sides together
            right_side_pts.reverse();
            left_side_pts.extend(right_side_pts);
            left_side_pts.push(left_side_pts[0]);
            left_side_pts.dedup();
            let mut result = None;
            if let Ok(ring) = Ring::new(left_side_pts) {
                if check_ring(&ring) {
                    result = Some((id, ring.into_polygon()));
                }
            }

            // Debug by looking at the left and right side individually
            /*if let Ok(poly1) = PolyLine::new(left_side_pts).map(|pl| pl.make_polygons(Distance::meters(0.1))) {
                if let Ok(poly2) = PolyLine::new(right_side_pts).map(|pl| pl.make_polygons(Distance::meters(0.1))) {
                    output.push((id, poly1.union(poly2)));
                }
            }*/

            result
        })
        .into_iter()
        .flatten()
        .collect()
}

fn check_ring(ring: &Ring) -> bool {
    // We still wind up with bowties. Just sanity check and see if any two line segments intersect.
    //
    // This fixes most, but not all, cases in SJC.
    let mut lines = Vec::new();
    for pair in ring.points().windows(2) {
        lines.push(Line::must_new(pair[0], pair[1]));
    }

    for l1 in &lines {
        for l2 in &lines {
            if l1.crosses(l2) {
                return false;
            }
        }
    }
    true
}
