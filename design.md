# Design

This project is structured into 3 layers:

1.  Core components for working with GTFS and trajectories
2.  A daily model combining raw bus operational data for one day
3.  An aggregate model summarizing bus operational data over many months

## Layer 1: core components

Status: Pretty solid. I'd like to clean these libraries up and publish to [crates.io](https://crates.io).

There are a few foundational structures needed for this project, and very likely useful elsewhere.

- A **GTFS model**, turning the raw CSV files into something structured and easily queried. The GTFS spec has a confusing definition of bus route, so this model splits it into "route variants," which visit a sequence of stops in a certain order. Each variant has a list of scheduled trips, which assign an expected arrival time to each stop.
  - The model can be filtered by a range of dates, such as weekends or a specific day. The route variants available change based on this.
  - There's a simple UI to browse stops, route variants, and to filter by dates. This should be widely usable just to understand a GTFS network.
- A **trajectory** data structure to describe movement through 2D space over time.
  - It supports operations like interpolating over time, clipping to a time range, and finding all times near some position.
  - There's a UI to animate objects along a trajectory, or show times when the trajectory passes nearby the mouse cursor. This is generally useful to understand trajectory data of any sort.
  - [MovingPandas](https://github.com/anitagraser/movingpandas) served as useful reference here

## Layer 2: daily model

Status: Partly working, but still not matching everything up correctly.

We have real data from Brazilian bus operators, but the raw data is ambiguous and hard to work with. This layer joins bus trajectory data, a GTFS network, and ticketing data when people board a bus. It produces a huge list of boarding events, representing every time a bus arrived at a stop and possibly picked up passengers. Each event has:

- the vehicle ID
- the GTFS trip ID being served
- the arrival and departure time at the stop
- the stop ID
- a list of new riders and a list of riders transferring (based on a configurable 2 hour rule)
  - this could be simplified as a number of riders, if expressing boarding events in CSV

These boarding events form a simple 2D table, which could be filtered and aggregated using traditional data science techniques. After we produce these boarding events, we can throw away the raw trajectory and ticketing data for many types of analysis and just work off this simpler summary. The process for producing these events works on a single day's data.

There's a somewhat elaborate UI for exploring all of this data. You can follow individual vehicles, view their inferred schedule, inspect each trip they took and the arrival times at stops. You can run the simulation forwards or in reverse, visualize when people board a bus, compare actual arrival times to the schedule, etc. All of this UI was developed for the purpose of debugging the raw data and developing the techniques used to clean and join up the data. The UI might also be useful to bus operators manually diagnosing what happened on an unusual day. But otherwise it probably isn't great for further analysis; it's too detailed to look at one day at a time.

## Layer 3: aggregate model

Status: Barely started.

We can run layer 2 for data over many months, then ask higher-level questions on top of the compressed boarding events. For example, how many people transfer at a stop? How delayed are buses along a route variant? These questions can be answered in layer 2 for a single moment in time, but the purpose of layer 3 is to filter (peak morning rush hours on weekdays in summer) and aggregate (total per route or stop).

I strongly suspect something like Tableau, Excel, dashboards on top of SQL, etc could answer these sorts of questions easily. So I will just prototype a simple UI for answering a few questions, for now.
