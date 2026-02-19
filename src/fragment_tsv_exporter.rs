//! Export XML fragments to TSV format
//!
//! This module provides functionality to export the xml_fragments table
//! from the SQLite database to TSV format, excluding large fields like
//! content_xml and content_html.

use anyhow::{Result, Context};
use diesel::prelude::*;
use std::collections::BTreeMap;
use std::path::Path;
use std::io::Write;

use crate::fragment_exporter::establish_connection_and_migrate;
use crate::fragments_schema::xml_fragments;
use crate::types::compare_frag_idx_code;

/// Fragment data for TSV export (excludes id, content_xml, content_html, created_at)
#[derive(Queryable, Selectable)]
#[diesel(table_name = xml_fragments)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct FragmentTsvRecord {
    #[diesel(select_expression = xml_fragments::cst_file)]
    pub cst_file: String,
    #[diesel(select_expression = xml_fragments::frag_idx_code)]
    pub frag_idx_code: String,
    #[diesel(select_expression = xml_fragments::frag_type)]
    pub frag_type: String,
    #[diesel(select_expression = xml_fragments::frag_review)]
    pub frag_review: Option<String>,
    #[diesel(select_expression = xml_fragments::nikaya)]
    pub nikaya: String,
    #[diesel(select_expression = xml_fragments::cst_code)]
    pub cst_code: Option<String>,
    #[diesel(select_expression = xml_fragments::sc_code)]
    pub sc_code: Option<String>,
    #[diesel(select_expression = xml_fragments::cst_vagga)]
    pub cst_vagga: Option<String>,
    #[diesel(select_expression = xml_fragments::cst_sutta)]
    pub cst_sutta: Option<String>,
    #[diesel(select_expression = xml_fragments::cst_paranum)]
    pub cst_paranum: Option<String>,
    #[diesel(select_expression = xml_fragments::sc_sutta)]
    pub sc_sutta: Option<String>,
    #[diesel(select_expression = xml_fragments::start_line)]
    pub start_line: i32,
    #[diesel(select_expression = xml_fragments::start_char)]
    pub start_char: i32,
    #[diesel(select_expression = xml_fragments::end_line)]
    pub end_line: i32,
    #[diesel(select_expression = xml_fragments::end_char)]
    pub end_char: i32,
    #[diesel(select_expression = xml_fragments::group_levels)]
    pub group_levels: String,
}

/// Export xml_fragments table to TSV file
///
/// Exports all fields except: id, content_xml, content_html, created_at
///
/// # Arguments
/// * `db_path` - Path to the fragments database
/// * `output_path` - Path to write the TSV file
///
/// # Returns
/// Number of rows exported or error
pub fn export_fragments_to_tsv(db_path: &Path, output_path: &Path) -> Result<usize> {
    // Connect to database and run migrations
    let mut conn = establish_connection_and_migrate(db_path)?;

    // Query all fragments with only the fields we want
    use crate::fragments_schema::xml_fragments::dsl;

    let fragments: Vec<FragmentTsvRecord> = dsl::xml_fragments
        .select(FragmentTsvRecord::as_select())
        .order(dsl::cst_file.asc())
        .load(&mut conn)
        .context("Failed to query fragments")?;

    // Group by cst_file and sort each group by frag_idx_code (version-style)
    let mut grouped: BTreeMap<String, Vec<FragmentTsvRecord>> = BTreeMap::new();
    for frag in fragments {
        grouped.entry(frag.cst_file.clone())
            .or_insert_with(Vec::new)
            .push(frag);
    }

    // Sort each group using version-style comparison
    for frags in grouped.values_mut() {
        frags.sort_by(|a, b| compare_frag_idx_code(&a.frag_idx_code, &b.frag_idx_code));
    }

    // Write to TSV file
    let file = std::fs::File::create(output_path)
        .context(format!("Failed to create output file: {:?}", output_path))?;
    let mut writer = std::io::BufWriter::new(file);

    // Write header
    writeln!(writer, "{}",
        "cst_file\tfrag_idx_code\tfrag_type\tfrag_review\tnikaya\tcst_code\tsc_code\tcst_vagga\tcst_sutta\tcst_paranum\tsc_sutta\tstart_line\tstart_char\tend_line\tend_char\tgroup_levels"
    ).context("Failed to write TSV header")?;

    // Write rows
    let count: usize = grouped.values().map(|v| v.len()).sum();
    for frags in grouped.values() {
        for frag in frags {
            writeln!(writer, "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
                escape_tsv_field(&frag.cst_file),
                frag.frag_idx_code,
                escape_tsv_field(&frag.frag_type),
                frag.frag_review.as_deref().unwrap_or(""),
                escape_tsv_field(&frag.nikaya),
                frag.cst_code.as_deref().unwrap_or(""),
                frag.sc_code.as_deref().unwrap_or(""),
                frag.cst_vagga.as_deref().unwrap_or(""),
                frag.cst_sutta.as_deref().unwrap_or(""),
                frag.cst_paranum.as_deref().unwrap_or(""),
                frag.sc_sutta.as_deref().unwrap_or(""),
                frag.start_line,
                frag.start_char,
                frag.end_line,
                frag.end_char,
                escape_tsv_field(&frag.group_levels),
            ).context("Failed to write TSV row")?;
        }
    }

    writer.flush().context("Failed to flush TSV output")?;

    Ok(count)
}

/// Escape TSV field by replacing tabs and newlines
fn escape_tsv_field(field: &str) -> String {
    field
        .replace('\t', "\\t")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;
    use crate::{detect_nikaya_structure, parse_into_fragments, export_fragments_to_db};
    use crate::types::ParserOverrides;

    #[test]
    fn test_export_fragments_to_tsv() {
        // Create a simple XML sample
        let original_xml = r#"<?xml version="1.0"?>
<TEI.2>
<teiHeader></teiHeader>
<text>
<body>
<p rend="nikaya">Dīghanikāyo</p>
<div id="dn1" type="book">
<head rend="book">Sīlakkhandhavaggapāḷi</head>
<div id="dn1_1" type="sutta">
<head rend="chapter">1. Brahmajālasutta</head>
<p rend="bodytext" n="1">Evaṃ me sutaṃ.</p>
</div>
</div>
</body>
</text>
</TEI.2>"#;

        let temp_db = NamedTempFile::new().unwrap();
        let db_path = temp_db.path();

        let temp_tsv = NamedTempFile::new().unwrap();
        let tsv_path = temp_tsv.path();

        // Parse and export to DB
        let structure = detect_nikaya_structure(original_xml).unwrap();
        let fragments = parse_into_fragments(original_xml, &structure, "test.xml", &ParserOverrides::default(), false).unwrap();
        export_fragments_to_db(&fragments, &structure, db_path).unwrap();

        // Export to TSV
        let count = export_fragments_to_tsv(db_path, tsv_path).unwrap();

        // Verify
        assert_eq!(count, fragments.len());

        // Read TSV and verify format
        let tsv_content = std::fs::read_to_string(tsv_path).unwrap();
        let lines: Vec<&str> = tsv_content.lines().collect();

        // Should have header + data rows
        assert_eq!(lines.len(), fragments.len() + 1);

        // Check header
        assert!(lines[0].contains("cst_file"));
        assert!(lines[0].contains("frag_idx_code"));
        assert!(lines[0].contains("frag_type"));
        assert!(!lines[0].contains("content_xml"));
        assert!(!lines[0].contains("content_html"));
        assert!(!lines[0].contains("created_at"));

        // Check first data row
        assert!(lines[1].contains("test.xml"));
        assert!(lines[1].contains("digha"));
    }

    #[test]
    fn test_escape_tsv_field() {
        assert_eq!(escape_tsv_field("hello"), "hello");
        assert_eq!(escape_tsv_field("hello\tworld"), "hello\\tworld");
        assert_eq!(escape_tsv_field("hello\nworld"), "hello\\nworld");
        assert_eq!(escape_tsv_field("hello\rworld"), "hello\\rworld");
        assert_eq!(escape_tsv_field("a\tb\nc\rd"), "a\\tb\\nc\\rd");
    }
}
