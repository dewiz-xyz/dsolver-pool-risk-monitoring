#!/usr/bin/env bash
set -euo pipefail

# Dump the 'result' and 'pool_result' tables from PostgreSQL as SQL files,
# then compress them into a single zip archive.
#
# Usage:
#   ./scripts/dump_tables.sh                         # uses DATABASE_URL from .env or environment
#   DATABASE_URL="postgres://user:pass@host/db" ./scripts/dump_tables.sh
#   ./scripts/dump_tables.sh --output-dir /tmp/dumps

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# Require pg_dump and zip
for cmd in pg_dump zip; do
    if ! command -v "$cmd" &>/dev/null; then
        echo "ERROR: '$cmd' is not installed or not in PATH."
        exit 1
    fi
done

# Load .env if present
ENV_FILE="$PROJECT_ROOT/.env"
if [[ -f "$ENV_FILE" ]]; then
    if [[ -z "${DATABASE_URL:-}" ]]; then
        DATABASE_URL="$(grep -E '^DATABASE_URL=' "$ENV_FILE" | cut -d'=' -f2-)"
        export DATABASE_URL
    fi
fi

if [[ -z "${DATABASE_URL:-}" ]]; then
    echo "ERROR: DATABASE_URL is not set. Add it to .env or export it before running this script."
    exit 1
fi

# Parse --output-dir argument (default: project root)
OUTPUT_DIR="$PROJECT_ROOT"
while [[ $# -gt 0 ]]; do
    case "$1" in
        --output-dir)
            OUTPUT_DIR="$2"
            shift 2
            ;;
        *)
            echo "Unknown argument: $1"
            echo "Usage: $0 [--output-dir <path>]"
            exit 1
            ;;
    esac
done

mkdir -p "$OUTPUT_DIR"

TIMESTAMP="$(date +%Y%m%d_%H%M%S)"
RESULT_FILE="$OUTPUT_DIR/result_${TIMESTAMP}.sql"
POOL_RESULT_FILE="$OUTPUT_DIR/pool_result_${TIMESTAMP}.sql"
ZIP_FILE="$OUTPUT_DIR/dump_${TIMESTAMP}.zip"

echo "Database : $DATABASE_URL"
echo "Output   : $OUTPUT_DIR"
echo ""

# Dump 'result' table (data only, INSERT statements)
echo "Dumping table 'result' → $RESULT_FILE ..."
pg_dump "$DATABASE_URL" \
    --no-password \
    --data-only \
    --inserts \
    --table=result \
    > "$RESULT_FILE"
echo "  Done. $(wc -l < "$RESULT_FILE") line(s) written."

# Dump 'pool_result' table (data only, INSERT statements)
echo "Dumping table 'pool_result' → $POOL_RESULT_FILE ..."
pg_dump "$DATABASE_URL" \
    --no-password \
    --data-only \
    --inserts \
    --table=pool_result \
    > "$POOL_RESULT_FILE"
echo "  Done. $(wc -l < "$POOL_RESULT_FILE") line(s) written."

# Compress both SQL files into a single zip archive
echo ""
echo "Compressing into $ZIP_FILE ..."
zip --junk-paths "$ZIP_FILE" "$RESULT_FILE" "$POOL_RESULT_FILE"

# Remove uncompressed SQL files
rm "$RESULT_FILE" "$POOL_RESULT_FILE"

echo ""
echo "Dump complete → $ZIP_FILE"
