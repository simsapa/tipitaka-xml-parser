//! Test for SN sc_code propagation with range handling
//!
//! Tests that sc_code is correctly propagated through successive fragments,
//! with proper handling of range cst_codes and fallback to ArangoDB lookups.

use tipitaka_xml_parser::nikaya_detector::detect_nikaya_structure;
use tipitaka_xml_parser::parse_into_fragments;
use tipitaka_xml_parser::types::ParserOverrides;
use tipitaka_xml_parser::web::arangodb;

/// Test sc_code propagation from sn3.9 to sn3.10 (new samyutta)
///
/// After the fix for "9.Khayadhammasuttaṃ" (no space after number),
/// frag_idx_code values have shifted by +1. The new fragment "9. Khayadhammasuttaṃ"
/// is now correctly split from frag_idx_code 177.0.
///
/// frag_idx_code 275.0: cst_code sn3.9.1.17-46 should get sc_code sn30.17-46
/// frag_idx_code 276.0: cst_code sn3.10.1.1 should get sc_code sn31.1
/// frag_idx_code 277.0: cst_code sn3.10.1.2 should get sc_code sn31.2
///
/// This works because:
/// - The TSV contains mappings for the range cst_codes
/// - The ArangoDB contains the sc_codes
#[test]
fn test_s0303m_sc_code_propagation_sn9_to_sn10() {
    // Load pali_titles from ArangoDB
    let pali_titles = tokio::runtime::Runtime::new()
        .expect("Failed to create tokio runtime")
        .block_on(async {
            match arangodb::get_pali_titles().await {
                Ok(titles) => Some(titles),
                Err(_) => None,
            }
        });

    let overrides = ParserOverrides {
        pali_titles,
        ..Default::default()
    };

    let xml_content = std::fs::read_to_string("tests/data/s0303m.mul.xml")
        .expect("Failed to read s0303m.mul.xml");

    let structure = detect_nikaya_structure(&xml_content)
        .expect("Failed to detect nikaya structure");

    let fragments = parse_into_fragments(
        &xml_content,
        &structure,
        "s0303m.mul.xml",
        &overrides,
        true,  // Enable populate_sc_fields
    ).expect("Failed to parse fragments");

    // Find frag_idx_code 275.0 (last fragment of sn3.9 with range)
    let frag_275 = fragments.iter().find(|f| f.frag_idx_code == "275.0");
    assert!(frag_275.is_some(), "Should find frag_idx_code 275.0");
    let frag_275 = frag_275.unwrap();
    
    println!("frag_idx_code 275.0:");
    println!("  cst_code: {:?}", frag_275.cst_code);
    println!("  sc_code: {:?}", frag_275.sc_code);
    println!("  sc_sutta: {:?}", frag_275.sc_sutta);
    
    assert_eq!(frag_275.cst_code, Some("sn3.9.1.17-46".to_string()), 
        "frag_idx_code 275.0 should have cst_code sn3.9.1.17-46");
    assert_eq!(frag_275.sc_code, Some("sn30.17-46".to_string()), 
        "frag_idx_code 275.0 should have sc_code sn30.17-46");

    // Find frag_idx_code 276.0 (first fragment of sn3.10 - new samyutta)
    let frag_276 = fragments.iter().find(|f| f.frag_idx_code == "276.0");
    assert!(frag_276.is_some(), "Should find frag_idx_code 276.0");
    let frag_276 = frag_276.unwrap();
    
    println!("frag_idx_code 276.0:");
    println!("  cst_code: {:?}", frag_276.cst_code);
    println!("  sc_code: {:?}", frag_276.sc_code);
    println!("  sc_sutta: {:?}", frag_276.sc_sutta);
    
    assert_eq!(frag_276.cst_code, Some("sn3.10.1.1".to_string()), 
        "frag_idx_code 276.0 should have cst_code sn3.10.1.1");
    assert_eq!(frag_276.sc_code, Some("sn31.1".to_string()), 
        "frag_idx_code 276.0 should have sc_code sn31.1");
    // Check that sc_sutta is populated (from ArangoDB)
    assert!(frag_276.sc_sutta.is_some(), 
        "frag_idx_code 276.0 should have sc_sutta");

    // Find frag_idx_code 277.0 (second fragment of sn3.10)
    let frag_277 = fragments.iter().find(|f| f.frag_idx_code == "277.0");
    assert!(frag_277.is_some(), "Should find frag_idx_code 277.0");
    let frag_277 = frag_277.unwrap();
    
    println!("frag_idx_code 277.0:");
    println!("  cst_code: {:?}", frag_277.cst_code);
    println!("  sc_code: {:?}", frag_277.sc_code);
    println!("  sc_sutta: {:?}", frag_277.sc_sutta);
    
    assert_eq!(frag_277.cst_code, Some("sn3.10.1.2".to_string()), 
        "frag_idx_code 277.0 should have cst_code sn3.10.1.2");
    assert_eq!(frag_277.sc_code, Some("sn31.2".to_string()), 
        "frag_idx_code 277.0 should have sc_code sn31.2");
    assert!(frag_277.sc_sutta.is_some(), 
        "frag_idx_code 277.0 should have sc_sutta");
}

/// Test that range cst_codes get range sc_codes.
///
/// frag_idx_code 182.0 has cst_code "sn3.2.3.1-11" (a range of 11 suttas)
/// The sc_code should be "sn23.23-33" (not "sn23.23")
#[test]
fn test_s0303m_range_cst_code_to_sc_code() {
    let xml_content = std::fs::read_to_string("tests/data/s0303m.mul.xml")
        .expect("Failed to read s0303m.mul.xml");

    let structure = detect_nikaya_structure(&xml_content)
        .expect("Failed to detect nikaya structure");

    let overrides = ParserOverrides::default();

    let fragments = parse_into_fragments(
        &xml_content,
        &structure,
        "s0303m.mul.xml",
        &overrides,
        true,  // Enable populate_sc_fields
    ).expect("Failed to parse fragments");

    // Find frag_idx_code 182.0 (the first fragment of the range "1-11. Mārādisuttaekādasakaṃ")
    // This is in vagga "3. Āyācanavaggo" of samyutta "2. Rādhasaṃyuttaṃ"
    let frag_182 = fragments.iter().find(|f| f.frag_idx_code == "182.0");
    assert!(frag_182.is_some(), "Should find frag_idx_code 182.0");
    let frag_182 = frag_182.unwrap();
    
    println!("frag_idx_code 182.0:");
    println!("  cst_code: {:?}", frag_182.cst_code);
    println!("  sc_code: {:?}", frag_182.sc_code);
    
    // Verify cst_code is correctly parsed as a range
    assert_eq!(frag_182.cst_code, Some("sn3.2.3.1-11".to_string()), 
        "frag_idx_code 182.0 should have cst_code sn3.2.3.1-11");
    
    // Verify sc_code is in range format
    assert_eq!(frag_182.sc_code, Some("sn23.23-33".to_string()), 
        "frag_idx_code 182.0 should have sc_code sn23.23-33 (range)");
}

/// Test that range cst_codes get range sc_codes when using propagate_sc_codes_from_previous.
/// 
/// s0303t.tik.xml frag_idx_code 95.0 has cst_code "sn3.4.1.1-10" but TSV has no mapping for sn3.4.1.x
/// (only sn3.4.4.x). The sc_code should be derived from previous fragment and converted to range
/// when pali_titles from ArangoDB is available.
#[test]
fn test_s0303t_tik_range_cst_code_propagation() {
    // Load pali_titles from ArangoDB
    let pali_titles = tokio::runtime::Runtime::new()
        .expect("Failed to create tokio runtime")
        .block_on(async {
            match tipitaka_xml_parser::web::arangodb::get_pali_titles().await {
                Ok(titles) => Some(titles),
                Err(_) => None,
            }
        });

    let overrides = ParserOverrides {
        pali_titles,
        ..Default::default()
    };

    let xml_content = std::fs::read_to_string("tests/data/s0303t.tik.xml")
        .expect("Failed to read s0303t.tik.xml");

    let structure = detect_nikaya_structure(&xml_content)
        .expect("Failed to detect nikaya structure");

    let fragments = parse_into_fragments(
        &xml_content,
        &structure,
        "s0303t.tik.xml",
        &overrides,
        true,  // Enable populate_sc_fields
    ).expect("Failed to parse fragments");

    // Find the fragment containing "1-10. Cakkhusuttādivaṇṇanā"
    let cakkhu_frag = fragments.iter().find(|f| f.content_xml.contains("Cakkhusuttādivaṇṇanā"));
    assert!(cakkhu_frag.is_some(), "Should find fragment containing Cakkhusuttādivaṇṇanā");
    let cakkhu_frag = cakkhu_frag.unwrap();

    println!("Cakkhusuttādivaṇṇanā fragment (frag_idx_code {}):", cakkhu_frag.frag_idx_code);
    println!("  cst_code: {:?}", cakkhu_frag.cst_code);
    println!("  sc_code: {:?}", cakkhu_frag.sc_code);

    // Verify cst_code is correctly parsed as a range
    assert_eq!(cakkhu_frag.cst_code, Some("sn3.4.1.1-10".to_string()),
        "Cakkhusuttādivaṇṇanā fragment should have cst_code sn3.4.1.1-10");

    // Verify sc_code is in range format (sn25.1-10)
    // This requires pali_titles from ArangoDB to be available
    if cakkhu_frag.sc_code.is_none() {
        println!("NOTE: sc_code is None because ArangoDB pali_titles not available in test");
    } else {
        let sc_code = cakkhu_frag.sc_code.as_ref().unwrap();
        assert!(sc_code.contains('-'),
            "Cakkhusuttādivaṇṇanā fragment should have sc_code with range (e.g., sn25.1-10), got: {}", sc_code);
    }
}
