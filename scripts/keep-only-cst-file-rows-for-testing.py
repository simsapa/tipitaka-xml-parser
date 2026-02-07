#!/usr/bin/env python3
"""Keep only xml_fragments rows for cst_files that have checked or moved fragments.

This script filters the xml_fragments table to keep only rows belonging to
cst_files that have at least one fragment with frag_review 'checked' or 'moved'.
All other rows are deleted.

Usage:
    python scripts/keep-only-cst-file-rows-for-testing.py <database_path>
"""

import sqlite3
import sys
from pathlib import Path


def main():
    if len(sys.argv) != 2:
        print("Usage: python keep-only-cst-file-rows-for-testing.py <database_path>")
        print()
        print("This script will:")
        print("  1. Find all cst_files with frag_review='checked' or 'moved'")
        print("  2. Delete all xml_fragments rows NOT in those cst_files")
        sys.exit(1)

    db_path = Path(sys.argv[1])

    if not db_path.exists():
        print(f"Error: Database file not found: {db_path}")
        sys.exit(1)

    print(f"Opening database: {db_path}")
    conn = sqlite3.connect(db_path)
    cursor = conn.cursor()

    # Count total rows before filtering
    cursor.execute("SELECT COUNT(*) FROM xml_fragments")
    total_before = cursor.fetchone()[0]
    print(f"Total rows before filtering: {total_before}")

    # Find distinct cst_file values with frag_review 'checked' or 'moved'
    print("\nFinding cst_files with 'checked' or 'moved' fragments...")
    cursor.execute("""
        SELECT DISTINCT cst_file
        FROM xml_fragments
        WHERE frag_review IN ('checked', 'moved')
        ORDER BY cst_file
    """)

    cst_files = [row[0] for row in cursor.fetchall()]

    # Manual additions for tests
    cst_files.extend(["s0102m.mul.xml", "s0102a.att.xml", "s0102t.tik.xml"])

    if not cst_files:
        print("No cst_files found with 'checked' or 'moved' frag_review")
        print("No rows will be deleted.")
        conn.close()
        return

    print(f"{len(cst_files)} cst_files will be kept: \n{"\n".join(cst_files)}\n")

    # Delete rows that don't belong to the selected cst_files
    placeholders = ','.join('?' * len(cst_files))
    print("\nDeleting rows not in the selected cst_files...")
    delete_query = f"""
        DELETE FROM xml_fragments
        WHERE cst_file NOT IN ({placeholders})
    """
    cursor.execute(delete_query, cst_files)
    deleted_count = cursor.rowcount

    # Commit changes
    conn.commit()

    # Count remaining rows
    cursor.execute("SELECT COUNT(*) FROM xml_fragments")
    total_after = cursor.fetchone()[0]

    # Get database size before VACUUM
    db_size_before = db_path.stat().st_size / (1024 * 1024)  # MB

    # Run VACUUM to reclaim space
    print("\nRunning VACUUM to reclaim disk space...")
    cursor.execute("VACUUM")
    conn.commit()

    conn.close()

    # Get database size after VACUUM
    db_size_after = db_path.stat().st_size / (1024 * 1024)  # MB
    space_saved = db_size_before - db_size_after

    print("\n✓ Deletion complete!")
    print(f"  Rows before:  {total_before}")
    print(f"  Rows deleted: {deleted_count}")
    print(f"  Rows after:   {total_after}")
    print(f"  CST files kept: {len(cst_files)}")
    print("\n✓ Database size reduced:")
    print(f"  Before: {db_size_before:.2f} MB")
    print(f"  After:  {db_size_after:.2f} MB")
    print(f"  Saved:  {space_saved:.2f} MB ({space_saved/db_size_before*100:.1f}%)")


if __name__ == "__main__":
    main()
