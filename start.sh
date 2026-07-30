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

exec ./server/target/release/bugbounty-server
