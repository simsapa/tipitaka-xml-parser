//! Test for SN Atthakatha samyutta boundary issue
//!
//! Issue: When a new samyutta starts (e.g., sn1_2 "2. Devaputtasaṃyuttaṃ"),
//! the samyutta header (div + head) should be included with the FIRST sutta
//! of that samyutta, not as a separate fragment.
//!
//! Fragment 76 currently contains:
//!   <div id="sn1_2" n="sn1_2" type="samyutta">
//!   <head rend="chapter">2. Devaputtasaṃyuttaṃ</head>
//!
//! But this should be included with fragment 77 which has:
//!   <p rend="title">1. Paṭhamavaggo</p>
//!   <p rend="subhead">1. Paṭhamakassapasuttavaṇṇanā</p>
//!   <p rend="bodytext" n="82">...

use std::fs;
use tipitaka_xml_parser::{
    nikaya_detector::detect_nikaya_structure,
    parsers::samyutta_nikaya_commentary::parse_into_fragments,
    types::ParserOverrides,
};

#[test]
fn test_s0301a_att_samyutta_boundary() {
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

    // Find the first sutta of samyutta 2 (Devaputtasaṃyuttaṃ)
    // It should be sn1.1.2.1.1 (book 1, samyutta 2, vagga 1, sutta 1)
    // With sc_code sn2.1
    let first_sutta_samyutta_2 = fragments.iter()
        .find(|f| f.sc_code.as_deref() == Some("sn2.1"))
        .expect("Should find first sutta of samyutta 2 (sc_code sn2.1)");

    println!("First sutta of samyutta 2:");
    println!("  frag_idx_code: {}", first_sutta_samyutta_2.frag_idx_code);
    println!("  cst_code: {:?}", first_sutta_samyutta_2.cst_code);
    println!("  sc_code: {:?}", first_sutta_samyutta_2.sc_code);
    println!("  cst_vagga: {:?}", first_sutta_samyutta_2.cst_vagga);
    println!("  start_line: {}", first_sutta_samyutta_2.start_line);
    println!("  content preview: {}", &first_sutta_samyutta_2.content_xml[..500.min(first_sutta_samyutta_2.content_xml.len())]);
    println!();

    // The first sutta of samyutta 2 SHOULD contain the samyutta header
    assert!(first_sutta_samyutta_2.content_xml.contains("Devaputtasaṃyuttaṃ"),
        "First sutta of samyutta 2 should contain the samyutta header 'Devaputtasaṃyuttaṃ'.\n\
         Content:\n{}", first_sutta_samyutta_2.content_xml);

    // It should also contain the vagga title
    assert!(first_sutta_samyutta_2.content_xml.contains("Paṭhamavaggo"),
        "First sutta of samyutta 2 should contain the vagga title 'Paṭhamavaggo'");

    // It should also contain the sutta subhead
    assert!(first_sutta_samyutta_2.content_xml.contains("Paṭhamakassapasuttavaṇṇanā"),
        "First sutta of samyutta 2 should contain the sutta subhead 'Paṭhamakassapasuttavaṇṇanā'");

    // Check there's NO separate fragment containing ONLY the samyutta header
    // (without actual sutta content)
    for frag in &fragments {
        if frag.content_xml.contains("Devaputtasaṃyuttaṃ") &&
           frag.content_xml.contains("type=\"samyutta\"") {
            // This fragment has the samyutta header
            // It should ALSO contain actual sutta content
            let has_sutta_content = frag.content_xml.contains("rend=\"subhead\"") ||
                                    frag.content_xml.contains("rend=\"bodytext\"");
            assert!(has_sutta_content,
                "Fragment {} with samyutta header should also contain sutta content.\n\
                 Content:\n{}", frag.frag_idx_code, frag.content_xml);
        }
    }
}

#[test]
fn test_s0301a_att_frag_76_content() {
    // Specifically check what fragment 76 contains
    let xml_path = "tests/data/s0301a.att.xml";
    let xml_content = fs::read_to_string(xml_path)
        .expect("Failed to read s0301a.att.xml");

    let structure = detect_nikaya_structure(&xml_content)
        .expect("Should detect SN structure");

    let fragments = parse_into_fragments(&xml_content, &structure, "s0301a.att.xml", &ParserOverrides::default(), true)
        .expect("Should parse fragments");

    let frag_76 = fragments.get(76)
        .expect("Should have frag_idx_code 76.0");

    println!("Fragment 76:");
    println!("  cst_code: {:?}", frag_76.cst_code);
    println!("  sc_code: {:?}", frag_76.sc_code);
    println!("  cst_vagga: {:?}", frag_76.cst_vagga);
    println!("  start_line: {}", frag_76.start_line);
    println!("  end_line: {}", frag_76.end_line);
    println!("  content:\n{}", frag_76.content_xml);

    // Fragment 76 should NOT be a fragment containing ONLY the samyutta header
    // without actual sutta content
    if frag_76.content_xml.contains("Devaputtasaṃyuttaṃ") {
        // If it contains the samyutta header, it should also have sutta content
        let has_sutta_content = frag_76.content_xml.contains("rend=\"subhead\"") ||
                                frag_76.content_xml.contains("rend=\"bodytext\"");
        assert!(has_sutta_content,
            "Fragment 76 contains samyutta header but no sutta content - this is the bug!");
    }
}
