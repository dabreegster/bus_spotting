use abstutil::Timer;
use anyhow::Result;
use geom::{Distance, LonLat};
use geom::{FindClosest, GPSBounds};
use street_network::{DrivingSide, LaneType, StreetNetwork};

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

    // Find the shortest route between the intersections closest to the start/end of each route
    // shape.
    let mut closest = FindClosest::new(&gps_bounds.to_bounds());
    for (id, i) in &streets.intersections {
        closest.add(*id, &[i.point]);
    }

    // TODO This never matches right now.
    for (id, path) in timer
        .parallelize(
            "snap route shapes",
            gtfs.shapes.iter().map(|(id, pl)| (id, pl)).collect(),
            |(id, pl)| {
                let threshold = Distance::meters(100.0);
                let mut result = None;
                if let Some((from, _)) = closest.closest_pt(pl.first_pt(), threshold) {
                    if let Some((to, _)) = closest.closest_pt(pl.last_pt(), threshold) {
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
        info!("{:?} has a path of len {}", id, path.len());
    }

    // For debugging, convert to the drawable form of StreetNetwork and stash that.
    let initial = street_network::initial::InitialMap::new(&streets, timer);
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
