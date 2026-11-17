# Watcher daemon, game adapters, read-only UI

Date: 2026-11-17

## Context

Counting and killing must keep working when no UI is open, and must not depend on a window being alive. Run detection differs per game because each game stores state differently. Noita runs on macOS through a wrapper (CrossOver or Whisky), so its save folder sits inside the wrapper's prefix.

## Decision

- Swift, three parts: a LaunchAgent watcher, a menu bar app, and a WidgetKit widget.
- The watcher owns counting, the budget, and killing. It writes a state file. The menu bar app and the widget only read it.
- A game is an adapter: a process matcher plus a run-event detector (start, end, outcome). Only the Noita adapter is built and tested in v1. Adding a game is one adapter and one config entry.
- Noita's detector watches `save00` inside the CrossOver bottle "Steam": `~/Library/Application Support/CrossOver/Bottles/Steam/drive_c/users/crossover/AppData/LocalLow/Nolla_Games_Noita/save00`. Process to match: `noita.exe` (Wine process, full Windows path in argv).
- Run end (verified 2026-11-17 on a live death): a new `stats/sessions/<run-start>_stats.xml` with `dead="1"` is the end signal. The game writes it only when the player leaves the death screen, not at the moment of death. `world/` is emptied around the same time, but a 2s poll can see it already refilling (153 files down to 3 mid-load), so an empty `world/` is not required. The file name is the run's start time, `playtime` inside is in-game seconds (a run left idle overnight read 755s), `dead="1"`. Quitting to the desktop does not empty `world/`, so it does not end a run.
- Run start: `world/` refills with files (0 to 12 in 5 seconds) right after an end. The stats file for a run only exists after it ends.
- Junk sessions: runs ended within about 10 seconds (new game pressed immediately) also write a stats file. Count a run only when `playtime` is above a threshold (to decide). `*_kills.xml` is written late and sometimes in batches, so it is not an end signal.
- If an adapter cannot find its save folder, the watcher falls back to the time cap only and the widget says so. It never silently counts zero runs.
- Processes with no adapter are ignored.

## Consequences

- Closing the menu bar app or widget does not stop enforcement.
- Widgets refresh on the OS schedule, so the count shown can lag the watcher.
- The phone and sync are out of scope.
