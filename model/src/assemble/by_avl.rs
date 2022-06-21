use abstutil::Timer;

use crate::Model;

// TODO This is dead code, just stashing notes here

impl Model {
    pub fn assemble_using_stops(&self, timer: &mut Timer) -> Result<()> {
        for (vehicle_id, variants) in model.vehicle_to_possible_routes()? {
            // Based on all possible variants that might be served, let's union all of the stops


            // Each variant says "visit these stops in this order" (and at some time)
            // TODO really do make a way to draw the shape of the X variants that match, and the
            // actual AVL path.


            // TODO ahhh even better idea: create a trajectory for each trip!! then we can visually
            // do the live replay, draw the pink line over time. then maybe we score by just
            // stepping every few seconds, or just at stops or something.
            //
            // (problem is there are lots of trips and are short-lived. so if we're very delayed,
            // the match will not be good)
            //
            //
            // maybe it's all about comparing trajectories.


            let variant = model.gtfs.variant(variants.pop().unwrap());
        }
    }
}
