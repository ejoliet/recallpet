#!/usr/bin/env bash
# Deletes RecallPet's entire local data directory (the SQLite database and
# its WAL/SHM files). Quit RecallPet before running this. Irreversible.
set -euo pipefail

DATA_DIR="$HOME/Library/Application Support/RecallPet"

echo "This will permanently delete: $DATA_DIR"
read -r -p "Type 'yes' to continue: " confirmation

if [ "$confirmation" != "yes" ]; then
  echo "Aborted. Nothing was deleted."
  exit 1
fi

if [ -d "$DATA_DIR" ]; then
  rm -rf "$DATA_DIR"
  echo "Deleted $DATA_DIR"
else
  echo "Nothing to delete: $DATA_DIR does not exist."
fi
