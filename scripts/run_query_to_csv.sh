#!/usr/bin/env bash
set -euo pipefail

# Resolve the directory containing this script (scripts/)
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
VENV_DIR="$SCRIPT_DIR/.venv"

cd "$SCRIPT_DIR"

# Create virtual environment if activate script is missing
if [[ ! -f "$VENV_DIR/bin/activate" ]]; then
    echo "Creating virtual environment at $VENV_DIR..."
    if ! python3 -m venv "$VENV_DIR"; then
        echo ""
        echo "ERROR: Failed to create virtual environment."
        echo "On Debian/Ubuntu, install the required package and retry:"
        echo "  sudo apt install python3-venv python3-full"
        exit 1
    fi
fi

# Activate virtual environment
source "$VENV_DIR/bin/activate"

# Install/upgrade dependencies
pip install --quiet --upgrade pip
pip install --quiet -r "$SCRIPT_DIR/requirements.txt"

# Run the script, forwarding all arguments
python "$SCRIPT_DIR/query_to_csv.py" "$@"
