#!/usr/bin/env bash
# Runs RecallPet in development mode (hot-reloads the frontend, debug Rust build).
set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.."

if [ ! -d node_modules ]; then
  npm install
fi

npm run dev
