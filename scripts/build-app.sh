#!/bin/sh
# Builds Roguestill.app (menu bar app, widget, embedded daemon) and copies it to ~/Applications.
# Needs: Rust (cargo), Xcode, xcodegen, and DEVELOPMENT_TEAM set to your Apple team ID.
set -e
: "${DEVELOPMENT_TEAM:?set DEVELOPMENT_TEAM to your Apple team ID}"
export PATH="$HOME/.cargo/bin:$PATH"
cd "$(dirname "$0")/../app/RoguestillBar"
xcodegen generate
xcodebuild -project Roguestill.xcodeproj -scheme Roguestill -configuration Release \
  -derivedDataPath .build/xc -allowProvisioningUpdates build
rm -rf "$HOME/Applications/Roguestill.app"
mkdir -p "$HOME/Applications"
cp -R .build/xc/Build/Products/Release/Roguestill.app "$HOME/Applications/"
open "$HOME/Applications/Roguestill.app"
