# Watcher daemon, game adapters, read-only UI

Date: 2026-11-17

## Context

Counting and killing must keep working when no UI is open, and must not depend on a window being alive. Run detection differs per game because each game stores state differently. Noita runs on macOS through a wrapper (CrossOver or Whisky), so its save folder sits inside the wrapper's prefix.

## Decision

- Swift, three parts: a LaunchAgent watcher, a menu bar app, and a WidgetKit widget.
- The watcher owns counting, the budget, and killing. It writes a state file. The menu bar app and the widget only read it.
- A game is an adapter: a process matcher plus a run-event detector (start, end, outcome). Only the Noita adapter is built and tested in v1. Adding a game is one adapter and one config entry.
- Noita's detector watches the save folder inside the wrapper prefix. The exact path and which change marks a run end are unverified and are the first thing to check on the machine.
- If an adapter cannot find its save folder, the watcher falls back to the time cap only and the widget says so. It never silently counts zero runs.
- Processes with no adapter are ignored.

## Consequences

- Closing the menu bar app or widget does not stop enforcement.
- Widgets refresh on the OS schedule, so the count shown can lag the watcher.
- The phone and sync are out of scope.
