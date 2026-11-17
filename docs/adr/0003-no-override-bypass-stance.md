# No in-app override

Date: 2026-11-17

## Context

Roguestill exists because stopping on its own fails. Friction screens (a wait timer, a typed reason) are easy to click through when tired or hooked, so they do not hold at the moment they matter.

## Decision

- No override in any UI. The watcher has no unlock path.
- A LaunchAgent with KeepAlive restarts the watcher if it is quit or killed.
- Limit changes are written to config and apply at the next 4am reset. Lowering takes effect then as well, so the rule is symmetric.
- An accountability partner with a PIN was considered and rejected for v1: it needs a second person and sync.

## Consequences

- Disabling the LaunchAgent in Terminal, deleting the app, or changing the system date still bypasses it. Those take deliberate steps, which is the intended friction.
- A mistaken limit stays in force until the next reset.
