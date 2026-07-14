#!/usr/bin/env bash
# Builds the release .app / .dmg bundle. Output lands under
# src-tauri/target/release/bundle/.
set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.."

if [ ! -d node_modules ]; then
  npm install
fi

npm run build
