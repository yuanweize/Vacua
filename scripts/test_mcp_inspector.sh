#!/usr/bin/env bash
set -euo pipefail

echo "=== Vacua MCP Official Inspector CI Gate ==="

INSPECTOR_PKG="@modelcontextprotocol/inspector@2.8.0"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKSPACE_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

VACUA_MCP_BIN="${1:-$WORKSPACE_ROOT/target/debug/vacua-mcp}"
FIXTURE_GEN_BIN="$WORKSPACE_ROOT/target/debug/inspector_fixture"

if [[ ! -x "$VACUA_MCP_BIN" ]]; then
    echo "ERROR: vacua-mcp binary not found at $VACUA_MCP_BIN"
    exit 1
fi

if [[ ! -x "$FIXTURE_GEN_BIN" ]]; then
    echo "Building inspector_fixture..."
    cargo build -p vacua-mcp --bin inspector_fixture
fi

FIXTURE_DIR="$(mktemp -d /tmp/vacua-inspector-XXXXXX)"
trap 'rm -rf "$FIXTURE_DIR"' EXIT

ROOT_DIR="$FIXTURE_DIR/root"
DB_DIR="$FIXTURE_DIR/db"
mkdir -p "$ROOT_DIR/Library/Caches/Homebrew/downloads"
mkdir -p "$ROOT_DIR/Duplicates"
mkdir -p "$DB_DIR"

# Seed a candidate item matching Homebrew cache rule
echo "bottle cache bytes for candidate test" > "$ROOT_DIR/Library/Caches/Homebrew/downloads/sample.bottle.tar.gz"

# Seed duplicate items
echo "IDENTICAL_DUPLICATE_BYTES_9876543210" > "$ROOT_DIR/Duplicates/dup_1.bin"
echo "IDENTICAL_DUPLICATE_BYTES_9876543210" > "$ROOT_DIR/Duplicates/dup_2.bin"

# Seed index snapshot
SNAP_ID="$("$FIXTURE_GEN_BIN" "$ROOT_DIR" "$DB_DIR/index.db")"
echo "Seeded snapshot ID: $SNAP_ID"

# Seed a developer project
mkdir -p "$ROOT_DIR/SampleCargoProject/src"
mkdir -p "$ROOT_DIR/SampleCargoProject/target/debug"
echo '[package]' > "$ROOT_DIR/SampleCargoProject/Cargo.toml"
echo 'name = "sample-crate"' >> "$ROOT_DIR/SampleCargoProject/Cargo.toml"
echo '# lock' > "$ROOT_DIR/SampleCargoProject/Cargo.lock"
echo 'fn main() {}' > "$ROOT_DIR/SampleCargoProject/src/main.rs"
echo 'compiled bytes' > "$ROOT_DIR/SampleCargoProject/target/debug/app"

# Create a clean shell wrapper so inspector-cli doesn't misparse vacua-mcp arguments
WRAPPER_SCRIPT="$FIXTURE_DIR/run_vacua_mcp.sh"
cat << EOF > "$WRAPPER_SCRIPT"
#!/usr/bin/env bash
exec "$VACUA_MCP_BIN" --allow-root "$ROOT_DIR" --index "$DB_DIR/index.db" --journal "$DB_DIR/journal.db" --allow-system-app-metadata "\$@"
EOF
chmod +x "$WRAPPER_SCRIPT"

echo "1. Checking tools/list..."
npx -y "$INSPECTOR_PKG" --cli "$WRAPPER_SCRIPT" --method tools/list --format json > "$FIXTURE_DIR/tools.json"
grep -q "vacua_get_capabilities" "$FIXTURE_DIR/tools.json"
grep -q "vacua_storage_summary" "$FIXTURE_DIR/tools.json"
grep -q "vacua_list_candidates" "$FIXTURE_DIR/tools.json"
grep -q "vacua_list_duplicates" "$FIXTURE_DIR/tools.json"
grep -q "vacua_analyze_developer_artifacts" "$FIXTURE_DIR/tools.json"
grep -q "vacua_list_developer_artifacts" "$FIXTURE_DIR/tools.json"
grep -q "vacua_get_developer_artifact" "$FIXTURE_DIR/tools.json"

echo "2. Checking resources/list..."
npx -y "$INSPECTOR_PKG" --cli "$WRAPPER_SCRIPT" --method resources/list --format json > "$FIXTURE_DIR/resources.json"
grep -q "vacua://capabilities" "$FIXTURE_DIR/resources.json"
grep -q "vacua://storage/summary" "$FIXTURE_DIR/resources.json"

echo "3. Checking resources/templates/list..."
npx -y "$INSPECTOR_PKG" --cli "$WRAPPER_SCRIPT" --method resources/templates/list --format json > "$FIXTURE_DIR/templates.json"
grep -q "vacua://candidate/{candidate_id}" "$FIXTURE_DIR/templates.json"
grep -q "vacua://snapshot/{snapshot_id}" "$FIXTURE_DIR/templates.json"
grep -q "vacua://duplicate/{group_id}" "$FIXTURE_DIR/templates.json"
grep -q "vacua://application/{application_id}" "$FIXTURE_DIR/templates.json"
grep -q "vacua://developer-artifact/{artifact_id}" "$FIXTURE_DIR/templates.json"

echo "4. Checking prompts/list..."
npx -y "$INSPECTOR_PKG" --cli "$WRAPPER_SCRIPT" --method prompts/list --format json > "$FIXTURE_DIR/prompts.json"
grep -q "review_storage_growth" "$FIXTURE_DIR/prompts.json"
grep -q "review_cleanup_proposal" "$FIXTURE_DIR/prompts.json"

echo "5. Calling vacua_get_capabilities..."
npx -y "$INSPECTOR_PKG" --cli "$WRAPPER_SCRIPT" --method tools/call --tool-name vacua_get_capabilities --format json > "$FIXTURE_DIR/cap.json"
grep -q '"mutation_authority":false' "$FIXTURE_DIR/cap.json"
grep -q '"executor_linked":false' "$FIXTURE_DIR/cap.json"
grep -q '"plan_export_enabled":false' "$FIXTURE_DIR/cap.json"

echo "6. Calling vacua_storage_summary..."
npx -y "$INSPECTOR_PKG" --cli "$WRAPPER_SCRIPT" --method tools/call --tool-name vacua_storage_summary --format json > "$FIXTURE_DIR/storage.json"
grep -q '"filesystem_type":"apfs"' "$FIXTURE_DIR/storage.json"

echo "7. Reading static resource: vacua://capabilities..."
npx -y "$INSPECTOR_PKG" --cli "$WRAPPER_SCRIPT" --method resources/read --uri "vacua://capabilities" --format json > "$FIXTURE_DIR/read_cap.json"
grep -q "vacua.mcp.server-capabilities.v1" "$FIXTURE_DIR/read_cap.json"

echo "8. Verifying ResourceTemplate: vacua://snapshot/{snapshot_id}..."
npx -y "$INSPECTOR_PKG" --cli "$WRAPPER_SCRIPT" --method resources/read --uri "vacua://snapshot/$SNAP_ID" --format json > "$FIXTURE_DIR/read_snap.json"
grep -q "$SNAP_ID" "$FIXTURE_DIR/read_snap.json"
grep -q "vacua.mcp.snapshot-detail.v1" "$FIXTURE_DIR/read_snap.json"

echo "9. Listing candidates to obtain candidate_id..."
npx -y "$INSPECTOR_PKG" --cli "$WRAPPER_SCRIPT" --method tools/call --tool-name vacua_list_candidates --format json > "$FIXTURE_DIR/cands.json"
CAND_ID=$(grep -o '"candidate_id":"cand-[^"]*"' "$FIXTURE_DIR/cands.json" | head -n1 | cut -d'"' -f4 || true)

if [[ -n "$CAND_ID" ]]; then
    echo "Verifying ResourceTemplate: vacua://candidate/$CAND_ID..."
    npx -y "$INSPECTOR_PKG" --cli "$WRAPPER_SCRIPT" --method resources/read --uri "vacua://candidate/$CAND_ID" --format json > "$FIXTURE_DIR/read_cand.json"
    grep -q "$CAND_ID" "$FIXTURE_DIR/read_cand.json"
    grep -q "vacua.mcp.candidate-detail.v1" "$FIXTURE_DIR/read_cand.json"
fi

echo "10. Listing duplicates to obtain group_id..."
npx -y "$INSPECTOR_PKG" --cli "$WRAPPER_SCRIPT" --method tools/call --tool-name vacua_list_duplicates --tool-args-json '{"min_size_bytes":1}' --format json > "$FIXTURE_DIR/dups.json"
DUP_ID=$(grep -o '"group_id":"dup-[^"]*"' "$FIXTURE_DIR/dups.json" | head -n1 | cut -d'"' -f4 || true)

if [[ -n "$DUP_ID" ]]; then
    echo "Verifying ResourceTemplate: vacua://duplicate/$DUP_ID..."
    npx -y "$INSPECTOR_PKG" --cli "$WRAPPER_SCRIPT" --method resources/read --uri "vacua://duplicate/$DUP_ID" --format json > "$FIXTURE_DIR/read_dup.json"
    grep -q "$DUP_ID" "$FIXTURE_DIR/read_dup.json"
    grep -q "vacua.mcp.duplicate-group.v1" "$FIXTURE_DIR/read_dup.json"
fi

echo "11. Listing applications to obtain application_id..."
npx -y "$INSPECTOR_PKG" --cli "$WRAPPER_SCRIPT" --method tools/call --tool-name vacua_list_applications --format json > "$FIXTURE_DIR/apps.json"
APP_ID=$(grep -o '"application_id":"[^"]*"' "$FIXTURE_DIR/apps.json" | head -n1 | cut -d'"' -f4 || true)

if [[ -n "$APP_ID" ]]; then
    echo "Verifying ResourceTemplate: vacua://application/$APP_ID..."
    npx -y "$INSPECTOR_PKG" --cli "$WRAPPER_SCRIPT" --method resources/read --uri "vacua://application/$APP_ID" --format json > "$FIXTURE_DIR/read_app.json"
    grep -q "$APP_ID" "$FIXTURE_DIR/read_app.json"
    grep -q "vacua.mcp.application-detail.v1" "$FIXTURE_DIR/read_app.json"
fi

echo "12. Calling vacua_analyze_developer_artifacts..."
npx -y "$INSPECTOR_PKG" --cli "$WRAPPER_SCRIPT" --method tools/call --tool-name vacua_analyze_developer_artifacts --format json > "$FIXTURE_DIR/analyze_art.json"
grep -q "vacua.mcp.developer-artifact-analysis.v1" "$FIXTURE_DIR/analyze_art.json"
grep -q "SampleCargoProject" "$FIXTURE_DIR/analyze_art.json"

echo "13. Calling vacua_list_developer_artifacts..."
npx -y "$INSPECTOR_PKG" --cli "$WRAPPER_SCRIPT" --method tools/call --tool-name vacua_list_developer_artifacts --format json > "$FIXTURE_DIR/list_art.json"
grep -q "vacua.mcp.developer-artifact-page.v1" "$FIXTURE_DIR/list_art.json"
ART_ID=$(grep -o '"artifact_id":"devart_[^"]*"' "$FIXTURE_DIR/list_art.json" | head -n1 | cut -d'"' -f4 || true)

if [[ -n "$ART_ID" ]]; then
    echo "14. Calling vacua_get_developer_artifact for $ART_ID..."
    npx -y "$INSPECTOR_PKG" --cli "$WRAPPER_SCRIPT" --method tools/call --tool-name vacua_get_developer_artifact --tool-args-json "{\"artifact_id\":\"$ART_ID\"}" --format json > "$FIXTURE_DIR/get_art.json"
    grep -q "vacua.mcp.developer-artifact-detail.v1" "$FIXTURE_DIR/get_art.json"
    grep -q "rebuild_evidence" "$FIXTURE_DIR/get_art.json"
    grep -q "cargo build" "$FIXTURE_DIR/get_art.json"

    echo "15. Verifying ResourceTemplate: vacua://developer-artifact/$ART_ID..."
    npx -y "$INSPECTOR_PKG" --cli "$WRAPPER_SCRIPT" --method resources/read --uri "vacua://developer-artifact/$ART_ID" --format json > "$FIXTURE_DIR/read_art.json"
    grep -q "$ART_ID" "$FIXTURE_DIR/read_art.json"
    grep -q "vacua.mcp.developer-artifact-detail.v1" "$FIXTURE_DIR/read_art.json"
fi

echo "=== All Inspector Protocol Verification Steps Passed! ==="
