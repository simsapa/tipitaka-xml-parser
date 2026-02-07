//! Test for SN samyutta transition parsing
//! 
//! Regression test for the issue where suttas after a samyutta transition were getting
//! incorrect cst_codes due to the parser treating samyutta headings as sutta fragments.
//! 
//! The bug occurred when parsing s0301m.mul.xml (SamyuttaNikayaMula), specifically:
//! - After sn1.3.3.5: the next sutta should be sn1.4.1.1, not sn1.4.4.4
//! - After sn1.4.3.5: the next sutta should be sn1.5.1.1, not sn1.5.5.5
//! - After sn1.6.2.5: the next sutta should be sn1.7.1.1, not sn1.7.7.7
//!
//! The root cause was that <head rend="chapter"> in SN was being treated as a Vagga
//! boundary instead of a Samyutta boundary, causing the parser to create a fragment
//! for the samyutta heading itself and inherit incorrect numbering.

use std::fs;
use tipitaka_xml_parser::{
    nikaya_detector::detect_nikaya_structure,
    parsers::samyutta_nikaya_mula::parse_into_fragments,
    types::{FragmentType, ParserOverrides},
};

#[test]
fn test_sn_samyutta_transition_sn1_3_to_sn1_4() {
    // Test transition from Kosalasaṃyuttaṃ (sn1_3) to Mārasaṃyuttaṃ (sn1_4)
    let xml_path = "tests/data/s0301m.mul.xml";
    let xml_content = fs::read_to_string(xml_path)
        .expect("Failed to read s0301m.mul.xml");
    
    let structure = detect_nikaya_structure(&xml_content)
        .expect("Should detect SN structure");
    
    let fragments = parse_into_fragments(&xml_content, &structure, "s0301m.mul.xml", &ParserOverrides::default(), false)
        .expect("Should parse fragments");
    
    let sutta_fragments: Vec<_> = fragments.iter()
        .filter(|f| matches!(f.frag_type, FragmentType::Sutta))
        .collect();
    
    // Find the last sutta of Kosalasaṃyuttaṃ (should be sn1.3.3.5)
    let last_sutta_sn1_3 = sutta_fragments.iter()
        .find(|f| {
            f.group_levels.iter().any(|l| l.id.as_ref() == Some(&"sn1_3".to_string()))
                && f.cst_sutta.as_ref().map(|s| s.contains("Pabbatūpamasuttaṃ")).unwrap_or(false)
        })
        .expect("Should find last sutta of Kosalasaṃyuttaṃ (Pabbatūpamasuttaṃ)");
    
    assert_eq!(last_sutta_sn1_3.cst_code.as_deref(), Some("sn1.3.3.5"),
        "Last sutta of Kosalasaṃyuttaṃ should be sn1.3.3.5");
    assert_eq!(last_sutta_sn1_3.cst_vagga.as_deref(), Some("3. Tatiyavaggo"));
    assert_eq!(last_sutta_sn1_3.cst_sutta.as_deref(), Some("5. Pabbatūpamasuttaṃ"));
    
    // Find the first sutta of Mārasaṃyuttaṃ (should be sn1.4.1.1)
    let first_sutta_sn1_4 = sutta_fragments.iter()
        .find(|f| {
            f.group_levels.iter().any(|l| l.id.as_ref() == Some(&"sn1_4".to_string()))
                && f.cst_sutta.as_ref().map(|s| s.contains("Tapokammasuttaṃ")).unwrap_or(false)
        })
        .expect("Should find first sutta of Mārasaṃyuttaṃ (Tapokammasuttaṃ)");
    
    // THE BUG FIX: Should be sn1.4.1.1, not sn1.4.4.4
    assert_eq!(first_sutta_sn1_4.cst_code.as_deref(), Some("sn1.4.1.1"),
        "First sutta of Mārasaṃyuttaṃ should be sn1.4.1.1, not sn1.4.4.4");
    assert_eq!(first_sutta_sn1_4.cst_vagga.as_deref(), Some("1. Paṭhamavaggo"));
    assert_eq!(first_sutta_sn1_4.cst_sutta.as_deref(), Some("1. Tapokammasuttaṃ"));
    
    // Verify fragment order
    assert!(first_sutta_sn1_4.frag_idx > last_sutta_sn1_3.frag_idx,
        "First sutta of Mārasaṃyuttaṃ should come after last sutta of Kosalasaṃyuttaṃ");
    
    // Note: There is currently a known issue where an extra fragment is created for the
    // samyutta/vagga headings. This fragment doesn't have a Sutta level in group_levels,
    // so it gets incorrect CST derivation. This needs to be fixed, but the core issue
    // (incorrect CST codes for actual suttas) has been resolved.
    // TODO: Fix the spurious fragment creation for samyutta/vagga headings
}

#[test]
fn test_sn_samyutta_transition_sn1_4_to_sn1_5() {
    // Test transition from Mārasaṃyuttaṃ (sn1_4) to Bhikkhunīsaṃyuttaṃ (sn1_5)
    let xml_path = "tests/data/s0301m.mul.xml";
    let xml_content = fs::read_to_string(xml_path)
        .expect("Failed to read s0301m.mul.xml");
    
    let structure = detect_nikaya_structure(&xml_content)
        .expect("Should detect SN structure");
    
    let fragments = parse_into_fragments(&xml_content, &structure, "s0301m.mul.xml", &ParserOverrides::default(), false)
        .expect("Should parse fragments");
    
    let sutta_fragments: Vec<_> = fragments.iter()
        .filter(|f| matches!(f.frag_type, FragmentType::Sutta))
        .collect();
    
    // Find the last sutta of Mārasaṃyuttaṃ (should be sn1.4.3.5)
    let last_sutta_sn1_4 = sutta_fragments.iter()
        .rev()  // Search from the end
        .find(|f| {
            f.group_levels.iter().any(|l| l.id.as_ref() == Some(&"sn1_4".to_string()))
                && f.cst_code.as_ref().map(|c| c.starts_with("sn1.4.3.")).unwrap_or(false)
        })
        .expect("Should find last sutta of Mārasaṃyuttaṃ");
    
    assert_eq!(last_sutta_sn1_4.cst_code.as_deref(), Some("sn1.4.3.5"),
        "Last sutta of Mārasaṃyuttaṃ should be sn1.4.3.5");
    
    // Find the first sutta of Bhikkhunīsaṃyuttaṃ
    // Note: Bhikkhunīsaṃyuttaṃ doesn't have vaggas, so it uses vagga 1: sn1.5.1.1
    let first_sutta_sn1_5 = sutta_fragments.iter()
        .find(|f| {
            f.group_levels.iter().any(|l| l.id.as_ref() == Some(&"sn1_5".to_string()))
                && f.cst_code.as_ref().map(|c| c == "sn1.5.1.1").unwrap_or(false)
        })
        .expect("Should find first sutta of Bhikkhunīsaṃyuttaṃ");
    
    // THE BUG FIX: Should be sn1.5.1.1 (vagga 1 for samyuttas without vaggas), not sn1.5.5.5
    assert_eq!(first_sutta_sn1_5.cst_code.as_deref(), Some("sn1.5.1.1"),
        "First sutta of Bhikkhunīsaṃyuttaṃ should be sn1.5.1.1, not sn1.5.5.5");
    
    // Verify fragment order
    assert!(first_sutta_sn1_5.frag_idx > last_sutta_sn1_4.frag_idx,
        "First sutta of Bhikkhunīsaṃyuttaṃ should come after last sutta of Mārasaṃyuttaṃ");
}

#[test]
fn test_sn_samyutta_transition_sn1_6_to_sn1_7() {
    // Test transition from Brahmāsaṃyuttaṃ (sn1_6) to Brāhmaṇasaṃyuttaṃ (sn1_7)
    let xml_path = "tests/data/s0301m.mul.xml";
    let xml_content = fs::read_to_string(xml_path)
        .expect("Failed to read s0301m.mul.xml");
    
    let structure = detect_nikaya_structure(&xml_content)
        .expect("Should detect SN structure");
    
    let fragments = parse_into_fragments(&xml_content, &structure, "s0301m.mul.xml", &ParserOverrides::default(), false)
        .expect("Should parse fragments");
    
    let sutta_fragments: Vec<_> = fragments.iter()
        .filter(|f| matches!(f.frag_type, FragmentType::Sutta))
        .collect();
    
    // Find the last sutta of Brahmāsaṃyuttaṃ (should be sn1.6.2.5)
    let last_sutta_sn1_6 = sutta_fragments.iter()
        .rev()
        .find(|f| {
            f.group_levels.iter().any(|l| l.id.as_ref() == Some(&"sn1_6".to_string()))
                && f.cst_code.as_ref().map(|c| c.starts_with("sn1.6.2.")).unwrap_or(false)
        })
        .expect("Should find last sutta of Brahmāsaṃyuttaṃ");
    
    assert_eq!(last_sutta_sn1_6.cst_code.as_deref(), Some("sn1.6.2.5"),
        "Last sutta of Brahmāsaṃyuttaṃ should be sn1.6.2.5");
    
    // Find the first sutta of Brāhmaṇasaṃyuttaṃ (should be sn1.7.1.1)
    let first_sutta_sn1_7 = sutta_fragments.iter()
        .find(|f| {
            f.group_levels.iter().any(|l| l.id.as_ref() == Some(&"sn1_7".to_string()))
                && f.cst_code.as_ref().map(|c| c == "sn1.7.1.1").unwrap_or(false)
        })
        .expect("Should find first sutta of Brāhmaṇasaṃyuttaṃ");
    
    // THE BUG FIX: Should be sn1.7.1.1, not sn1.7.7.7
    assert_eq!(first_sutta_sn1_7.cst_code.as_deref(), Some("sn1.7.1.1"),
        "First sutta of Brāhmaṇasaṃyuttaṃ should be sn1.7.1.1, not sn1.7.7.7");
    
    // Verify fragment order
    assert!(first_sutta_sn1_7.frag_idx > last_sutta_sn1_6.frag_idx,
        "First sutta of Brāhmaṇasaṃyuttaṃ should come after last sutta of Brahmāsaṃyuttaṃ");
}
