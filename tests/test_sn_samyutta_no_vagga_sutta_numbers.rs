//! Test for sutta numbering in SN samyuttas without vaggas
//! 
//! Tests that sutta numbers are correctly incremented in samyuttas that don't have vaggas

use tipitaka_xml_parser::nikaya_detector::detect_nikaya_structure;
use tipitaka_xml_parser::parse_into_fragments;
use tipitaka_xml_parser::types::{FragmentType, ParserOverrides};

#[test]
fn test_sn_vaṅgīsa_samyutta_sutta_numbering() {
    // Read the actual s0301m.mul.xml file
    let xml_content = std::fs::read_to_string("tests/data/s0301m.mul.xml")
        .expect("Failed to read s0301m.mul.xml");
    
    // Detect structure
    let structure = detect_nikaya_structure(&xml_content)
        .expect("Failed to detect nikaya structure");
    
    // Parse into fragments
    let fragments = parse_into_fragments(
        &xml_content,
        &structure,
        "s0301m.mul.xml",
        &ParserOverrides::default(),
        false
    ).expect("Failed to parse fragments");
    
    let sutta_fragments: Vec<_> = fragments.iter()
        .filter(|f| matches!(f.frag_type, FragmentType::Sutta))
        .collect();
    
    // Find suttas in samyutta 8 (Vaṅgīsasaṃyuttaṃ)
    let sn8_suttas: Vec<_> = sutta_fragments.iter()
        .filter(|f| {
            f.cst_code.as_ref()
                .map(|c| c.starts_with("sn1.8."))
                .unwrap_or(false)
        })
        .collect();
    
    println!("\nFound {} suttas in sn1.8 (Vaṅgīsasaṃyuttaṃ):", sn8_suttas.len());
    for (i, s) in sn8_suttas.iter().take(5).enumerate() {
        println!("  {}. frag_idx={}, cst_code={:?}, cst_sutta={:?}", 
            i+1, s.frag_idx, s.cst_code, s.cst_sutta);
        
        // Print first 200 chars of content for fragment 210
        if s.frag_idx == 210 {
            let preview = s.content_xml.chars().take(200).collect::<String>();
            println!("     Content preview: {}", preview);
            
            // Check if it has subhead tag
            if s.content_xml.contains("rend=\"subhead\"") {
                println!("     ✓ Contains subhead tag");
                
                // Check group_levels
                println!("     group_levels:");
                for level in &s.group_levels {
                    println!("       {:?}: '{}'", level.group_type, level.title);
                }
            } else {
                println!("     ✗ NO subhead tag");
            }
        }
    }
    
    // Find the first 3 suttas of Vaṅgīsasaṃyuttaṃ (sn1.8)
    // They should be: sn1.8.1.1, sn1.8.1.2, sn1.8.1.3
    
    // Sutta 1: Nikkhantasuttaṃ
    let sutta1 = sutta_fragments.iter()
        .find(|f| f.cst_sutta.as_ref().map(|s| s.contains("Nikkhantasuttaṃ")).unwrap_or(false))
        .expect("Should find Nikkhantasuttaṃ");
    
    println!("\nSutta 1 (Nikkhantasuttaṃ):");
    println!("  frag_idx: {}", sutta1.frag_idx);
    println!("  cst_code: {:?}", sutta1.cst_code);
    println!("  cst_vagga: {:?}", sutta1.cst_vagga);
    println!("  cst_sutta: {:?}", sutta1.cst_sutta);
    
    assert_eq!(sutta1.cst_code.as_deref(), Some("sn1.8.1.1"),
        "Sutta 1 should have cst_code='sn1.8.1.1'");
    assert_eq!(sutta1.cst_vagga, None,
        "Sutta 1 should have cst_vagga=None (no vagga in this samyutta)");
    
    // Sutta 2: Aratisuttaṃ
    let sutta2 = sutta_fragments.iter()
        .find(|f| f.cst_sutta.as_ref().map(|s| s.contains("Aratisuttaṃ")).unwrap_or(false))
        .expect("Should find Aratisuttaṃ");
    
    println!("\nSutta 2 (Aratisuttaṃ):");
    println!("  frag_idx: {}", sutta2.frag_idx);
    println!("  cst_code: {:?}", sutta2.cst_code);
    println!("  cst_vagga: {:?}", sutta2.cst_vagga);
    println!("  cst_sutta: {:?}", sutta2.cst_sutta);
    
    assert_eq!(sutta2.cst_code.as_deref(), Some("sn1.8.1.2"),
        "Sutta 2 should have cst_code='sn1.8.1.2', found: {:?}", sutta2.cst_code);
    assert_eq!(sutta2.cst_vagga, None,
        "Sutta 2 should have cst_vagga=None (no vagga in this samyutta)");
    assert_eq!(sutta2.cst_sutta.as_deref(), Some("2. Aratisuttaṃ"),
        "Sutta 2 should have cst_sutta='2. Aratisuttaṃ'");
    
    // Verify fragment ordering
    assert!(sutta2.frag_idx > sutta1.frag_idx,
        "Sutta 2 fragment should come after Sutta 1");
}
