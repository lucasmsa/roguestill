# Rust core, orthogonal rule model, Swift clients

Date: 2026-11-17

Supersedes in part: 0001 (reset time, junk threshold), 0002 (language, junk threshold left open). 0003 stands.

## Context

The first design hardcoded one budget shape and Swift for the watcher. The tool should behave like ScreenZen: several limit types, several actions, any app, and later a Windows gaming PC. That needs the rules to be data and the OS specifics to be swappable.

## Decision

- Core: a Rust daemon run by a LaunchAgent with KeepAlive. It owns counting, budgets and killing. Clients talk to it over a Unix socket with JSON messages. The Swift menu bar app and widget are thin clients that read status and edit config. Enforcement continues when no client is open.
- OS specifics sit behind three traits: process watcher, notifier, blocker. A Windows port is a new implementation of the three, not a rewrite.
- A rule is four independent parts:
  - Target: process name (including Wine `.exe`, e.g. `noita.exe`) or native bundle ID. Websites are out of scope.
  - Signal adapter (optional): a run-event detector. Noita's is defined in 0002. Targets without an adapter have no run events.
  - Budget: daily time, launch count, run count (needs an adapter), or schedule window.
  - Action: notify at a threshold, kill, mindful pause on launch.
- Time budgets use wall time while the target process runs, accumulated from a monotonic clock (0001). The clock pauses when the adapter's save folder has no writes for 5 minutes, so a game left open overnight costs 5 minutes, not hours.
- Noita junk runs: a run counts only with in-game `playtime` of at least 30 seconds. Shorter runs still add their minutes to the time budget (0001).
- Day reset is local midnight, replacing 4am from 0001. Configurable per rule.
- At the limit: 10 second warning, then the process is killed, including mid-run (0001). After the limit, any relaunch that day is killed again; this is a per-rule field with that default.
- Mindful pause is launch friction only: a screen shown before the app is allowed to continue. It never adds budget. Grace extensions are rejected, per 0003.
- The block is an animated overlay over the blocked app only. The core emits `blocked(target, reason)`; the client draws the overlay. Until the look is chosen with /visual-options, the placeholder is a plain grey shield.
- Storage: rules in a TOML file (editable, diffable), usage in SQLite with one row per session and per run (target, start, end, active seconds, run id). Budgets are recomputed from rows, not read from a counter.
- Tamper stance is 0003: KeepAlive restarts the daemon, rule edits are logged and apply at the next reset.

## Build order

1. Daemon, rule engine, process watcher, daily time budget, Noita adapter, kill, CLI.
2. Swift widget and block animation.
3. Launch count, run count, schedule windows, mindful pause.
4. Bundle ID targets.
5. Windows implementation of the traits.

## Consequences

- Adding a game is one adapter plus one rule; adding a limit type is one budget variant.
- Two languages: the Swift clients depend on the JSON socket contract, which must be versioned.
- Midnight reset lets a 23:50 session and a 00:20 session each use a full budget. Accepted.
- Edits mid-day do not apply until the reset, so a mistaken limit stays until then (0003).
