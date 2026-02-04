//! Integration tests for CheckedFragmentOverrides
//!
//! Tests that CheckedFragmentOverrides take precedence over FragmentAdjustments
//! and that SC code parsing and propagation work correctly.

use tipitaka_xml_parser::nikaya_detector::detect_nikaya_structure;
use tipitaka_xml_parser::parse_into_fragments;
use tipitaka_xml_parser::types::{
    CheckedFragmentOverride, CheckedFragmentOverrides, FragmentKey, FragmentType, ParserOverrides,
};

/// Test that CheckedFragmentOverrides take precedence over FragmentAdjustments
#[test]
fn test_checked_overrides_precedence() {
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
    let mut checked_overrides = CheckedFragmentOverrides::new();
    let key = FragmentKey {
        cst_file: "test.xml".to_string(),
        frag_idx: sutta_frag.frag_idx,
    };
    checked_overrides.insert(
        key,
        CheckedFragmentOverride {
            end_line: None,
            end_char: None,
            sc_code: Some("dn1.override".to_string()),
            sc_sutta: Some("Override Sutta Title".to_string()),
        },
    );

    let overrides = ParserOverrides {
        adjustments: None,
        checked_overrides: Some(checked_overrides),
    };

    let fragments_with_override =
        parse_into_fragments(xml, &structure, "test.xml", &overrides, false).unwrap();

    // Find the same fragment
    let overridden_frag = fragments_with_override
        .iter()
        .find(|f| f.frag_idx == sutta_frag.frag_idx)
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
fn test_extract_checked_overrides() {
    use tempfile::NamedTempFile;
    use tipitaka_xml_parser::fragment_exporter::{
        export_fragments_to_db, extract_checked_overrides, restore_frag_review_status,
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

    // Update frag_idx 1 to be "checked"
    diesel::update(
        xml_fragments::table
            .filter(xml_fragments::cst_file.eq("test.xml"))
            .filter(xml_fragments::frag_idx.eq(1)),
    )
    .set((
        xml_fragments::frag_review.eq("checked"),
        xml_fragments::sc_code.eq("dn1.test"),
    ))
    .execute(&mut conn)
    .unwrap();

    // Now extract checked overrides
    let (overrides, review_status) = extract_checked_overrides(db_path, "test.xml").unwrap();

    assert_eq!(overrides.len(), 1, "Should have 1 checked override");
    assert_eq!(
        review_status.len(),
        1,
        "Should have 1 frag_review status to restore"
    );

    let key = FragmentKey {
        cst_file: "test.xml".to_string(),
        frag_idx: 1,
    };
    let override_data = overrides.get(&key).expect("Should find override for frag_idx 1");
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
            .filter(xml_fragments::frag_idx.eq(1)),
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
        .filter(xml_fragments::frag_idx.eq(1))
        .select(xml_fragments::frag_review)
        .first(&mut conn)
        .unwrap();

    assert_eq!(
        frag_review.as_deref(),
        Some("checked"),
        "frag_review should be restored"
    );
}
