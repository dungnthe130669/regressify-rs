#!/usr/bin/env bash
# Builds the website from the staged release manifest and archives, copies
# /downloads/latest.txt into dist/downloads/, verifies the release-matched MCP
# catalog and Worker, then deploys both as one Cloudflare Worker
# (website/wrangler.jsonc). Binary archives and the Windows installer are hosted
# on GitHub Releases; the website serves latest.txt, install.ps1, install.sh,
# and the /mcp endpoint.
set -euo pipefail

fail() {
  echo "error: $*" >&2
  exit 1
}

[ -n "${MCP_CATALOG:-}" ] || fail "MCP_CATALOG must name the release-matched catalog"
[ -f "$MCP_CATALOG" ] || fail "MCP_CATALOG '$MCP_CATALOG' does not exist"
[ -n "${DOWNLOADS_DIR:-}" ] || fail "DOWNLOADS_DIR must name the staged release downloads"
[ -f "$DOWNLOADS_DIR/latest.txt" ] || fail "DOWNLOADS_DIR '$DOWNLOADS_DIR' has no latest.txt"

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
website="$root/website"
worker="$website/mcp"
downloads="$(cd "$DOWNLOADS_DIR" && pwd)"

python - "$root/ci" "$downloads/latest.txt" "$MCP_CATALOG" <<'PY'
import json, sys
from pathlib import Path
sys.path.insert(0, sys.argv[1])
from release_feed import validate_mcp_catalog_version
manifest = Path(sys.argv[2]).read_text(encoding="utf-8")
catalog = json.loads(Path(sys.argv[3]).read_text(encoding="utf-8"))
validate_mcp_catalog_version(manifest, catalog)
PY

(
  cd "$website"
  bun install --frozen-lockfile
  bun run check
  # The release's own manifest and archives drive the download page.
  RELEASE_REQUIRED=1 RELEASE_MANIFEST_FILE="$downloads/latest.txt" bun run build
)
[ ! -e "$website/dist/downloads" ] || fail "The site build must not produce dist/downloads"
mkdir -p "$website/dist/downloads"
cp "$downloads/latest.txt" "$website/dist/downloads/latest.txt"

mkdir -p "$worker/content"
cp "$MCP_CATALOG" "$worker/content/catalog.json"
(
  cd "$worker"
  npm ci
  npm run check
  npm test
)

(
  cd "$website"
  bun run worker:check
  bun run worker:build
)

[ -n "${CLOUDFLARE_ACCOUNT_ID:-}" ] || fail "Set the CLOUDFLARE_ACCOUNT_ID secret to deploy."
[ -n "${CLOUDFLARE_API_TOKEN:-}" ] || fail "Set the CLOUDFLARE_API_TOKEN secret to deploy."
export WRANGLER_SEND_METRICS=false
cd "$website"
"$worker/node_modules/.bin/wrangler" deploy --config wrangler.jsonc
