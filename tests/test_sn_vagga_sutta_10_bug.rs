//! Test for SN vagga sutta 10 parsing bug
//! 
//! Regression test for the issue where sutta 10 in vagga 2 was getting 
//! cst_code "sn1.3.3.10" instead of the correct "sn1.3.2.10".
//! 
//! The bug occurs when parsing s0301m.mul.xml (SamyuttaNikayaMula), specifically:
//! - frag_idx 132: cst_code "sn1.3.2.9" for "9. Paṭhamaaputtakasuttaṃ" (correct)
//! - frag_idx 133: cst_code should be "sn1.3.2.10" for "10. Dutiyaaputtakasuttaṃ" (was "sn1.3.3.10")
//!
//! The issue is that the parser was prematurely incrementing the vagga number when
//! encountering the vagga 3 title, even though sutta 10 is still in vagga 2.

use std::fs;
use tipitaka_xml_parser::{
    nikaya_detector::detect_nikaya_structure,
    parsers::samyutta_nikaya_mula::parse_into_fragments,
    types::{FragmentType, GroupType},
};

#[test]
fn test_s0301m_mul_sutta_10_vagga_2() {
    // Load the actual XML file
    let xml_path = "tests/data/s0301m.mul.xml";
    let xml_content = fs::read_to_string(xml_path)
        .expect("Failed to read s0301m.mul.xml");
    
    // Detect nikaya structure
    let structure = detect_nikaya_structure(&xml_content)
        .expect("Should detect SN structure");
    
    assert_eq!(structure.nikaya, "samyutta");
    
    // Parse into fragments
    let fragments = parse_into_fragments(&xml_content, &structure, "s0301m.mul.xml", None, false)
        .expect("Should parse fragments");
    
    // Filter to get only Sutta fragments
    let sutta_fragments: Vec<_> = fragments.iter()
        .filter(|f| matches!(f.frag_type, FragmentType::Sutta))
        .collect();
    
    // Find the two suttas in question
    let sutta_9 = sutta_fragments.iter()
        .find(|f| f.cst_sutta.as_ref().map(|s| s.contains("Paṭhamaaputtakasuttaṃ")).unwrap_or(false))
        .expect("Should find sutta 9 (Paṭhamaaputtakasuttaṃ)");
    
    let sutta_10 = sutta_fragments.iter()
        .find(|f| f.cst_sutta.as_ref().map(|s| s.contains("Dutiyaaputtakasuttaṃ")).unwrap_or(false))
        .expect("Should find sutta 10 (Dutiyaaputtakasuttaṃ)");
    
    // Verify sutta 9 is correct
    assert_eq!(sutta_9.cst_code.as_deref(), Some("sn1.3.2.9"),
        "Sutta 9 should have code sn1.3.2.9");
    assert_eq!(sutta_9.cst_sutta.as_deref(), Some("9. Paṭhamaaputtakasuttaṃ"));
    
    // Verify both suttas are in vagga 2 (this is the critical check!)
    assert_eq!(sutta_9.cst_vagga.as_ref().and_then(|v| v.split_whitespace().next()),
        Some("2."),
        "Sutta 9 should be in vagga 2");
    
    assert_eq!(sutta_10.cst_vagga.as_ref().and_then(|v| v.split_whitespace().next()),
        Some("2."),
        "Sutta 10 should be in vagga 2");
    
    // THE BUG: Sutta 10 should have code sn1.3.2.10, not sn1.3.3.10
    assert_eq!(sutta_10.cst_code.as_deref(), Some("sn1.3.2.10"),
        "Sutta 10 should have code sn1.3.2.10 (samyutta 3, vagga 2, sutta 10), not sn1.3.3.10");
    assert_eq!(sutta_10.cst_sutta.as_deref(), Some("10. Dutiyaaputtakasuttaṃ"));
    
    // Verify that vagga 3 of Kosalasaṃyuttaṃ (sn1_3) comes AFTER sutta 10
    // We need to filter for samyutta sn1_3 specifically, not just any vagga 3
    let kosala_samyutta_vagga_3_suttas: Vec<_> = sutta_fragments.iter()
        .filter(|f| {
            // Check if this sutta is in samyutta sn1_3
            let in_kosala_samyutta = f.group_levels.iter().any(|level| {
                level.id.as_ref().map(|id| id == "sn1_3").unwrap_or(false)
            });

            /*
             * NOTE: Earlier test condition
            // Check if it's in vagga 3 (and NOT a header fragment)
            let in_vagga_3 = f.cst_vagga.as_ref()
                .and_then(|v| v.split_whitespace().next())
                .map(|n| n == "3.")
                .unwrap_or(false)
                && f.cst_code.is_some();  // Skip header/intro fragments which have None cst_code
            */

            // Check if it's in vagga 3 (actual vagga, not samyutta title)
            // The cst_vagga field might be "3. Tatiyavaggo" (vagga) or "3. Kosalasaṃyuttaṃ" (samyutta)
            // Due to parser quirk, Samyutta titles are sometimes stored as Vagga levels
            // So we check the title content: if it ends with "saṃyuttaṃ", it's a Samyutta, not a Vagga
            let in_vagga_3 = f.group_levels.iter().any(|level| {
                matches!(level.group_type, GroupType::Vagga)
                    && level.title.starts_with("3.")
                    && !level.title.to_lowercase().ends_with("saṃyuttaṃ")
            }) && f.cst_code.is_some();  // Skip header/intro fragments which have None cst_code

            in_kosala_samyutta && in_vagga_3
        })
        .collect();
    
    // Debug: print Kosalasaṃyuttaṃ vagga 3 suttas
    eprintln!("Sutta 10 frag_idx: {}", sutta_10.frag_idx);
    eprintln!("Kosalasaṃyuttaṃ vagga 3 suttas: {}", kosala_samyutta_vagga_3_suttas.len());
    for sutta in &kosala_samyutta_vagga_3_suttas {
        eprintln!("  frag_idx={}, vagga={:?}, sutta={:?}, cst_code={:?}",
            sutta.frag_idx, sutta.cst_vagga, sutta.cst_sutta, sutta.cst_code);
    }
    
    // Check that Kosalasaṃyuttaṃ vagga 3 suttas come after sutta 10 in fragment order
    if let Some(first_vagga_3_sutta) = kosala_samyutta_vagga_3_suttas.first() {
        // Skip if this is a header/intro fragment (cst_code would be sn1.3.3.0)
        if first_vagga_3_sutta.cst_code.as_deref() == Some("sn1.3.3.0") {
            eprintln!("Skipping header fragment at frag_idx {}", first_vagga_3_sutta.frag_idx);
            // Check the next one
            if let Some(second_vagga_3_sutta) = kosala_samyutta_vagga_3_suttas.get(1) {
                assert!(second_vagga_3_sutta.frag_idx > sutta_10.frag_idx,
                    "Kosalasaṃyuttaṃ vagga 3 suttas should come after sutta 10 in fragment order (frag_idx {} vs {})",
                    second_vagga_3_sutta.frag_idx, sutta_10.frag_idx);
            }
        } else {
            assert!(first_vagga_3_sutta.frag_idx > sutta_10.frag_idx,
                "Kosalasaṃyuttaṃ vagga 3 suttas should come after sutta 10 in fragment order (frag_idx {} vs {})",
                first_vagga_3_sutta.frag_idx, sutta_10.frag_idx);
        }
    } else {
        panic!("Should find at least one sutta in Kosalasaṃyuttaṃ vagga 3");
    }
}

#[test]
fn test_s0301m_mul_vagga_boundaries() {
    // Additional test to verify vagga boundaries are correctly detected
    let xml_path = "tests/data/s0301m.mul.xml";
    let xml_content = fs::read_to_string(xml_path)
        .expect("Failed to read s0301m.mul.xml");
    
    let structure = detect_nikaya_structure(&xml_content)
        .expect("Should detect SN structure");
    
    let fragments = parse_into_fragments(&xml_content, &structure, "s0301m.mul.xml", None, false)
        .expect("Should parse fragments");
    
    let sutta_fragments: Vec<_> = fragments.iter()
        .filter(|f| matches!(f.frag_type, FragmentType::Sutta))
        .collect();
    
    // Find suttas in Samyutta 3 (Kosalasaṃyuttaṃ)
    let samyutta_3_suttas: Vec<_> = sutta_fragments.iter()
        .filter(|f| {
            f.group_levels.iter().any(|level| {
                level.id.as_ref().map(|id| id == "sn1_3").unwrap_or(false)
            })
        })
        .collect();
    
    // Group by vagga number
    let mut vagga_counts = std::collections::HashMap::new();
    for sutta in &samyutta_3_suttas {
        if let Some(vagga) = &sutta.cst_vagga {
            let vagga_num = vagga.split_whitespace().next().unwrap_or("?");
            *vagga_counts.entry(vagga_num.to_string()).or_insert(0) += 1;
        }
    }
    
    // Verify that vagga 2 has at least 10 suttas
    let vagga_2_count = vagga_counts.get("2.").copied().unwrap_or(0);
    assert!(vagga_2_count >= 10,
        "Vagga 2 should have at least 10 suttas, found {}", vagga_2_count);
    
    // Verify that vagga 3 exists and has suttas
    let vagga_3_count = vagga_counts.get("3.").copied().unwrap_or(0);
    assert!(vagga_3_count > 0,
        "Vagga 3 should exist and have suttas, found {}", vagga_3_count);
}
