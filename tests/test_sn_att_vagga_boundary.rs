//! Test for SN Atthakatha vagga boundary bug
//!
//! Regression test for the issue where frag_idx 10 in s0301a.att.xml incorrectly
//! included the next vagga's title and had wrong cst_code/sc_code.
//!
//! Issue: cst_file s0301a.att.xml frag_idx 10 should get:
//! - cst_code: sn1.1.1.10
//! - sc_code: sn1.10
//!
//! But it was getting:
//! - cst_code: sn1.1.2.10 (wrong vagga number)
//! - sc_code: sn2.10 (wrong)
//!
//! The fragment was incorrectly including "<p rend="title">2. Nandanavaggo</p>" at the end.

use std::fs;
use tipitaka_xml_parser::{
    nikaya_detector::detect_nikaya_structure,
    parsers::samyutta_nikaya_atthakatha::parse_into_fragments,
    types::ParserOverrides,
};

#[test]
fn test_s0301a_att_frag_idx_10_vagga_boundary() {
    // Load the actual XML file
    let xml_path = "tests/data/s0301a.att.xml";
    let xml_content = fs::read_to_string(xml_path)
        .expect("Failed to read s0301a.att.xml");

    // Detect nikaya structure
    let structure = detect_nikaya_structure(&xml_content)
        .expect("Should detect SN structure");

    assert_eq!(structure.nikaya, "samyutta");

    // Parse into fragments with SC field population
    let fragments = parse_into_fragments(&xml_content, &structure, "s0301a.att.xml", &ParserOverrides::default(), true)
        .expect("Should parse fragments");

    // Get frag_idx 10 - it should be the last sutta in vagga 1 (Naḷavaggo)
    let frag_10 = fragments.get(10)
        .expect("Should have frag_idx 10");

    // frag_idx 10 should be in vagga 1 (Naḷavaggo), NOT vagga 2
    assert!(frag_10.cst_vagga.as_ref().map(|v| v.contains("Naḷavaggo")).unwrap_or(false),
        "frag_idx 10 should be in Naḷavaggo (vagga 1), but got cst_vagga: {:?}",
        frag_10.cst_vagga);

    // The cst_code should be sn1.1.1.10 (book 1, samyutta 1, vagga 1, sutta 10)
    // NOT sn1.1.2.10 (which would mean vagga 2)
    assert_eq!(frag_10.cst_code.as_deref(), Some("sn1.1.1.10"),
        "frag_idx 10 should have cst_code sn1.1.1.10, but got {:?}",
        frag_10.cst_code);

    // The sc_code should be sn1.10, NOT sn2.10
    assert_eq!(frag_10.sc_code.as_deref(), Some("sn1.10"),
        "frag_idx 10 should have sc_code sn1.10, but got {:?}",
        frag_10.sc_code);

    // frag_idx 10 should NOT contain "2. Nandanavaggo" at the end
    assert!(!frag_10.content_xml.contains("Nandanavaggo"),
        "frag_idx 10 should NOT contain 'Nandanavaggo' (next vagga's title)");
}

#[test]
fn test_s0301a_att_vagga_2_first_sutta() {
    // Load the actual XML file
    let xml_path = "tests/data/s0301a.att.xml";
    let xml_content = fs::read_to_string(xml_path)
        .expect("Failed to read s0301a.att.xml");

    // Detect nikaya structure
    let structure = detect_nikaya_structure(&xml_content)
        .expect("Should detect SN structure");

    // Parse into fragments with SC field population
    let fragments = parse_into_fragments(&xml_content, &structure, "s0301a.att.xml", &ParserOverrides::default(), true)
        .expect("Should parse fragments");

    // Find the first sutta in vagga 2 (Nandanavaggo)
    // It should contain the vagga title at the start
    let vagga_2_first_sutta = fragments.iter()
        .find(|f| f.cst_code.as_deref() == Some("sn1.1.2.1"))
        .expect("Should find first sutta in vagga 2");

    // It should be in vagga 2
    assert!(vagga_2_first_sutta.cst_vagga.as_ref().map(|v| v.contains("Nandanavaggo")).unwrap_or(false),
        "First sutta in vagga 2 should have cst_vagga '2. Nandanavaggo', but got {:?}",
        vagga_2_first_sutta.cst_vagga);

    // It should contain the vagga title at the start
    assert!(vagga_2_first_sutta.content_xml.contains("Nandanavaggo"),
        "First sutta in vagga 2 should contain 'Nandanavaggo' (vagga title at the start)");

    // It should also contain the sutta subhead
    assert!(vagga_2_first_sutta.content_xml.contains("Nandanasuttavaṇṇanā"),
        "First sutta in vagga 2 should contain 'Nandanasuttavaṇṇanā'");

    // sc_code should be sn1.11
    assert_eq!(vagga_2_first_sutta.sc_code.as_deref(), Some("sn1.11"),
        "First sutta in vagga 2 should have sc_code sn1.11, but got {:?}",
        vagga_2_first_sutta.sc_code);
}
