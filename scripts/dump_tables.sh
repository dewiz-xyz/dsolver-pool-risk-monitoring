#!/usr/bin/env bash
set -euo pipefail

# Dump the 'result' and 'pool_result' tables from PostgreSQL.
#
# Usage:
#   ./scripts/dump_tables.sh                         # uses DATABASE_URL from .env or environment
#   DATABASE_URL="postgres://user:pass@host/db" ./scripts/dump_tables.sh
#   ./scripts/dump_tables.sh --output-dir /tmp/dumps

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# Load .env if present
ENV_FILE="$PROJECT_ROOT/.env"
if [[ -f "$ENV_FILE" ]]; then
    # Export only DATABASE_URL if not already set
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
RESULT_FILE="$OUTPUT_DIR/result_${TIMESTAMP}.csv"
POOL_RESULT_FILE="$OUTPUT_DIR/pool_result_${TIMESTAMP}.csv"

echo "Database : $DATABASE_URL"
echo "Output   : $OUTPUT_DIR"
echo ""

# Dump 'result' table
echo "Dumping table 'result' → $RESULT_FILE ..."
psql "$DATABASE_URL" \
    --no-password \
    --tuples-only \
    --command "\COPY result TO STDOUT WITH (FORMAT CSV, HEADER TRUE)" \
    > "$RESULT_FILE"
echo "  Done. $(wc -l < "$RESULT_FILE") line(s) written (including header)."

# Dump 'pool_result' table
echo "Dumping table 'pool_result' → $POOL_RESULT_FILE ..."
psql "$DATABASE_URL" \
    --no-password \
    --tuples-only \
    --command "\COPY pool_result TO STDOUT WITH (FORMAT CSV, HEADER TRUE)" \
    > "$POOL_RESULT_FILE"
echo "  Done. $(wc -l < "$POOL_RESULT_FILE") line(s) written (including header)."

echo ""
echo "Dump complete."
