#!/usr/bin/env python3
"""
query_to_csv.py — Run a SQL query against a PostgreSQL database and export results to CSV.

Usage:
    python query_to_csv.py                        # uses defaults from .env / environment
    python query_to_csv.py --query "SELECT * FROM result LIMIT 100"
    python query_to_csv.py --query "SELECT * FROM pool_result" --output pool_result.csv
    python query_to_csv.py --db-url "postgres://user:pass@host:5432/dbname"

Dependencies:
    pip install psycopg2-binary python-dotenv
"""

import argparse
import csv
import os
import sys
from urllib.parse import urlparse

# Optional: load .env file if python-dotenv is available
try:
    from dotenv import load_dotenv
    load_dotenv()
except ImportError:
    pass

try:
    import psycopg2
    import psycopg2.extras
except ImportError:
    print("Error: psycopg2 is not installed. Run: pip install psycopg2-binary", file=sys.stderr)
    sys.exit(1)

DEFAULT_QUERY = "SELECT * FROM pool_result LIMIT 1000;"
DEFAULT_OUTPUT = "output.csv"


def parse_db_url(url: str) -> dict:
    """Parse a postgres:// URL into psycopg2 connection kwargs."""
    parsed = urlparse(url)
    return {
        "host": parsed.hostname or "localhost",
        "port": parsed.port or 5432,
        "dbname": parsed.path.lstrip("/") or "postgres",
        "user": parsed.username,
        "password": parsed.password,
    }


def run_query_to_csv(db_url: str, query: str, output_path: str) -> int:
    """Connect to PostgreSQL, execute query, and write results to a CSV file.

    Returns the number of rows written.
    """
    conn_kwargs = parse_db_url(db_url)

    conn = psycopg2.connect(**conn_kwargs)
    try:
        with conn.cursor(cursor_factory=psycopg2.extras.RealDictCursor) as cur:
            cur.execute(query)
            rows = cur.fetchall()

            if not rows:
                print("Query returned no rows.")
                with open(output_path, "w", newline="", encoding="utf-8") as f:
                    f.write("")
                return 0

            fieldnames = list(rows[0].keys())

            with open(output_path, "w", newline="", encoding="utf-8") as f:
                writer = csv.DictWriter(f, fieldnames=fieldnames)
                writer.writeheader()
                writer.writerows(rows)

            return len(rows)
    finally:
        conn.close()


def main():
    parser = argparse.ArgumentParser(
        description="Execute a SQL query and export results to CSV."
    )
    parser.add_argument(
        "--db-url",
        default=os.environ.get("DATABASE_URL", "postgres://rust:teste@localhost:5432/postgres"),
        help="PostgreSQL connection URL (default: $DATABASE_URL from environment or .env)",
    )
    parser.add_argument(
        "--query",
        default=DEFAULT_QUERY,
        help=f"SQL query to execute (default: {DEFAULT_QUERY!r})",
    )
    parser.add_argument(
        "--output",
        default=DEFAULT_OUTPUT,
        help=f"Output CSV file path (default: {DEFAULT_OUTPUT})",
    )
    args = parser.parse_args()

    print(f"Connecting to database...")
    print(f"Query : {args.query}")
    print(f"Output: {args.output}")

    try:
        row_count = run_query_to_csv(args.db_url, args.query, args.output)
        print(f"Done. {row_count} row(s) written to '{args.output}'.")
    except psycopg2.OperationalError as e:
        print(f"Connection error: {e}", file=sys.stderr)
        sys.exit(1)
    except psycopg2.Error as e:
        print(f"Database error: {e}", file=sys.stderr)
        sys.exit(1)
    except OSError as e:
        print(f"File error: {e}", file=sys.stderr)
        sys.exit(1)


if __name__ == "__main__":
    main()
