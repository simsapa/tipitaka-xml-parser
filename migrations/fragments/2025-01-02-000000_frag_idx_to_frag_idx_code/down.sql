-- Rollback: Convert frag_idx_code TEXT back to frag_idx INTEGER
-- WARNING: Inserted fragments (sub-index > 0, e.g., "21.1") will be LOST on rollback
-- Only fragments with ".0" suffix can be converted back to integers

-- Step 1: Create table with original frag_idx INTEGER schema
CREATE TABLE xml_fragments_old (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    cst_file TEXT NOT NULL,
    frag_idx INTEGER NOT NULL,
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

-- Step 2: Copy data back, converting "N.0" to N (only .0 fragments, others are lost)
-- Extract the major index by taking substring before the '.'
INSERT INTO xml_fragments_old (
    id, cst_file, frag_idx, frag_type, frag_review, nikaya,
    cst_code, sc_code, content_xml, content_html,
    cst_vagga, cst_sutta, cst_paranum, sc_sutta,
    start_line, start_char, end_line, end_char,
    group_levels, created_at
)
SELECT
    id, cst_file,
    CAST(substr(frag_idx_code, 1, instr(frag_idx_code, '.') - 1) AS INTEGER),
    frag_type, frag_review, nikaya,
    cst_code, sc_code, content_xml, content_html,
    cst_vagga, cst_sutta, cst_paranum, sc_sutta,
    start_line, start_char, end_line, end_char,
    group_levels, created_at
FROM xml_fragments
WHERE substr(frag_idx_code, instr(frag_idx_code, '.') + 1) = '0';

-- Step 3: Drop the new table
DROP TABLE xml_fragments;

-- Step 4: Rename old table to original name
ALTER TABLE xml_fragments_old RENAME TO xml_fragments;
