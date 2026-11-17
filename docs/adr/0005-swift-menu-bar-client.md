# Swift menu bar client, overlay and widget

## Status

Accepted 2026-11-17. Refines build step 2 of 0004.

## Decision

- Order: menu bar app with block overlay first, WidgetKit widget after.
- The app is the only socket client. It polls `limiterd.sock` every second, writes the status JSON to an App Group container and reloads the widget timeline. The widget never touches the daemon. Whether a widget extension could read the socket was not verified; the App Group path avoids the question.
- Block event: each rule in `Status` carries `blocked_seq`, a counter the daemon increments on every SIGTERM it sends. `v` stays 1 (the field is additive, default 0). The socket is one-shot, so a push stream would need a second protocol; the app polls instead and fires the overlay when `blocked_seq` rises. Latency is at most one poll. A daemon restart resets the counter to 0, so the app treats a decrease as a reset and does not fire.
- Overlay: during the 10 second warning, a countdown over the game's window. After the kill, a block card with target and reason for 3 seconds, then it disappears. It never waits for a click (0003). Look chosen with /visual-options; grey shield until then.
- Failure view: daemon unreachable shows an alert glyph, "daemon not running" and the last numbers greyed with a timestamp. A `v` newer than the app knows shows "update the app" and no numbers. It never shows zero.
- Test seam: a fake daemon on a temp socket replays ok, warning, exceeded, degraded and a `blocked_seq` bump. Swift tests and headless snapshots run against it; one real kill at the end.
- Icons: SF Symbol placeholder in the menu bar, no app icon yet. Generated options later (open item). The menu bar icon must read at about 18 px.
- Name stays run-limiter. Renaming (bundle ID, `com.lucasmsa.run-limiter` label, socket directory) is its own change after this works. `roguestill` is the candidate.

## Consequences

- Widget numbers lag by the OS timeline refresh plus one poll (0002).
- The overlay needs the app running; enforcement does not (0002).
- `blocked_seq` counts processes terminated, so a game with two matching processes bumps it twice.
