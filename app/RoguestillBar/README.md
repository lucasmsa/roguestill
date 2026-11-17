# RoguestillBar

Requires macOS 14+ and Swift 6. No third-party dependencies.

From the repository root, start the fake daemon:

```sh
python3 tools/fake-roguestilld.py /tmp/fk.sock cycle
```

In another terminal:

```sh
cd app/RoguestillBar
ROGUESTILL_SOCKET=/tmp/fk.sock swift run RoguestillBar
```

The app runs as an accessory with no Dock icon. Quit from its menu bar menu.
Without the environment override, it reads `~/Library/Application Support/roguestill/roguestilld.sock`.
The grey shield is a placeholder. The overlay sits at the top-right of the main screen and accepts no mouse input.

Build and test in environments requiring SwiftPM sandboxing to be disabled:

```sh
swift build --disable-sandbox
swift test --disable-sandbox
```
