#!/bin/bash
# Start the Bug Bounty Vault server
# Run this from the repo root directory

set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
cd "$SCRIPT_DIR"

echo "🚀 Starting Bug Bounty Vault..."
echo "   Repo root: $SCRIPT_DIR"
echo "   Server:    http://localhost:3000"
echo ""

# One server at a time: Tantivy's index uses a file lock, so a stale instance
# on :3000 must be gone before we start. Kill it and drop any lingering locks.
if PIDS=$(lsof -ti :3000 2>/dev/null); then
  echo "   Found stale server on :3000 — stopping (PIDs: $(echo $PIDS | tr '\n' ' '))"
  echo "$PIDS" | xargs kill 2>/dev/null || true
  # Give it a moment to release the index, then clear any orphaned lock files.
  sleep 1
fi
rm -f .search_index/.tantivy-*.lock 2>/dev/null || true

exec ./server/target/release/bugbounty-server
