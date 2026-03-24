#!/usr/bin/env bash
set -euo pipefail

# Resolve project root (one level up from this script)
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
VENV_DIR="$PROJECT_ROOT/.venv"

cd "$PROJECT_ROOT"

# Create virtual environment if it doesn't exist
if [[ ! -d "$VENV_DIR" ]]; then
    echo "Creating virtual environment at $VENV_DIR..."
    python3 -m venv "$VENV_DIR"
fi

# Activate virtual environment
source "$VENV_DIR/bin/activate"

# Install/upgrade dependencies
pip install --quiet --upgrade pip
pip install --quiet -r "$PROJECT_ROOT/requirements.txt"

# Run the script, forwarding all arguments
python "$PROJECT_ROOT/query_to_csv.py" "$@"
