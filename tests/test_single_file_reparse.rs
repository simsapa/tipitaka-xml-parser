//! Integration tests for single-file reparse feature
//!
//! Tests that reparsing a single file:
//! - Preserves checked fragment overrides
//! - Is idempotent (reparsing twice produces the same result)
//! - Correctly restores frag_review status

use std::path::PathBuf;
use tempfile::NamedTempFile;
use tipitaka_xml_parser::fragment_exporter::{
    export_fragments_to_db, extract_correction_overrides, restore_frag_review_status,
};
use tipitaka_xml_parser::nikaya_detector::detect_nikaya_structure;
use tipitaka_xml_parser::parse_into_fragments;
use tipitaka_xml_parser::types::ParserOverrides;

/// Test that single-file reparse is idempotent
#[test]
fn test_reparse_idempotent() {
    // Use a real test file
    let test_file = PathBuf::from("tests/data/s0101m.mul.xml");
    if !test_file.exists() {
        eprintln!("Skipping test: test file not found");
        return;
    }

    let temp_db = NamedTempFile::new().unwrap();
    let db_path = temp_db.path();

    // Read XML file
    let xml_content = std::fs::read_to_string(&test_file).unwrap();
    let structure = detect_nikaya_structure(&xml_content).unwrap();
    let cst_file = test_file.file_name().unwrap().to_str().unwrap();

    // First parse
    let fragments1 =
        parse_into_fragments(&xml_content, &structure, cst_file, &ParserOverrides::default(), true)
            .unwrap();
    export_fragments_to_db(&fragments1, &structure, db_path).unwrap();

    // Second parse (should produce identical results)
    let fragments2 =
        parse_into_fragments(&xml_content, &structure, cst_file, &ParserOverrides::default(), true)
            .unwrap();

    // Compare fragment counts
    assert_eq!(
        fragments1.len(),
        fragments2.len(),
        "Fragment count should be identical on reparse"
    );

    // Compare key fields of each fragment
    for (f1, f2) in fragments1.iter().zip(fragments2.iter()) {
        assert_eq!(f1.frag_idx, f2.frag_idx, "frag_idx should match");
        assert_eq!(f1.frag_type, f2.frag_type, "frag_type should match");
        assert_eq!(f1.nikaya, f2.nikaya, "nikaya should match");
        assert_eq!(f1.cst_code, f2.cst_code, "cst_code should match");
        assert_eq!(f1.start_line, f2.start_line, "start_line should match");
        assert_eq!(f1.end_line, f2.end_line, "end_line should match");
        assert_eq!(
            f1.content_xml.len(),
            f2.content_xml.len(),
            "content_xml length should match for frag_idx {}",
            f1.frag_idx
        );
    }
}

/// Test that reparse preserves checked fragment overrides
#[test]
fn test_reparse_preserves_correction_overrides() {
    use diesel::prelude::*;
    use diesel::sqlite::SqliteConnection;
    use tipitaka_xml_parser::fragments_schema::xml_fragments;

    // Use a real test file
    let test_file = PathBuf::from("tests/data/s0101m.mul.xml");
    if !test_file.exists() {
        eprintln!("Skipping test: test file not found");
        return;
    }

    let temp_db = NamedTempFile::new().unwrap();
    let db_path = temp_db.path();

    // Read and parse
    let xml_content = std::fs::read_to_string(&test_file).unwrap();
    let structure = detect_nikaya_structure(&xml_content).unwrap();
    let cst_file = test_file.file_name().unwrap().to_str().unwrap();

    let fragments =
        parse_into_fragments(&xml_content, &structure, cst_file, &ParserOverrides::default(), true)
            .unwrap();
    export_fragments_to_db(&fragments, &structure, db_path).unwrap();

    // Mark a fragment as "checked" with a custom sc_code
    let mut conn = SqliteConnection::establish(db_path.to_str().unwrap()).unwrap();

    let checked_sc_code = "dn1.1.test-override";
    let checked_sc_sutta = "Test Override Sutta";

    diesel::update(
        xml_fragments::table
            .filter(xml_fragments::cst_file.eq(cst_file))
            .filter(xml_fragments::frag_idx.eq(1)),
    )
    .set((
        xml_fragments::frag_review.eq("checked"),
        xml_fragments::sc_code.eq(checked_sc_code),
        xml_fragments::sc_sutta.eq(checked_sc_sutta),
    ))
    .execute(&mut conn)
    .unwrap();

    // Extract checked overrides
    let (correction_overrides, review_status) =
        extract_correction_overrides(db_path, cst_file).unwrap();

    assert!(!correction_overrides.is_empty(), "Should have checked overrides");

    // Re-parse with checked overrides
    let overrides = ParserOverrides {
        adjustments: None,
        correction_overrides: Some(correction_overrides),
    };

    let reparsed_fragments =
        parse_into_fragments(&xml_content, &structure, cst_file, &overrides, true).unwrap();

    // Export the reparsed fragments
    export_fragments_to_db(&reparsed_fragments, &structure, db_path).unwrap();

    // Restore frag_review status
    restore_frag_review_status(db_path, cst_file, &review_status).unwrap();

    // Verify the overrides were preserved
    let mut conn = SqliteConnection::establish(db_path.to_str().unwrap()).unwrap();

    let (sc_code, sc_sutta, frag_review): (Option<String>, Option<String>, Option<String>) =
        xml_fragments::table
            .filter(xml_fragments::cst_file.eq(cst_file))
            .filter(xml_fragments::frag_idx.eq(1))
            .select((
                xml_fragments::sc_code,
                xml_fragments::sc_sutta,
                xml_fragments::frag_review,
            ))
            .first(&mut conn)
            .unwrap();

    assert_eq!(
        sc_code.as_deref(),
        Some(checked_sc_code),
        "sc_code should be preserved after reparse"
    );
    assert_eq!(
        sc_sutta.as_deref(),
        Some(checked_sc_sutta),
        "sc_sutta should be preserved after reparse"
    );
    assert_eq!(
        frag_review.as_deref(),
        Some("checked"),
        "frag_review should be restored after reparse"
    );
}

/// Test that all checked fragments in a file maintain their review status
#[test]
fn test_reparse_restores_all_review_statuses() {
    use diesel::prelude::*;
    use diesel::sqlite::SqliteConnection;
    use tipitaka_xml_parser::fragments_schema::xml_fragments;

    // Use a real test file
    let test_file = PathBuf::from("tests/data/s0101m.mul.xml");
    if !test_file.exists() {
        eprintln!("Skipping test: test file not found");
        return;
    }

    let temp_db = NamedTempFile::new().unwrap();
    let db_path = temp_db.path();

    // Read and parse
    let xml_content = std::fs::read_to_string(&test_file).unwrap();
    let structure = detect_nikaya_structure(&xml_content).unwrap();
    let cst_file = test_file.file_name().unwrap().to_str().unwrap();

    let fragments =
        parse_into_fragments(&xml_content, &structure, cst_file, &ParserOverrides::default(), true)
            .unwrap();
    export_fragments_to_db(&fragments, &structure, db_path).unwrap();

    // Mark multiple fragments with different review statuses
    let mut conn = SqliteConnection::establish(db_path.to_str().unwrap()).unwrap();

    // Mark frag_idx 1 as "checked"
    diesel::update(
        xml_fragments::table
            .filter(xml_fragments::cst_file.eq(cst_file))
            .filter(xml_fragments::frag_idx.eq(1)),
    )
    .set(xml_fragments::frag_review.eq("checked"))
    .execute(&mut conn)
    .unwrap();

    // Mark frag_idx 2 as "in-progress"
    diesel::update(
        xml_fragments::table
            .filter(xml_fragments::cst_file.eq(cst_file))
            .filter(xml_fragments::frag_idx.eq(2)),
    )
    .set(xml_fragments::frag_review.eq("in-progress"))
    .execute(&mut conn)
    .unwrap();

    // Mark frag_idx 3 as "needs-review"
    diesel::update(
        xml_fragments::table
            .filter(xml_fragments::cst_file.eq(cst_file))
            .filter(xml_fragments::frag_idx.eq(3)),
    )
    .set(xml_fragments::frag_review.eq("needs-review"))
    .execute(&mut conn)
    .unwrap();

    // Extract checked overrides (should get all 3)
    let (correction_overrides, review_status) =
        extract_correction_overrides(db_path, cst_file).unwrap();

    assert_eq!(review_status.len(), 3, "Should have 3 review statuses");
    assert_eq!(
        review_status.get(&1).map(|s| s.as_str()),
        Some("checked")
    );
    assert_eq!(
        review_status.get(&2).map(|s| s.as_str()),
        Some("in-progress")
    );
    assert_eq!(
        review_status.get(&3).map(|s| s.as_str()),
        Some("needs-review")
    );

    // Re-parse and export
    let overrides = ParserOverrides {
        adjustments: None,
        correction_overrides: Some(correction_overrides),
    };

    let reparsed_fragments =
        parse_into_fragments(&xml_content, &structure, cst_file, &overrides, true).unwrap();
    export_fragments_to_db(&reparsed_fragments, &structure, db_path).unwrap();

    // Restore frag_review status
    let restored_count = restore_frag_review_status(db_path, cst_file, &review_status).unwrap();
    assert_eq!(restored_count, 3, "Should restore 3 review statuses");

    // Verify all statuses were restored
    let mut conn = SqliteConnection::establish(db_path.to_str().unwrap()).unwrap();

    let frag1_review: Option<String> = xml_fragments::table
        .filter(xml_fragments::cst_file.eq(cst_file))
        .filter(xml_fragments::frag_idx.eq(1))
        .select(xml_fragments::frag_review)
        .first(&mut conn)
        .unwrap();
    assert_eq!(frag1_review.as_deref(), Some("checked"));

    let frag2_review: Option<String> = xml_fragments::table
        .filter(xml_fragments::cst_file.eq(cst_file))
        .filter(xml_fragments::frag_idx.eq(2))
        .select(xml_fragments::frag_review)
        .first(&mut conn)
        .unwrap();
    assert_eq!(frag2_review.as_deref(), Some("in-progress"));

    let frag3_review: Option<String> = xml_fragments::table
        .filter(xml_fragments::cst_file.eq(cst_file))
        .filter(xml_fragments::frag_idx.eq(3))
        .select(xml_fragments::frag_review)
        .first(&mut conn)
        .unwrap();
    assert_eq!(frag3_review.as_deref(), Some("needs-review"));
}
