//! Test for SN Tika (sub-commentary) vagga boundary bug
//!
//! Regression test for the issue where fragments incorrectly
//! included the next vagga's title and had wrong cst_code/sc_code.
//!
//! Issue: The last sutta in vagga 1 (Naḷavaggo) should get:
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
    parsers::samyutta_nikaya_tika::parse_into_fragments,
    types::ParserOverrides,
};

#[test]
fn test_s0301t_tik_last_sutta_vagga_1() {
    // Load the actual XML file
    let xml_path = "tests/data/s0301t.tik.xml";
    let xml_content = fs::read_to_string(xml_path)
        .expect("Failed to read s0301t.tik.xml");

    // Detect nikaya structure
    let structure = detect_nikaya_structure(&xml_content)
        .expect("Should detect SN structure");

    assert_eq!(structure.nikaya, "samyutta");

    // Parse into fragments with SC field population
    let fragments = parse_into_fragments(&xml_content, &structure, "s0301t.tik.xml", &ParserOverrides::default(), true)
        .expect("Should parse fragments");

    // Find the last sutta in vagga 1 (Naḷavaggo) by cst_code
    let last_sutta_vagga_1 = fragments.iter()
        .find(|f| f.cst_code.as_deref() == Some("sn1.1.1.10"))
        .expect("Should find last sutta in vagga 1 (sn1.1.1.10)");

    // It should be in vagga 1 (Naḷavaggo), NOT vagga 2
    assert!(last_sutta_vagga_1.cst_vagga.as_ref().map(|v| v.contains("Naḷavaggo")).unwrap_or(false),
        "Last sutta in vagga 1 should be in Naḷavaggo, but got cst_vagga: {:?}",
        last_sutta_vagga_1.cst_vagga);

    // The sc_code should be sn1.10, NOT sn2.10
    assert_eq!(last_sutta_vagga_1.sc_code.as_deref(), Some("sn1.10"),
        "Last sutta in vagga 1 should have sc_code sn1.10, but got {:?}",
        last_sutta_vagga_1.sc_code);

    // It should NOT contain "2. Nandanavaggo" at the end
    assert!(!last_sutta_vagga_1.content_xml.contains("Nandanavaggo"),
        "Last sutta in vagga 1 should NOT contain 'Nandanavaggo' (next vagga's title)");
}

#[test]
fn test_s0301t_tik_vagga_2_first_sutta() {
    // Load the actual XML file
    let xml_path = "tests/data/s0301t.tik.xml";
    let xml_content = fs::read_to_string(xml_path)
        .expect("Failed to read s0301t.tik.xml");

    // Detect nikaya structure
    let structure = detect_nikaya_structure(&xml_content)
        .expect("Should detect SN structure");

    // Parse into fragments with SC field population
    let fragments = parse_into_fragments(&xml_content, &structure, "s0301t.tik.xml", &ParserOverrides::default(), true)
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

#[test]
fn test_s0301t_tik_first_sutta_vagga_3() {
    // Test for titles without space after dot like "1.Sattisuttavaṇṇanā"
    // The first sutta in vagga 3 should have cst_code sn1.1.3.1 (not sn1.1.3.0)
    let xml_path = "tests/data/s0301t.tik.xml";
    let xml_content = fs::read_to_string(xml_path)
        .expect("Failed to read s0301t.tik.xml");

    let structure = detect_nikaya_structure(&xml_content)
        .expect("Should detect SN structure");

    let fragments = parse_into_fragments(&xml_content, &structure, "s0301t.tik.xml", &ParserOverrides::default(), true)
        .expect("Should parse fragments");

    // Find the first sutta in vagga 3 (Sattivaggo)
    let first_sutta_vagga_3 = fragments.iter()
        .find(|f| f.cst_code.as_deref() == Some("sn1.1.3.1"))
        .expect("Should find first sutta in vagga 3 (sn1.1.3.1)");

    // It should be in vagga 3 (Sattivaggo)
    assert!(first_sutta_vagga_3.cst_vagga.as_ref().map(|v| v.contains("Sattivaggo")).unwrap_or(false),
        "First sutta in vagga 3 should be in Sattivaggo, but got cst_vagga: {:?}",
        first_sutta_vagga_3.cst_vagga);

    // cst_sutta should contain the title (even if no space after dot)
    assert!(first_sutta_vagga_3.cst_sutta.as_ref().map(|s| s.contains("Sattisuttavaṇṇanā")).unwrap_or(false),
        "First sutta in vagga 3 should have cst_sutta containing 'Sattisuttavaṇṇanā', but got {:?}",
        first_sutta_vagga_3.cst_sutta);
}
