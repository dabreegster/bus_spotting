use std::collections::BTreeMap;

use abstutil::Timer;
use anyhow::Result;
use geom::{Distance, FindClosest, GPSBounds, LonLat, PolyLine, Polygon, Pt2D};
use street_network::initial::InitialMap;
use street_network::{Direction, DrivingSide, LaneType, OriginalRoad, StreetNetwork};

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
    // TODO Specify or calculate DrivingSide
    let streets = import_streets(
        &osm_xml_input,
        DrivingSide::Right,
        gps_bounds.get_rectangle(),
        timer,
    )?;
    let initial = InitialMap::new(&streets, timer);

    // For the left side of the road, should we start from i1 or i2?
    let left_side_i1 = if streets.config.driving_side == DrivingSide::Right {
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
    let mut closest: FindClosest<(OriginalRoad, bool)> = FindClosest::new(&gps_bounds.to_bounds());
    for (id, r) in &initial.roads {
        if let Ok(pl) = r.trimmed_center_pts.shift_left(r.half_width) {
            closest.add((*id, left_side_i1), pl.points());
        }
        if let Ok(pl) = r.trimmed_center_pts.shift_right(r.half_width) {
            closest.add((*id, !left_side_i1), pl.points());
        }
    }

    let mut all_paths = Vec::new();
    let mut all_path_ids = Vec::new();
    for (id, path) in timer
        .parallelize(
            "snap route shapes",
            gtfs.shapes.iter().map(|(id, pl)| (id, pl)).collect(),
            |(id, pl)| {
                let threshold = Distance::meters(50.0);
                let mut result = None;
                if let Some(((from_r, from_i1), _)) = closest.closest_pt(pl.first_pt(), threshold) {
                    if let Some(((to_r, to_i1), _)) = closest.closest_pt(pl.last_pt(), threshold) {
                        // Pathfind from the intersections
                        let from = if from_i1 { from_r.i1 } else { from_r.i2 };
                        let to = if to_i1 { to_r.i1 } else { to_r.i2 };

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
        if let Ok(pl) = make_snapped_shape(&initial, &path) {
            gtfs.snapped_shapes.insert(id.clone(), pl);
        }
        all_paths.push(path.into_iter().map(|(r, _)| r).collect());
        all_path_ids.push(id.clone());
    }

    for (polygons, shape_id) in render_overlapping_paths(&initial, all_paths)
        .into_iter()
        .zip(all_path_ids.into_iter())
    {
        gtfs.nonoverlapping_shapes.insert(shape_id, polygons);
    }

    // For debugging, convert to the drawable form of StreetNetwork and stash that.
    for r in initial.roads.values() {
        gtfs.road_geometry
            .push(r.trimmed_center_pts.make_polygons(2.0 * r.half_width));
    }
    for i in initial.intersections.into_values() {
        gtfs.intersection_geometry.push(i.polygon);
    }

    timer.stop("snap routes to street network");
    Ok(())
}

fn import_streets(
    osm_xml_input: &str,
    driving_side: DrivingSide,
    clip_pts: Vec<LonLat>,
    timer: &mut Timer,
) -> Result<StreetNetwork> {
    let mut street_network = import_streets::osm_to_street_network(
        osm_xml_input,
        Some(clip_pts),
        import_streets::Options::default_for_side(driving_side),
        timer,
    )?;
    let consolidate_all_intersections = false;
    let remove_disconnected = true;
    street_network.run_all_simplifications(
        consolidate_all_intersections,
        remove_disconnected,
        timer,
    );
    Ok(street_network)
}

fn make_snapped_shape(
    initial: &InitialMap,
    path: &Vec<(OriginalRoad, Direction)>,
) -> Result<PolyLine> {
    let mut pts = Vec::new();
    for (r, dir) in path {
        let mut append = initial.roads[r].trimmed_center_pts.clone().into_points();
        if *dir == Direction::Back {
            append.reverse();
        }
        pts.extend(append);
    }
    PolyLine::new(pts)
}

// Per input path, return a list of polygons that should be logically unioned together to form one
// shape.
//
// Lots of logic shared with map_gui's draw_overlapping_paths.
fn render_overlapping_paths(
    initial: &InitialMap,
    paths: Vec<Vec<OriginalRoad>>,
) -> Vec<Vec<Polygon>> {
    let road_width_multiplier = 3.0;

    let mut output = std::iter::repeat_with(Vec::new)
        .take(paths.len())
        .collect::<Vec<Vec<Polygon>>>();

    // Per road, just figure out what path indices we need
    let mut objects_per_road: BTreeMap<OriginalRoad, Vec<usize>> = BTreeMap::new();
    let mut objects_per_movement: Vec<(OriginalRoad, OriginalRoad, usize)> = Vec::new();
    for (idx, path) in paths.into_iter().enumerate() {
        for step in &path {
            objects_per_road
                .entry(step.clone())
                .or_insert_with(Vec::new)
                .push(idx);
        }
        for pair in path.windows(2) {
            objects_per_movement.push((pair[0].clone(), pair[1].clone(), idx));
        }
    }

    // Per road and object, mark the 4 corners of the thickened polyline.
    // (beginning left, beginning right, end left, end right)
    let mut pieces: BTreeMap<(OriginalRoad, usize), (Pt2D, Pt2D, Pt2D, Pt2D)> = BTreeMap::new();
    // Per road, divide the needed objects proportionally
    for (road_id, objects) in objects_per_road {
        let road = &initial.roads[&road_id];
        let total_width = road_width_multiplier * 2.0 * road.half_width;
        let width_per_piece = total_width / (objects.len() as f64);
        for (piece_idx, path_idx) in objects.into_iter().enumerate() {
            let width_from_left_side = (0.5 + (piece_idx as f64)) * width_per_piece;
            // This logic is shift_from_left_side
            if let Ok(pl) = road
                .trimmed_center_pts
                .shift_from_center(total_width, width_from_left_side)
            {
                let polygon = pl.make_polygons(width_per_piece);
                output[path_idx].push(polygon);

                // Reproduce what make_polygons does to get the 4 corners
                if let Some(corners) = pl.get_four_corners_of_thickened(width_per_piece) {
                    pieces.insert((road_id, path_idx), corners);
                }
            }
        }
    }

    // Fill in intersections
    for (from, to, path_idx) in objects_per_movement {
        if let Some(from_corners) = pieces.get(&(from, path_idx)) {
            if let Some(to_corners) = pieces.get(&(to, path_idx)) {
                /*let from_road = app.map().get_r(from);
                let to_road = app.map().get_r(to);
                if let CommonEndpoint::One(i) = from_road.common_endpoint(to_road) {
                    let (from_left, from_right) = if from_road.src_i == i {
                        (from_corners.0, from_corners.1)
                    } else {
                        (from_corners.2, from_corners.3)
                    };
                    let (to_left, to_right) = if to_road.src_i == i {
                        (to_corners.0, to_corners.1)
                    } else {
                        (to_corners.2, to_corners.3)
                    };
                    // Glue the 4 corners together
                    if let Ok(ring) =
                        Ring::new(vec![from_left, from_right, to_right, to_left, from_left])
                    {
                        output[path_idx].push(ring.into_polygon());
                    }
                }*/
            }
        }
    }
    output
}
