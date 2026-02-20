//! Integration tests for CorrectionFragmentOverrides
//!
//! Tests that CorrectionFragmentOverrides work correctly
//! and that SC code parsing and propagation work correctly.

use tipitaka_xml_parser::nikaya_detector::detect_nikaya_structure;
use tipitaka_xml_parser::parse_into_fragments;
use tipitaka_xml_parser::types::{
    CorrectionFragmentOverride, CorrectionFragmentOverrides,
    FragmentKey, FragmentType, ParserOverrides,
};

/// Test that CorrectionFragmentOverrides are applied correctly
#[test]
fn test_correction_overrides_precedence() {
    // Create a simple XML sample
    let xml = r#"<?xml version="1.0"?>
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

    let structure = detect_nikaya_structure(xml).unwrap();

    // Parse without overrides first
    let fragments_no_override =
        parse_into_fragments(xml, &structure, "test.xml", &ParserOverrides::default(), false)
            .unwrap();

    // Find a sutta fragment
    let sutta_frag = fragments_no_override
        .iter()
        .find(|f| matches!(f.frag_type, FragmentType::Sutta))
        .expect("Should find a Sutta fragment");

    // Now parse with a checked override for sc_code
    let mut correction_overrides = CorrectionFragmentOverrides::new();
    let key = FragmentKey {
        cst_file: "test.xml".to_string(),
        frag_idx_code: sutta_frag.frag_idx_code.clone(),
    };
    correction_overrides.insert(
        key,
        CorrectionFragmentOverride {
            collapse: false,
            start_line: None,
            start_char: None,
            end_line: None,
            end_char: None,
            sc_code: Some("dn1.override".to_string()),
            sc_sutta: Some("Override Sutta Title".to_string()),
            cst_code: None,
            cst_vagga: None,
            cst_sutta: None,
            cst_paranum: None,
            frag_review: None,
            frag_type: None,
        },
    );

    let overrides = ParserOverrides {
        correction_overrides: Some(correction_overrides),
        inserted_fragments: None,
        pali_titles: None,
    };

    let fragments_with_override =
        parse_into_fragments(xml, &structure, "test.xml", &overrides, false).unwrap();

    // Find the same fragment
    let overridden_frag = fragments_with_override
        .iter()
        .find(|f| f.frag_idx_code == sutta_frag.frag_idx_code)
        .expect("Should find the same fragment");

    // The sc_code and sc_sutta should be overridden
    assert_eq!(
        overridden_frag.sc_code.as_deref(),
        Some("dn1.override"),
        "sc_code should be overridden"
    );
    assert_eq!(
        overridden_frag.sc_sutta.as_deref(),
        Some("Override Sutta Title"),
        "sc_sutta should be overridden"
    );
}

/// Test SC code parsing for different nikaya types
#[test]
fn test_sc_code_parsing() {
    use tipitaka_xml_parser::parsers::helpers::parse_sc_code;

    // DN format: dn1
    let dn = parse_sc_code("dn1").unwrap();
    assert_eq!(dn.prefix, "dn");
    assert_eq!(dn.sutta, Some(1));
    assert_eq!(dn.samyutta, None);
    assert_eq!(dn.nipata, None);

    // MN format: mn41
    let mn = parse_sc_code("mn41").unwrap();
    assert_eq!(mn.prefix, "mn");
    assert_eq!(mn.sutta, Some(41));

    // SN format: sn5.1
    let sn = parse_sc_code("sn5.1").unwrap();
    assert_eq!(sn.prefix, "sn");
    assert_eq!(sn.samyutta, Some(5));
    assert_eq!(sn.sutta, Some(1));

    // AN format: an3.1
    let an = parse_sc_code("an3.1").unwrap();
    assert_eq!(an.prefix, "an");
    assert_eq!(an.nipata, Some(3));
    assert_eq!(an.sutta, Some(1));

    // Invalid format
    assert!(parse_sc_code("invalid").is_none());
    assert!(parse_sc_code("").is_none());
}

/// Test that database extraction functions work correctly
#[test]
fn test_extract_correction_overrides() {
    use tempfile::NamedTempFile;
    use tipitaka_xml_parser::fragment_exporter::{
        export_fragments_to_db, extract_correction_overrides, restore_frag_review_status,
    };

    // Create test XML
    let xml = r#"<?xml version="1.0"?>
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

    // Parse and export
    let structure = detect_nikaya_structure(xml).unwrap();
    let fragments =
        parse_into_fragments(xml, &structure, "test.xml", &ParserOverrides::default(), false)
            .unwrap();
    export_fragments_to_db(&fragments, &structure, db_path).unwrap();

    // Set a fragment as "checked" by directly updating the database
    use diesel::prelude::*;
    use diesel::sqlite::SqliteConnection;
    use tipitaka_xml_parser::fragments_schema::xml_fragments;

    let mut conn = SqliteConnection::establish(db_path.to_str().unwrap()).unwrap();

    // Update frag_idx_code "1.0" to be "checked"
    diesel::update(
        xml_fragments::table
            .filter(xml_fragments::cst_file.eq("test.xml"))
            .filter(xml_fragments::frag_idx_code.eq("1.0")),
    )
    .set((
        xml_fragments::frag_review.eq("checked"),
        xml_fragments::sc_code.eq("dn1.test"),
    ))
    .execute(&mut conn)
    .unwrap();

    // Now extract checked overrides
    let (overrides, review_status, _inserted_fragments) = extract_correction_overrides(db_path, "test.xml").unwrap();

    assert_eq!(overrides.len(), 1, "Should have 1 checked override");
    assert_eq!(
        review_status.len(),
        1,
        "Should have 1 frag_review status to restore"
    );

    let key = FragmentKey {
        cst_file: "test.xml".to_string(),
        frag_idx_code: "1.0".to_string(),
    };
    let override_data = overrides.get(&key).expect("Should find override for frag_idx_code 1.0");
    assert_eq!(
        override_data.sc_code.as_deref(),
        Some("dn1.test"),
        "sc_code should be extracted"
    );

    // Test restore_frag_review_status
    // First clear the frag_review to simulate a reparse
    diesel::update(
        xml_fragments::table
            .filter(xml_fragments::cst_file.eq("test.xml"))
            .filter(xml_fragments::frag_idx_code.eq("1.0")),
    )
    .set(xml_fragments::frag_review.eq::<Option<String>>(None))
    .execute(&mut conn)
    .unwrap();

    // Now restore
    let restored = restore_frag_review_status(db_path, "test.xml", &review_status).unwrap();
    assert_eq!(restored, 1, "Should restore 1 frag_review status");

    // Verify it was restored
    let frag_review: Option<String> = xml_fragments::table
        .filter(xml_fragments::cst_file.eq("test.xml"))
        .filter(xml_fragments::frag_idx_code.eq("1.0"))
        .select(xml_fragments::frag_review)
        .first(&mut conn)
        .unwrap();

    assert_eq!(
        frag_review.as_deref(),
        Some("checked"),
        "frag_review should be restored"
    );
}

/// Test that boundary overrides from checked fragments are applied during parsing
///
/// This test verifies that when a checked fragment has end_line/end_char values,
/// they are used to determine the fragment boundaries instead of the default detection.
#[test]
fn test_boundary_override_from_checked_fragment() {
    // Create test XML with multiple lines to make boundary changes measurable
    let xml = r#"<?xml version="1.0"?>
<TEI.2>
<teiHeader></teiHeader>
<text>
<body>
<p rend="nikaya">Dīghanikāyo</p>
<div id="dn1" type="book">
<head rend="book">Sīlakkhandhavaggapāḷi</head>
<div id="dn1_1" type="sutta">
<head rend="chapter">1. Brahmajālasutta</head>
<p rend="bodytext" n="1">First paragraph.</p>
<p rend="bodytext" n="2">Second paragraph.</p>
<p rend="bodytext" n="3">Third paragraph.</p>
</div>
</div>
</body>
</text>
</TEI.2>"#;

    let structure = detect_nikaya_structure(xml).unwrap();

    // Parse without overrides first to get baseline
    let fragments_baseline =
        parse_into_fragments(xml, &structure, "test.xml", &ParserOverrides::default(), false)
            .unwrap();

    // Find the sutta fragment
    let sutta_frag = fragments_baseline
        .iter()
        .find(|f| matches!(f.frag_type, FragmentType::Sutta))
        .expect("Should find a Sutta fragment");

    let original_end_line = sutta_frag.end_line;
    let original_content_len = sutta_frag.content_xml.len();

    // Now parse with a checked override that changes the end boundary
    // Set end_line to one line before the original end (still after start)
    assert!(
        original_end_line > sutta_frag.start_line + 1,
        "Sutta fragment should span multiple lines for this test"
    );
    let override_end_line = original_end_line - 1; // One line before the end

    let mut correction_overrides = CorrectionFragmentOverrides::new();
    let key = FragmentKey {
        cst_file: "test.xml".to_string(),
        frag_idx_code: sutta_frag.frag_idx_code.clone(),
    };
    correction_overrides.insert(
        key,
        CorrectionFragmentOverride {
            collapse: false,
            start_line: None,
            start_char: None,
            end_line: Some(override_end_line),
            end_char: Some(0),
            sc_code: None,
            sc_sutta: None,
            cst_code: None,
            cst_vagga: None,
            cst_sutta: None,
            cst_paranum: None,
            frag_review: None,
            frag_type: None,
        },
    );

    let overrides = ParserOverrides {
        correction_overrides: Some(correction_overrides),
        inserted_fragments: None,
        pali_titles: None,
    };

    let fragments_with_override =
        parse_into_fragments(xml, &structure, "test.xml", &overrides, false).unwrap();

    // Find the same fragment
    let overridden_frag = fragments_with_override
        .iter()
        .find(|f| f.frag_idx_code == sutta_frag.frag_idx_code)
        .expect("Should find the same fragment");

    // The end_line should be different
    assert_ne!(
        overridden_frag.end_line, original_end_line,
        "end_line should be changed by the override"
    );
    assert_eq!(
        overridden_frag.end_line, override_end_line,
        "end_line should match the override value"
    );

    // The content should be shorter
    assert!(
        overridden_frag.content_xml.len() < original_content_len,
        "content_xml should be shorter with earlier boundary"
    );
}

/// Test that correction boundary overrides are applied during parsing
#[test]
fn test_boundary_override_applied() {
    let xml = r#"<?xml version="1.0"?>
<TEI.2>
<teiHeader></teiHeader>
<text>
<body>
<p rend="nikaya">Dīghanikāyo</p>
<div id="dn1" type="book">
<head rend="book">Sīlakkhandhavaggapāḷi</head>
<div id="dn1_1" type="sutta">
<head rend="chapter">1. Brahmajālasutta</head>
<p rend="bodytext" n="1">First paragraph content here.</p>
<p rend="bodytext" n="2">Second paragraph content here.</p>
<p rend="bodytext" n="3">Third paragraph content here.</p>
<p rend="bodytext" n="4">Fourth paragraph content here.</p>
</div>
</div>
</body>
</text>
</TEI.2>"#;

    let structure = detect_nikaya_structure(xml).unwrap();

    // Parse without overrides first to find the sutta fragment
    let fragments_baseline =
        parse_into_fragments(xml, &structure, "test.xml", &ParserOverrides::default(), false)
            .unwrap();

    let sutta_frag = fragments_baseline
        .iter()
        .find(|f| matches!(f.frag_type, FragmentType::Sutta))
        .expect("Should find a Sutta fragment");

    let original_end_line = sutta_frag.end_line;

    assert!(
        original_end_line > sutta_frag.start_line + 2,
        "Fragment must span enough lines for this test"
    );

    let checked_end_line = original_end_line - 1;

    // Create CorrectionFragmentOverrides
    let mut correction_overrides = CorrectionFragmentOverrides::new();
    let key = FragmentKey {
        cst_file: "test.xml".to_string(),
        frag_idx_code: sutta_frag.frag_idx_code.clone(),
    };
    correction_overrides.insert(
        key,
        CorrectionFragmentOverride {
            collapse: false,
            start_line: None,
            start_char: None,
            end_line: Some(checked_end_line),
            end_char: Some(0),
            sc_code: None,
            sc_sutta: None,
            cst_code: None,
            cst_vagga: None,
            cst_sutta: None,
            cst_paranum: None,
            frag_review: None,
            frag_type: None,
        },
    );

    let overrides = ParserOverrides {
        correction_overrides: Some(correction_overrides),
        inserted_fragments: None,
        pali_titles: None,
    };

    let fragments_with_override =
        parse_into_fragments(xml, &structure, "test.xml", &overrides, false).unwrap();

    // Find the overridden fragment
    let overridden_frag = fragments_with_override
        .iter()
        .find(|f| f.frag_idx_code == sutta_frag.frag_idx_code)
        .expect("Should find the same fragment");

    // The correction override value should be used
    assert_eq!(
        overridden_frag.end_line, checked_end_line,
        "Correction override should set the boundary"
    );
}

/// Test that fragment boundaries remain continuous when Header fragment is extended by checked override
///
/// This is a regression test for the bug where extending the Header fragment's end boundary
/// via checked override caused the next fragment (Sutta) to overlap with the Header, resulting
/// in XML reconstruction being larger than the original.
///
/// The fix ensures that when a Header fragment's boundary is adjusted, the next fragment
/// starts at the adjusted end position rather than the original position.
#[test]
fn test_header_fragment_boundary_continuity_with_override() {
    // Create XML with a clear header/body boundary
    let xml = r#"<?xml version="1.0"?>
<TEI.2>
<teiHeader></teiHeader>
<text>
<body>
<p rend="nikaya">Saṃyuttanikāyo</p>
<div id="sn1" type="book">
<head rend="book">Sagāthāvaggapāḷi</head>
<div id="sn1_1" type="samyutta">
<head rend="samyutta">1. Devatāsaṃyuttaṃ</head>
<div id="sn1_1_1" type="vagga">
<head rend="vagga">1. Naḷavaggo</head>
<p rend="subhead">1. Oghataraṇasuttaṃ</p>
<p rend="bodytext" n="1">Test content.</p>
</div>
</div>
</div>
</body>
</text>
</TEI.2>"#;

    let structure = detect_nikaya_structure(xml).unwrap();

    // Parse without overrides to get baseline boundaries
    let fragments_baseline =
        parse_into_fragments(xml, &structure, "test.xml", &ParserOverrides::default(), false)
            .unwrap();

    // Find the Header fragment
    let header_frag = fragments_baseline
        .iter()
        .find(|f| matches!(f.frag_type, FragmentType::Header))
        .expect("Should find a Header fragment");

    // Verify reconstruction of baseline fragments doesn't lose or duplicate bytes
    let baseline_reconstruction: String = fragments_baseline
        .iter()
        .map(|f| f.content_xml.as_str())
        .collect();
    assert_eq!(
        baseline_reconstruction.len(),
        xml.len(),
        "Baseline reconstruction should match original XML length"
    );

    // Now create a checked override that extends the Header fragment's end boundary
    // Move end boundary to include an extra line
    let extended_end_line = header_frag.end_line + 1;
    let extended_end_char = 0;

    let mut correction_overrides = CorrectionFragmentOverrides::new();
    let key = FragmentKey {
        cst_file: "test.xml".to_string(),
        frag_idx_code: header_frag.frag_idx_code.clone(),
    };
    correction_overrides.insert(
        key,
        CorrectionFragmentOverride {
            collapse: false,
            start_line: None,
            start_char: None,
            end_line: Some(extended_end_line),
            end_char: Some(extended_end_char),
            sc_code: None,
            sc_sutta: None,
            cst_code: None,
            cst_vagga: None,
            cst_sutta: None,
            cst_paranum: None,
            frag_review: None,
            frag_type: None,
        },
    );

    let overrides = ParserOverrides {
        correction_overrides: Some(correction_overrides),
        inserted_fragments: None,
        pali_titles: None,
    };

    let fragments_with_override =
        parse_into_fragments(xml, &structure, "test.xml", &overrides, false).unwrap();

    // Find the fragments after override
    let header_override = fragments_with_override
        .iter()
        .find(|f| matches!(f.frag_type, FragmentType::Header))
        .expect("Should find Header fragment after override");

    let sutta_override = fragments_with_override
        .iter()
        .find(|f| matches!(f.frag_type, FragmentType::Sutta))
        .expect("Should find Sutta fragment after override");

    // Verify the Header fragment was extended
    assert_eq!(
        header_override.end_line, extended_end_line,
        "Header fragment should be extended to the override end_line"
    );

    // KEY REGRESSION TEST: The Sutta fragment should start exactly where the
    // extended Header fragment ends (no gap, no overlap)
    assert_eq!(
        sutta_override.start_line, header_override.end_line,
        "Sutta should start on the same line where Header ends"
    );
    assert_eq!(
        sutta_override.start_char, header_override.end_char,
        "Sutta should start at the same char position where Header ends"
    );

    // Verify reconstruction still works (no bytes lost or duplicated)
    let override_reconstruction: String = fragments_with_override
        .iter()
        .map(|f| f.content_xml.as_str())
        .collect();

    assert_eq!(
        override_reconstruction.len(),
        xml.len(),
        "Reconstruction with override should still match original XML length (no overlap or gap)"
    );
    assert_eq!(
        override_reconstruction, xml,
        "Reconstruction with override should exactly match original XML"
    );
}

/// Test that fragment content is correctly extracted based on overridden boundaries
///
/// This verifies the content_xml field reflects the boundary change.
#[test]
fn test_boundary_override_content_extraction() {
    // Create XML where we can verify content extraction
    let xml = r#"<?xml version="1.0"?>
<TEI.2>
<teiHeader></teiHeader>
<text>
<body>
<p rend="nikaya">Dīghanikāyo</p>
<div id="dn1" type="book">
<head rend="book">Sīlakkhandhavaggapāḷi</head>
<div id="dn1_1" type="sutta">
<head rend="chapter">1. Brahmajālasutta</head>
<p rend="bodytext" n="1">AAAA</p>
<p rend="bodytext" n="2">BBBB</p>
<p rend="bodytext" n="3">CCCC</p>
</div>
</div>
</body>
</text>
</TEI.2>"#;

    let structure = detect_nikaya_structure(xml).unwrap();

    // Parse without overrides to get full content
    let fragments_baseline =
        parse_into_fragments(xml, &structure, "test.xml", &ParserOverrides::default(), false)
            .unwrap();

    let sutta_frag = fragments_baseline
        .iter()
        .find(|f| matches!(f.frag_type, FragmentType::Sutta))
        .expect("Should find a Sutta fragment");

    // Full content should contain all markers
    assert!(
        sutta_frag.content_xml.contains("AAAA"),
        "Full content should contain AAAA"
    );
    assert!(
        sutta_frag.content_xml.contains("CCCC"),
        "Full content should contain CCCC"
    );

    // Find line number of "BBBB" - we'll set end boundary just before it
    let lines: Vec<&str> = xml.lines().collect();
    let bbbb_line = lines
        .iter()
        .position(|line| line.contains("BBBB"))
        .map(|i| i + 1) // Convert to 1-indexed
        .expect("Should find BBBB line");

    // Create override with boundary that cuts off before BBBB
    let mut correction_overrides = CorrectionFragmentOverrides::new();
    let key = FragmentKey {
        cst_file: "test.xml".to_string(),
        frag_idx_code: sutta_frag.frag_idx_code.clone(),
    };
    correction_overrides.insert(
        key,
        CorrectionFragmentOverride {
            collapse: false,
            start_line: None,
            start_char: None,
            end_line: Some(bbbb_line),
            end_char: Some(0), // At start of BBBB line
            sc_code: None,
            sc_sutta: None,
            cst_code: None,
            cst_vagga: None,
            cst_sutta: None,
            cst_paranum: None,
            frag_review: None,
            frag_type: None,
        },
    );

    let overrides = ParserOverrides {
        correction_overrides: Some(correction_overrides),
        inserted_fragments: None,
        pali_titles: None,
    };

    let fragments_with_override =
        parse_into_fragments(xml, &structure, "test.xml", &overrides, false).unwrap();

    let overridden_frag = fragments_with_override
        .iter()
        .find(|f| f.frag_idx_code == sutta_frag.frag_idx_code)
        .expect("Should find the same fragment");

    // With the override, AAAA should be present but BBBB and CCCC should be cut off
    assert!(
        overridden_frag.content_xml.contains("AAAA"),
        "Overridden content should still contain AAAA"
    );
    assert!(
        !overridden_frag.content_xml.contains("BBBB"),
        "Overridden content should NOT contain BBBB (cut off by boundary)"
    );
    assert!(
        !overridden_frag.content_xml.contains("CCCC"),
        "Overridden content should NOT contain CCCC (cut off by boundary)"
    );
}
