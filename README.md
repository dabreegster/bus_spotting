# Bus spotting

This is an experiment to combine data from a bus operator and answer some
sample questions with a simple UI. The input data includes GTFS, discrepencies
from the schedule, bus GPS trackers, and boarding events. One way of joining
this data is to come up with a higher-level "schema" describing things like:

- the trajectory of a vehicle, segmented into semantic stages like traveling
  between stops and waiting at a stop or depot
- multi-leg journeys a person takes, making heuristic guesses about where they
  alight, waiting times, how they transfer, etc
- a boarding event pairing passengers waiting at a stop with a particular
  vehicle

Viewing vehicles and passengers as having a state machine, a day can be
"simulated" (played back from recorded events). This is useful for
visualization.

## Status

See the [user guide](user_guide.md). This project is under development during
June and July 2022. When the dust settles, I'll write up documentation for
everything.

## Design

See [here](design.md) for more recent notes.

- Generally assuming the unit of analysis is one day at a time
- Agnostic to the transit agency; as soon as we have ticketing or GPS data from
  another group, we can transform it into the same common formats
- Assuming the data stays small, no need for a database. The data never needs
  to hit a network; the user can transform their raw input once, then run the
  UI against the simplified schema.
- There's a simple UI using
  [widgetry](https://github.com/a-b-street/abstreet/tree/master/widgetry/). It
  can run natively or in a web browser.
- An API to read the schema and work with it could be wired up to another
  language, if needed.

## Use cases

Basic data exploration:

1.  Playback bus movement over a day
2.  Explore stops, routes, timetables, etc. (To my knowledge, there's no free
    GTFS viewer that lets you answer something like "for a route ID, see all
    the variations in stop sequence and the timetable for this")

And particular questions:

1.  Which bus routes are used most frequently as the last leg of a journey?
2.  For journeys ending in a user-drawn region, where do the trips start and
    what route does the first leg use?
3.  Per bus stop and route, draw a waiting time graph for passengers there
4.  Where are buses most frequently delayed in traffic or at intersections?
