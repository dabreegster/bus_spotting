# Matching raw bus data

We have real data from Brazilian bus operators, but the raw data is ambiguous and hard to work with. This layer joins bus trajectory data, a GTFS network, and ticketing data when people board a bus. It produces a huge list of boarding events, representing every time a bus arrived at a stop and possibly picked up passengers. Each event has:

- the vehicle ID
- the GTFS trip ID being served
- the arrival and departure time at the stop
- the stop ID
- a list of new riders and a list of riders transferring (based on a configurable 2 hour rule)
  - this could be simplified as a number of riders, if expressing boarding events in CSV

These boarding events form a simple 2D table, which could be filtered and aggregated using traditional data science techniques. After we produce these boarding events, we can throw away the raw trajectory and ticketing data for many types of analysis and just work off this simpler summary. The process for producing these events works on a single day's data.

## Raw data

### Trajectories

### GTFS

### Ticketing

## The matching algorithm

The problem re-stated: we have trajectory (lat, long, time tuples) for a bunch of buses, covering a full day. We have a (lat, long) description of bus routes -- both stop positions, and a more detailed shape in between. We also have an ideal timetable that breaks down each route into trips. One route might have 5 trips a day, and we know the time the bus is supposed to arrive at each stop in sequence. The goal is to match the two up, chopping up the real bus trajectory into "actual" trips, where we say the bus visits some sequence of stops in a route, recording the actual time it's there. Each vehicle tends to serve a sequence of routes through the day and also spend some time doing something unexplained (doing doughnuts in a parking lot, driving from the end of one route to the start of another, etc). It's common for one vehicle to serve two routes in opposite directions -- so whatever we do has to pay attention to stop order

The thing that seems to work the best so far.subroutine: given a vehicle and a possible route, return all actual trips it takes.  (actual trip = "stop 1 at t1, stop 2 at t2, ..."

    Find all times the trajectory is within 10 meters of each stop
    Try to stitch those together into sequences of stops in order. Start with the earliest time at stop 1. Find the next earliest time at stop 2. Then the next earliest at stop 3.

Then build up a bit more structure. subroutine: given a vehicle, return all trips it takes over the whole day, for any possible route.

    Use some other data to figure out routes that possibly match (usually about 4 or 5 per vehicle)
    Call the above
    A vehicle can't be doing two trips at the same time. So walk through the total list of trips and pick some that don't overlap. I've been sorting by the full trip duration and preferring to slot in shorter trips first, because the first subroutine is buggy

### Problems

Problems: I've seen buses that appear to serve most of a route, but then just give up on the last few stops and detour or start another route. Hours later, they restart the same route and actually hit all the stops. The first subroutine will insert a multi-hour delay between two of the stops

## Alternatives considered

Link to issue
