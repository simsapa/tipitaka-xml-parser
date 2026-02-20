//! Test for SN sutta number ranges and vagga boundary detection
//!
//! Tests that:
//! 1. Sutta ranges like "2-11. Jātisuttādidasakaṃ" are detected as sutta boundaries
//! 2. `<p rend="title">` vagga boundaries close the current fragment in SN
//! 3. Fragment content patterns match expected structure

use tipitaka_xml_parser::nikaya_detector::detect_nikaya_structure;
use tipitaka_xml_parser::parse_into_fragments;
use tipitaka_xml_parser::types::{FragmentType, ParserOverrides};

/// Test that sutta number ranges (e.g., "2-11.") are detected as sutta boundaries
#[test]
fn test_sn_sutta_range_boundary() {
    // Read the actual s0302m.mul.xml file
    let xml_content = std::fs::read_to_string("tests/data/s0302m.mul.xml")
        .expect("Failed to read s0302m.mul.xml");

    // Detect structure
    let structure = detect_nikaya_structure(&xml_content)
        .expect("Failed to detect nikaya structure");

    // Parse into fragments
    let fragments = parse_into_fragments(
        &xml_content,
        &structure,
        "s0302m.mul.xml",
        &ParserOverrides::default(),
        false
    ).expect("Failed to parse fragments");

    // Find sutta fragments
    let sutta_fragments: Vec<_> = fragments.iter()
        .filter(|f| matches!(f.frag_type, FragmentType::Sutta))
        .collect();

    // Find fragment containing "1. Jarāmaraṇasuttaṃ" - should NOT also contain "2-11. Jātisuttādidasakaṃ"
    let jaramarana_frag = sutta_fragments.iter()
        .find(|f| f.content_xml.contains("1. Jarāmaraṇasuttaṃ"));

    assert!(jaramarana_frag.is_some(), "Should find fragment containing '1. Jarāmaraṇasuttaṃ'");
    let jaramarana_frag = jaramarana_frag.unwrap();

    // This fragment should NOT contain the next sutta "2-11. Jātisuttādidasakaṃ"
    // If it does, the range number wasn't recognized as a sutta boundary
    assert!(
        !jaramarana_frag.content_xml.contains("2-11. Jātisuttādidasakaṃ"),
        "Fragment with '1. Jarāmaraṇasuttaṃ' should NOT contain '2-11. Jātisuttādidasakaṃ'. \
         The range sutta number should create a fragment boundary."
    );

    println!("Fragment with '1. Jarāmaraṇasuttaṃ':");
    println!("  frag_idx_code: {}", jaramarana_frag.frag_idx_code);
    println!("  cst_code: {:?}", jaramarana_frag.cst_code);
    println!("  cst_vagga: {:?}", jaramarana_frag.cst_vagga);
    println!("  cst_sutta: {:?}", jaramarana_frag.cst_sutta);

    // Find the fragment containing "2-11. Jātisuttādidasakaṃ" - should be a separate fragment
    let jati_frag = sutta_fragments.iter()
        .find(|f| f.content_xml.contains("2-11. Jātisuttādidasakaṃ"));

    assert!(jati_frag.is_some(), "Should find a separate fragment containing '2-11. Jātisuttādidasakaṃ'");
    let jati_frag = jati_frag.unwrap();

    println!("\nFragment with '2-11. Jātisuttādidasakaṃ':");
    println!("  frag_idx_code: {}", jati_frag.frag_idx_code);
    println!("  cst_code: {:?}", jati_frag.cst_code);
    println!("  cst_vagga: {:?}", jati_frag.cst_vagga);
    println!("  cst_sutta: {:?}", jati_frag.cst_sutta);

    // The fragment indices should be consecutive, indicating they are separate fragments
    assert!(
        jati_frag.frag_idx_code > jaramarana_frag.frag_idx_code,
        "Fragment with '2-11. Jātisuttādidasakaṃ' should have a higher frag_idx_code than '1. Jarāmaraṇasuttaṃ'"
    );
}

/// Test that vagga boundaries (<p rend="title">) close the current fragment
#[test]
fn test_sn_vagga_title_closes_fragment() {
    // Read the actual s0302m.mul.xml file
    let xml_content = std::fs::read_to_string("tests/data/s0302m.mul.xml")
        .expect("Failed to read s0302m.mul.xml");

    // Detect structure
    let structure = detect_nikaya_structure(&xml_content)
        .expect("Failed to detect nikaya structure");

    // Parse into fragments
    let fragments = parse_into_fragments(
        &xml_content,
        &structure,
        "s0302m.mul.xml",
        &ParserOverrides::default(),
        false
    ).expect("Failed to parse fragments");

    // Find sutta fragments
    let sutta_fragments: Vec<_> = fragments.iter()
        .filter(|f| matches!(f.frag_type, FragmentType::Sutta))
        .collect();

    // Find fragment containing "2-11. Jātisuttādidasakaṃ" (last sutta in vagga 8)
    let jati_frag = sutta_fragments.iter()
        .find(|f| f.content_xml.contains("2-11. Jātisuttādidasakaṃ"));

    assert!(jati_frag.is_some(), "Should find fragment containing '2-11. Jātisuttādidasakaṃ'");
    let jati_frag = jati_frag.unwrap();

    println!("Fragment with '2-11. Jātisuttādidasakaṃ':");
    println!("  frag_idx_code: {}", jati_frag.frag_idx_code);
    println!("  end_line: {}", jati_frag.end_line);
    println!("  cst_vagga: {:?}", jati_frag.cst_vagga);
    println!("  contains '9. Antarapeyyālaṃ': {}", jati_frag.content_xml.contains("9. Antarapeyyālaṃ"));

    // This fragment should NOT contain the next vagga "9. Antarapeyyālaṃ"
    // If it does, the vagga boundary wasn't properly closing the fragment
    assert!(
        !jati_frag.content_xml.contains("9. Antarapeyyālaṃ"),
        "Fragment with '2-11. Jātisuttādidasakaṃ' should NOT contain '9. Antarapeyyālaṃ'. \
         The vagga title (<p rend=\"title\">) should close the fragment boundary."
    );

    // Find the fragment containing "1. Satthusuttaṃ" (first sutta of vagga 9)
    let satthu_frag = sutta_fragments.iter()
        .find(|f| f.content_xml.contains("1. Satthusuttaṃ"));

    assert!(satthu_frag.is_some(), "Should find fragment containing '1. Satthusuttaṃ'");
    let satthu_frag = satthu_frag.unwrap();

    println!("\nFragment with '1. Satthusuttaṃ':");
    println!("  frag_idx_code: {}", satthu_frag.frag_idx_code);
    println!("  start_line: {}", satthu_frag.start_line);
    println!("  cst_vagga: {:?}", satthu_frag.cst_vagga);
    println!("  cst_sutta: {:?}", satthu_frag.cst_sutta);
    println!("  contains '9. Antarapeyyālaṃ': {}", satthu_frag.content_xml.contains("9. Antarapeyyālaṃ"));

    // The fragment with Satthusutta should have cst_vagga = "9. Antarapeyyālaṃ"
    assert_eq!(
        satthu_frag.cst_vagga.as_deref(),
        Some("9. Antarapeyyālaṃ"),
        "Fragment with '1. Satthusuttaṃ' should have cst_vagga='9. Antarapeyyālaṃ'"
    );
}

/// Test that frag_idx_code with vagga 9 starts with proper content pattern
/// s0302m.mul.xml should have fragment containing:
/// <p rend="title">9. Antarapeyyālaṃ</p>
/// <p rend="subhead">1. Satthusuttaṃ</p>
/// <p rend="bodytext" n="73">
#[test]
fn test_sn_antarapeyyala_fragment_content() {
    let xml_content = std::fs::read_to_string("tests/data/s0302m.mul.xml")
        .expect("Failed to read s0302m.mul.xml");

    let structure = detect_nikaya_structure(&xml_content)
        .expect("Failed to detect nikaya structure");

    let fragments = parse_into_fragments(
        &xml_content,
        &structure,
        "s0302m.mul.xml",
        &ParserOverrides::default(),
        false
    ).expect("Failed to parse fragments");

    // Find fragment containing "1. Satthusuttaṃ" (vagga 9)
    let satthu_frag = fragments.iter()
        .find(|f| f.content_xml.contains("1. Satthusuttaṃ"));

    assert!(satthu_frag.is_some(), "Should find fragment containing '1. Satthusuttaṃ'");
    let frag = satthu_frag.unwrap();

    // Check that fragment has correct cst_vagga
    assert_eq!(
        frag.cst_vagga.as_deref(),
        Some("9. Antarapeyyālaṃ"),
        "Fragment should have cst_vagga='9. Antarapeyyālaṃ'"
    );

    // Check contains the vagga title
    assert!(
        frag.content_xml.contains("<p rend=\"title\">9. Antarapeyyālaṃ</p>"),
        "Fragment should contain '<p rend=\"title\">9. Antarapeyyālaṃ</p>'"
    );

    // Check contains the sutta subhead
    assert!(
        frag.content_xml.contains("<p rend=\"subhead\">1. Satthusuttaṃ</p>"),
        "Fragment should contain '<p rend=\"subhead\">1. Satthusuttaṃ</p>'"
    );

    // Check contains bodytext with n="73"
    assert!(
        frag.content_xml.contains("<p rend=\"bodytext\" n=\"73\">"),
        "Fragment should contain '<p rend=\"bodytext\" n=\"73\">'"
    );
}

/// Test that frag_idx_code 10.0 (s0301m.mul.xml) starts with proper content pattern
/// s0301m.mul.xml should have fragment containing:
/// <p rend="subhead">10. Araññasuttaṃ</p>
/// <p rend="bodytext" n="10"> ...
/// <p rend="gathalast">Araññe dasamo vutto, vaggo tena pavuccati.</p>
#[test]
fn test_sn_aranan_sutta_fragment_content() {
    let xml_content = std::fs::read_to_string("tests/data/s0301m.mul.xml")
        .expect("Failed to read s0301m.mul.xml");

    let structure = detect_nikaya_structure(&xml_content)
        .expect("Failed to detect nikaya structure");

    let fragments = parse_into_fragments(
        &xml_content,
        &structure,
        "s0301m.mul.xml",
        &ParserOverrides::default(),
        false
    ).expect("Failed to parse fragments");

    // Find fragment containing "10. Araññasuttaṃ"
    let aranan_frag = fragments.iter()
        .find(|f| f.content_xml.contains("10. Araññasuttaṃ"));

    assert!(aranan_frag.is_some(), "Should find fragment containing '10. Araññasuttaṃ'");
    let frag = aranan_frag.unwrap();

    // Check contains the sutta subhead
    assert!(
        frag.content_xml.contains("<p rend=\"subhead\">10. Araññasuttaṃ</p>"),
        "Fragment should contain '<p rend=\"subhead\">10. Araññasuttaṃ</p>'"
    );

    // Check contains bodytext with n="10"
    assert!(
        frag.content_xml.contains("<p rend=\"bodytext\" n=\"10\">"),
        "Fragment should contain '<p rend=\"bodytext\" n=\"10\">'"
    );

    // Check contains the closing gathalast
    assert!(
        frag.content_xml.contains("<p rend=\"gathalast\">Araññe dasamo vutto, vaggo tena pavuccati.</p>"),
        "Fragment should contain '<p rend=\"gathalast\">Araññe dasamo vutto, vaggo tena pavuccati.</p>'"
    );
}

/// Test that frag_idx_code 11.0 (s0301m.mul.xml) has proper content pattern
/// s0301m.mul.xml should have fragment containing:
/// <p rend="title">2. Nandanavaggo</p>
/// <p rend="subhead">1. Nandanasuttaṃ</p>
/// <p rend="bodytext" n="11">
#[test]
fn test_sn_nandana_vagga_fragment_content() {
    let xml_content = std::fs::read_to_string("tests/data/s0301m.mul.xml")
        .expect("Failed to read s0301m.mul.xml");

    let structure = detect_nikaya_structure(&xml_content)
        .expect("Failed to detect nikaya structure");

    let fragments = parse_into_fragments(
        &xml_content,
        &structure,
        "s0301m.mul.xml",
        &ParserOverrides::default(),
        false
    ).expect("Failed to parse fragments");

    // Find fragment containing "1. Nandanasuttaṃ" (vagga 2)
    let nandana_frag = fragments.iter()
        .find(|f| f.content_xml.contains("1. Nandanasuttaṃ"));

    assert!(nandana_frag.is_some(), "Should find fragment containing '1. Nandanasuttaṃ'");
    let frag = nandana_frag.unwrap();

    // Check that fragment has correct cst_vagga
    assert_eq!(
        frag.cst_vagga.as_deref(),
        Some("2. Nandanavaggo"),
        "Fragment should have cst_vagga='2. Nandanavaggo'"
    );

    // Check contains the vagga title
    assert!(
        frag.content_xml.contains("<p rend=\"title\">2. Nandanavaggo</p>"),
        "Fragment should contain '<p rend=\"title\">2. Nandanavaggo</p>'"
    );

    // Check contains the sutta subhead
    assert!(
        frag.content_xml.contains("<p rend=\"subhead\">1. Nandanasuttaṃ</p>"),
        "Fragment should contain '<p rend=\"subhead\">1. Nandanasuttaṃ</p>'"
    );

    // Check contains bodytext with n="11"
    assert!(
        frag.content_xml.contains("<p rend=\"bodytext\" n=\"11\">"),
        "Fragment should contain '<p rend=\"bodytext\" n=\"11\">'"
    );
}
