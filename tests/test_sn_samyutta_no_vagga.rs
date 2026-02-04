//! Test for SN samyuttas without vaggas
//! 
//! Tests that samyutta titles are correctly NOT assigned to cst_vagga
//! when the samyutta doesn't have vaggas (e.g., Vaṅgīsasaṃyuttaṃ)

use tipitaka_xml_parser::nikaya_detector::detect_nikaya_structure;
use tipitaka_xml_parser::parse_into_fragments;
use tipitaka_xml_parser::types::{FragmentType, ParserOverrides};

#[test]
fn test_sn_vaṅgīsa_samyutta_no_vagga() {
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
    
    // Find fragments around the transition from sn1.7 to sn1.8
    // Fragment 208 should be the last sutta of sn1.7 (which has vaggas)
    // Fragment 209 should be the first sutta of sn1.8 (Vaṅgīsasaṃyuttaṃ, which has NO vaggas)
    
    let sutta_fragments: Vec<_> = fragments.iter()
        .filter(|f| matches!(f.frag_type, FragmentType::Sutta))
        .collect();
    
    // Find fragment with cst_code sn1.7.2.12 (last sutta of samyutta 7)
    let frag_208_opt = sutta_fragments.iter()
        .find(|f| f.cst_code.as_deref() == Some("sn1.7.2.12"));
    
    if let Some(frag_208) = frag_208_opt {
        println!("\nFragment 208 (sn1.7.2.12):");
        println!("  frag_idx: {}", frag_208.frag_idx);
        println!("  cst_code: {:?}", frag_208.cst_code);
        println!("  cst_vagga: {:?}", frag_208.cst_vagga);
        println!("  cst_sutta: {:?}", frag_208.cst_sutta);
        
        // Samyutta 7 has vaggas, so cst_vagga should be Some
        assert!(frag_208.cst_vagga.is_some(), 
            "Fragment 208 (sn1.7.2.12) should have a vagga");
    }
    
    // Find fragment with cst_code sn1.8.1.1 (first sutta of Vaṅgīsasaṃyuttaṃ)
    let frag_209_opt = sutta_fragments.iter()
        .find(|f| f.cst_code.as_deref() == Some("sn1.8.1.1"));
    
    assert!(frag_209_opt.is_some(), 
        "Should find fragment with cst_code sn1.8.1.1");
    
    let frag_209 = frag_209_opt.unwrap();
    
    println!("\nFragment 209 (sn1.8.1.1):");
    println!("  frag_idx: {}", frag_209.frag_idx);
    println!("  cst_code: {:?}", frag_209.cst_code);
    println!("  cst_vagga: {:?}", frag_209.cst_vagga);
    println!("  cst_sutta: {:?}", frag_209.cst_sutta);
    println!("  group_levels:");
    for level in &frag_209.group_levels {
        println!("    {:?}: {}", level.group_type, level.title);
    }
    
    // The critical assertion: Vaṅgīsasaṃyuttaṃ has NO vaggas
    // So cst_vagga should be None, NOT "8. Vaṅgīsasaṃyuttaṃ"
    assert!(frag_209.cst_vagga.is_none(), 
        "Fragment 209 (sn1.8.1.1) should have cst_vagga=None because Vaṅgīsasaṃyuttaṃ has no vaggas. \
          Found: {:?}", frag_209.cst_vagga);
    
    // The sutta title should be "1. Nikkhantasuttaṃ"
    assert_eq!(frag_209.cst_sutta.as_deref(), Some("1. Nikkhantasuttaṃ"),
        "Fragment 209 should have cst_sutta='1. Nikkhantasuttaṃ'");
}
