# Shared daily budget of runs or minutes

Date: 2026-11-17

## Context

Noita has no run counter or API. The goal is to stop at a fixed number of runs per day, but a launch can hold many deaths and a run can last an hour, so launches and a run cap alone do not bound play time. Hopping to another roguelike would dodge a per-game limit.

## Decision

- One budget per day across all roguelikes: N runs OR M minutes, whichever is hit first. Defaults 5 runs and 90 minutes, both configurable.
- A run counts when its end (death or win) is detected. A run shorter than 5 minutes (configurable) is a bad-seed restart and is not counted. Its minutes still count toward the time cap, which is the backstop against restarting at 4:59 repeatedly.
- The day resets at 4am local time. A run at 1am Saturday counts against Friday.
- At the limit the game is killed after a 10 second warning, and relaunches are killed until the reset. The run in progress does not finish.
- Time is accumulated from a monotonic clock, so moving the system date forward does not refund minutes. The day boundary itself uses wall-clock time.

## Consequences

- 3 Noita runs plus 2 Hades runs exhaust the run cap.
- Raising a limit takes effect at the next reset, never mid-day.
- Changing the system date to cross 4am resets the day. This is an accepted hole.
