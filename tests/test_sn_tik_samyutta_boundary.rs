//! Test for SN Tika samyutta boundary issue
//!
//! Issue: When a new samyutta starts (e.g., sn1_3 "3. Kosalasaṃyuttaṃ"),
//! the samyutta header (div + head) should be included with the FIRST sutta
//! of that samyutta, not as a separate fragment.
//!
//! Fragment 113 currently contains only:
//!   <div id="sn1_3" n="sn1_3" type="samyutta">
//!   <head rend="chapter">3. Kosalasaṃyuttaṃ</head>
//!
//! But this should be included with the next fragment which has:
//!   <p rend="title">1. Paṭhamavaggo</p>
//!   <p rend="subhead">1. Daharasuttavaṇṇanā</p>
//!   <p rend="bodytext" n="112">...

use std::fs;
use tipitaka_xml_parser::{
    nikaya_detector::detect_nikaya_structure,
    parsers::samyutta_nikaya_tika::parse_into_fragments,
    types::ParserOverrides,
};

#[test]
fn test_s0301t_tik_samyutta_boundary() {
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

    // Find the first sutta of samyutta 3 (Kosalasaṃyuttaṃ)
    // It should contain the samyutta header + vagga title + sutta content
    let first_sutta_samyutta_3 = fragments.iter()
        .find(|f| f.content_xml.contains("Daharasuttavaṇṇanā"))
        .expect("Should find first sutta of samyutta 3");

    println!("First sutta of samyutta 3 (Kosalasaṃyuttaṃ):");
    println!("  frag_idx: {}", first_sutta_samyutta_3.frag_idx);
    println!("  cst_code: {:?}", first_sutta_samyutta_3.cst_code);
    println!("  sc_code: {:?}", first_sutta_samyutta_3.sc_code);
    println!("  cst_vagga: {:?}", first_sutta_samyutta_3.cst_vagga);
    println!("  start_line: {}", first_sutta_samyutta_3.start_line);
    println!("  content preview: {}", &first_sutta_samyutta_3.content_xml[..600.min(first_sutta_samyutta_3.content_xml.len())]);
    println!();

    // The first sutta of samyutta 3 SHOULD contain the samyutta header
    assert!(first_sutta_samyutta_3.content_xml.contains("Kosalasaṃyuttaṃ"),
        "First sutta of samyutta 3 should contain the samyutta header 'Kosalasaṃyuttaṃ'.\n\
         Content:\n{}", first_sutta_samyutta_3.content_xml);

    // It should also contain the vagga title
    assert!(first_sutta_samyutta_3.content_xml.contains("Paṭhamavaggo"),
        "First sutta of samyutta 3 should contain the vagga title 'Paṭhamavaggo'");

    // It should also contain the sutta subhead
    assert!(first_sutta_samyutta_3.content_xml.contains("Daharasuttavaṇṇanā"),
        "First sutta of samyutta 3 should contain the sutta subhead 'Daharasuttavaṇṇanā'");

    // Check there's NO separate fragment containing ONLY the samyutta header
    // (without actual sutta content)
    for frag in &fragments {
        if frag.content_xml.contains("Kosalasaṃyuttaṃ") &&
           frag.content_xml.contains("type=\"samyutta\"") {
            // This fragment has the samyutta header
            // It should ALSO contain actual sutta content
            let has_sutta_content = frag.content_xml.contains("rend=\"subhead\"") ||
                                    frag.content_xml.contains("rend=\"bodytext\"");
            assert!(has_sutta_content,
                "Fragment {} with samyutta header 'Kosalasaṃyuttaṃ' should also contain sutta content.\n\
                 Content:\n{}", frag.frag_idx, frag.content_xml);
        }
    }
}

#[test]
fn test_s0301t_tik_no_samyutta_only_fragments() {
    // Load the actual XML file
    let xml_path = "tests/data/s0301t.tik.xml";
    let xml_content = fs::read_to_string(xml_path)
        .expect("Failed to read s0301t.tik.xml");

    let structure = detect_nikaya_structure(&xml_content)
        .expect("Should detect SN structure");

    let fragments = parse_into_fragments(&xml_content, &structure, "s0301t.tik.xml", &ParserOverrides::default(), true)
        .expect("Should parse fragments");

    // Check that no fragment contains ONLY a samyutta header (no sutta content)
    for frag in &fragments {
        if frag.content_xml.contains("type=\"samyutta\"") {
            let has_sutta_content = frag.content_xml.contains("rend=\"subhead\"") ||
                                    frag.content_xml.contains("rend=\"bodytext\"");
            if !has_sutta_content {
                println!("Fragment {} has samyutta but no sutta content:", frag.frag_idx);
                println!("{}", frag.content_xml);
            }
            assert!(has_sutta_content,
                "Fragment {} contains samyutta header but no sutta content - this is a bug!\n\
                 Content:\n{}", frag.frag_idx, frag.content_xml);
        }
    }
}
