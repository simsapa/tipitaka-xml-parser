-- Migration: Convert frag_idx INTEGER to frag_idx_code TEXT
-- This enables support for inserted fragments with sub-indices (e.g., "21.1" between "21.0" and "22.0")

-- Step 1: Create new table with frag_idx_code TEXT instead of frag_idx INTEGER
CREATE TABLE xml_fragments_new (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    cst_file TEXT NOT NULL,
    frag_idx_code TEXT NOT NULL,
    frag_type TEXT NOT NULL,
    frag_review TEXT,
    nikaya TEXT NOT NULL,
    cst_code TEXT,
    sc_code TEXT,
    content_xml TEXT NOT NULL,
    content_html TEXT,
    cst_vagga TEXT,
    cst_sutta TEXT,
    cst_paranum TEXT,
    sc_sutta TEXT,
    start_line INTEGER NOT NULL,
    start_char INTEGER NOT NULL,
    end_line INTEGER NOT NULL,
    end_char INTEGER NOT NULL,
    group_levels TEXT NOT NULL,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (nikaya) REFERENCES nikaya_structures(nikaya)
);

-- Step 2: Copy data with conversion (integer N -> string "N.0")
INSERT INTO xml_fragments_new (
    id, cst_file, frag_idx_code, frag_type, frag_review, nikaya,
    cst_code, sc_code, content_xml, content_html,
    cst_vagga, cst_sutta, cst_paranum, sc_sutta,
    start_line, start_char, end_line, end_char,
    group_levels, created_at
)
SELECT
    id, cst_file, CAST(frag_idx AS TEXT) || '.0', frag_type, frag_review, nikaya,
    cst_code, sc_code, content_xml, content_html,
    cst_vagga, cst_sutta, cst_paranum, sc_sutta,
    start_line, start_char, end_line, end_char,
    group_levels, created_at
FROM xml_fragments;

-- Step 3: Drop old table
DROP TABLE xml_fragments;

-- Step 4: Rename new table to original name
ALTER TABLE xml_fragments_new RENAME TO xml_fragments;
