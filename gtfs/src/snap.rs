use abstutil::Timer;
use anyhow::Result;
use geom::{Distance, LonLat};
use geom::{FindClosest, GPSBounds, PolyLine};
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
        if let Ok(pl) = make_snapped_shape(&initial, path) {
            gtfs.snapped_shapes.insert(id.clone(), pl);
        }
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
    path: Vec<(OriginalRoad, Direction)>,
) -> Result<PolyLine> {
    let mut pts = Vec::new();
    for (r, dir) in path {
        let mut append = initial.roads[&r].trimmed_center_pts.clone().into_points();
        if dir == Direction::Back {
            append.reverse();
        }
        pts.extend(append);
    }
    PolyLine::new(pts)
}
