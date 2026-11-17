# Settings window with live edits

## Status

Accepted 2026-11-17. Supersedes the "edits wait for the reset" parts of 0003 and 0004.

## Decision

- The menu bar app has a Settings window: per game the daily limit, warning seconds, idle pause, reset hour and "close the game" switch; add and remove games; open the app at login.
- Edits apply immediately. The daemon compares each rule with `rules.toml` on every poll and swaps in a changed rule, keeping the minutes already used today. New games start being watched at once. This matches ScreenZen, which also lets the limit be changed at any time.
- The window never writes TOML. It calls the bundled `roguestill` CLI: `config` prints the rules as JSON, `config-set` takes JSON on stdin, validates it with the same parser the daemon uses and replaces `rules.toml` atomically. A rejected change leaves the file untouched.
- Runs per day is an optional second limit (`limit_runs`, 0 is invalid, absent means no limit). When today's counted runs reach it, the day is over exactly as if the minutes ran out: warning, then close, and a relaunch is closed again. It is checked every poll from the same run count shown as "Runs today: 2 of 5".
- Every config text change is still logged in `rule_edits`, so raising a limit mid-session leaves a record.

## Consequences

- Raising the limit while blocked works. The no-override stance in 0003 now rests on friction elsewhere (KeepAlive, the edit log), not on delayed edits.
- Removing a game from the file does not stop its running rule until the next reset.
