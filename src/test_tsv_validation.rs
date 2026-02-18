//! Test to validate parsed CST fields against cst-vs-sc.tsv
//!
//! This test ensures that the cst_code, cst_file, and cst_sutta extracted from
//! XML fragments match the expected values in the TSV mapping file.

use std::collections::HashMap;
use std::path::Path;
use anyhow::{Result, Context};

use crate::{
    detect_nikaya_structure,
    parse_into_fragments,
};
use crate::types::ParserOverrides;
use crate::encoding::read_xml_file;
use crate::sutta_builder::TsvRecord;

// Import the static TSV data
static CST_VS_SC_TSV: &str = include_str!("../assets/cst-vs-sc.tsv");

#[derive(Debug, Clone)]
struct TsvExpectation {
    cst_code: String,
    cst_file: String,
    cst_sutta: String,
    sc_code: String,
}

/// Load TSV expectations for a given XML file
fn load_tsv_expectations(cst_file: &str) -> Result<Vec<TsvExpectation>> {
    // Deserialize all TSV records
    let mut reader = csv::ReaderBuilder::new()
        .delimiter(b'\t')
        .from_reader(CST_VS_SC_TSV.as_bytes());
    
    let records: Vec<TsvRecord> = reader
        .deserialize()
        .collect::<Result<Vec<TsvRecord>, csv::Error>>()
        .context("Failed to deserialize TSV records")?;
    
    // Normalize filename for comparison (handle both with and without "romn/" prefix)
    let normalized_filename = cst_file.trim_start_matches("romn/");
    
    // Filter records matching the cst_file and convert to TsvExpectation
    let expectations: Vec<TsvExpectation> = records
        .into_iter()
        .filter_map(|record| {
            let file_normalized = record.tsv_cst_file.trim_start_matches("romn/");
            
            if file_normalized == normalized_filename {
                Some(TsvExpectation {
                    cst_code: record.tsv_cst_code,
                    cst_file: record.tsv_cst_file,
                    cst_sutta: record.tsv_cst_sutta,
                    sc_code: record.tsv_sc_code,
                })
            } else {
                None
            }
        })
        .collect();
    
    Ok(expectations)
}

#[test]
fn test_parse_matches_tsv_s0101m() {
    let xml_path = Path::new("tests/data/s0101m.mul.xml");
    
    // Load expectations from TSV
    let expectations = load_tsv_expectations("s0101m.mul.xml")
        .expect("Failed to load TSV expectations");
    
    assert!(!expectations.is_empty(), "No TSV expectations found for s0101m.mul.xml");
    
    // Parse XML file
    let xml_content = read_xml_file(xml_path)
        .expect("Failed to read XML file");
    
    let nikaya_structure = detect_nikaya_structure(&xml_content)
        .expect("Failed to detect nikaya structure");
    
    let fragments = parse_into_fragments(&xml_content, &nikaya_structure, "s0101m.mul.xml", &ParserOverrides::default(), true)
        .expect("Failed to parse fragments");
    
    // Filter to Sutta fragments only
    let sutta_fragments: Vec<_> = fragments.iter()
        .filter(|f| matches!(f.frag_type, crate::types::FragmentType::Sutta))
        .collect();
    
    // Build a map of expected data by cst_sutta for easier lookup
    let expected_map: HashMap<String, &TsvExpectation> = expectations.iter()
        .map(|e| (e.cst_sutta.clone(), e))
        .collect();
    
    // Validate each sutta fragment
    let mut errors = Vec::new();
    
    for fragment in &sutta_fragments {
        if let Some(ref cst_sutta) = fragment.cst_sutta {
            if let Some(expected) = expected_map.get(cst_sutta) {
                // Check cst_code
                if fragment.cst_code.as_deref() != Some(&expected.cst_code) {
                    errors.push(format!(
                        "Sutta '{}': expected cst_code '{}', got '{:?}'",
                        cst_sutta, expected.cst_code, fragment.cst_code
                    ));
                }
                
                // Check cst_file
                let fragment_file = fragment.cst_file.trim_start_matches("romn/");
                let expected_file = expected.cst_file.trim_start_matches("romn/");
                if fragment_file != expected_file {
                    errors.push(format!(
                        "Sutta '{}': expected cst_file '{}', got '{}'",
                        cst_sutta, expected_file, fragment_file
                    ));
                }
                
                // Check sc_code
                if fragment.sc_code.as_deref() != Some(&expected.sc_code) {
                    errors.push(format!(
                        "Sutta '{}': expected sc_code '{}', got '{:?}'",
                        cst_sutta, expected.sc_code, fragment.sc_code
                    ));
                }
            } else {
                errors.push(format!(
                    "Sutta '{}' not found in TSV expectations",
                    cst_sutta
                ));
            }
        }
    }
    
    // Report all errors
    if !errors.is_empty() {
        panic!("TSV validation failed with {} errors:\n{}", 
               errors.len(), errors.join("\n"));
    }
}

#[test]
fn test_parse_matches_tsv_s0201m() {
    let xml_path = Path::new("tests/data/s0201m.mul.xml");
    
    // Load expectations from TSV
    let expectations = load_tsv_expectations("s0201m.mul.xml")
        .expect("Failed to load TSV expectations");
    
    assert!(!expectations.is_empty(), "No TSV expectations found for s0201m.mul.xml");
    
    // Parse XML file
    let xml_content = read_xml_file(xml_path)
        .expect("Failed to read XML file");
    
    let nikaya_structure = detect_nikaya_structure(&xml_content)
        .expect("Failed to detect nikaya structure");
    
    let fragments = parse_into_fragments(&xml_content, &nikaya_structure, "s0201m.mul.xml", &ParserOverrides::default(), true)
        .expect("Failed to parse fragments");
    
    // Filter to Sutta fragments only
    let sutta_fragments: Vec<_> = fragments.iter()
        .filter(|f| matches!(f.frag_type, crate::types::FragmentType::Sutta))
        .collect();
    
    // Build a map of expected data by cst_sutta for easier lookup
    let expected_map: HashMap<String, &TsvExpectation> = expectations.iter()
        .map(|e| (e.cst_sutta.clone(), e))
        .collect();
    
    // Validate each sutta fragment
    let mut errors = Vec::new();
    
    for fragment in &sutta_fragments {
        if let Some(ref cst_sutta) = fragment.cst_sutta {
            if let Some(expected) = expected_map.get(cst_sutta) {
                // Check cst_code
                if fragment.cst_code.as_deref() != Some(&expected.cst_code) {
                    errors.push(format!(
                        "Sutta '{}': expected cst_code '{}', got '{:?}'",
                        cst_sutta, expected.cst_code, fragment.cst_code
                    ));
                }
                
                // Check cst_file
                let fragment_file = fragment.cst_file.trim_start_matches("romn/");
                let expected_file = expected.cst_file.trim_start_matches("romn/");
                if fragment_file != expected_file {
                    errors.push(format!(
                        "Sutta '{}': expected cst_file '{}', got '{}'",
                        cst_sutta, expected_file, fragment_file
                    ));
                }
                
                // Check sc_code
                if fragment.sc_code.as_deref() != Some(&expected.sc_code) {
                    errors.push(format!(
                        "Sutta '{}': expected sc_code '{}', got '{:?}'",
                        cst_sutta, expected.sc_code, fragment.sc_code
                    ));
                }
            } else {
                errors.push(format!(
                    "Sutta '{}' not found in TSV expectations",
                    cst_sutta
                ));
            }
        }
    }
    
    // Report all errors
    if !errors.is_empty() {
        panic!("TSV validation failed with {} errors:\n{}", 
               errors.len(), errors.join("\n"));
    }
}

#[test]
fn test_s0201m_first_sutta_fragment() {
    // This test specifically verifies that fragment index 1 (the first sutta fragment)
    // from s0201m.mul.xml has the correct cst_code and cst_sutta values.
    // This ensures the preamble is correctly included with the first sutta.
    
    let xml_path = Path::new("tests/data/s0201m.mul.xml");

    // Parse XML file
    let xml_content = read_xml_file(xml_path)
        .expect("Failed to read XML file");
    
    let nikaya_structure = detect_nikaya_structure(&xml_content)
        .expect("Failed to detect nikaya structure");
    
    let fragments = parse_into_fragments(&xml_content, &nikaya_structure, "s0201m.mul.xml", &ParserOverrides::default(), true)
        .expect("Failed to parse fragments");
    
    // Fragment 0 should be Header, fragment 1 should be the first Sutta
    assert!(fragments.len() > 1, "Expected at least 2 fragments");
    
    let first_sutta_fragment = &fragments[1];
    
    // Verify fragment type
    assert!(
        matches!(first_sutta_fragment.frag_type, crate::types::FragmentType::Sutta),
        "Fragment 1 should be a Sutta fragment, got: {:?}", first_sutta_fragment.frag_type
    );
    
    // Verify cst_code
    assert_eq!(
        first_sutta_fragment.cst_code.as_deref(),
        Some("mn1.1.1"),
        "Fragment 1 should have cst_code 'mn1.1.1', got: {:?}",
        first_sutta_fragment.cst_code
    );
    
    // Verify cst_sutta
    assert_eq!(
        first_sutta_fragment.cst_sutta.as_deref(),
        Some("1. Mūlapariyāyasuttaṃ"),
        "Fragment 1 should have cst_sutta '1. Mūlapariyāyasuttaṃ', got: {:?}",
        first_sutta_fragment.cst_sutta
    );
    
    // Verify frag_idx_code
    assert_eq!(
        first_sutta_fragment.frag_idx_code,
        "1.0",
        "Fragment should have frag_idx_code '1.0', got: {}",
        first_sutta_fragment.frag_idx_code
    );
}

#[test]
fn test_s0201a_att_vagga_zero_fragment() {
    // This test specifically verifies that fragment index 1 from s0201a.att.xml
    // (the commentary file) correctly gets cst_code "mn1.0.0" for the introduction
    // section (vagga 0) which includes <div id="mn1_0" n="mn1_0" type="vagga">
    
    let xml_path = Path::new("tests/data/s0201a.att.xml");
    let xml_content = read_xml_file(xml_path)
        .expect("Failed to read XML file");
    
    let nikaya_structure = detect_nikaya_structure(&xml_content)
        .expect("Failed to detect nikaya structure");
    
    let fragments = parse_into_fragments(&xml_content, &nikaya_structure, "s0201a.att.xml", &ParserOverrides::default(), false)
        .expect("Failed to parse fragments");
    
    // Fragment 0 should be Header, fragment 1 should be the introduction (vagga 0)
    assert!(fragments.len() > 2, "Expected at least 3 fragments");
    
    let intro_fragment = &fragments[1];
    
    // Verify fragment type
    assert!(
        matches!(intro_fragment.frag_type, crate::types::FragmentType::Sutta),
        "Fragment 1 should be a Sutta fragment, got: {:?}", intro_fragment.frag_type
    );
    
    // Verify cst_code for vagga 0 (introduction)
    assert_eq!(
        intro_fragment.cst_code.as_deref(),
        Some("mn1.0.0"),
        "Fragment 1 should have cst_code 'mn1.0.0' for vagga 0, got: {:?}",
        intro_fragment.cst_code
    );
    
    // Verify frag_idx_code
    assert_eq!(
        intro_fragment.frag_idx_code,
        "1.0",
        "Fragment should have frag_idx_code '1.0', got: {}",
        intro_fragment.frag_idx_code
    );
    
    // Verify that fragment 2 has the correct cst_code for the first real sutta
    let first_sutta_fragment = &fragments[2];
    assert_eq!(
        first_sutta_fragment.cst_code.as_deref(),
        Some("mn1.1.1"),
        "Fragment 2 should have cst_code 'mn1.1.1', got: {:?}",
        first_sutta_fragment.cst_code
    );
}


#[test]
fn test_s0201a_att_kakacupama_sutta() {
    // This test verifies that "1. Kakacūpamasuttavaṇṇanā" in s0201a.att.xml
    // correctly gets cst_code "mn1.3.1" derived from vagga id "mn1_3" and sutta title "1. ..."
    
    let xml_path = Path::new("tests/data/s0201a.att.xml");
    let xml_content = read_xml_file(xml_path)
        .expect("Failed to read XML file");
    
    let nikaya_structure = detect_nikaya_structure(&xml_content)
        .expect("Failed to detect nikaya structure");
    
    let fragments = parse_into_fragments(&xml_content, &nikaya_structure, "s0201a.att.xml", &ParserOverrides::default(), false)
        .expect("Failed to parse fragments");
    
    // Find the fragment containing Kakacūpamasuttavaṇṇanā
    let kakacupama_fragment = fragments.iter()
        .find(|f| f.content_xml.contains("Kakacūpamasuttavaṇṇanā"))
        .expect("Should find Kakacūpamasuttavaṇṇanā fragment");
    
    // Verify cst_code
    assert_eq!(
        kakacupama_fragment.cst_code.as_deref(),
        Some("mn1.3.1"),
        "Kakacūpamasuttavaṇṇanā should have cst_code 'mn1.3.1', got: {:?}",
        kakacupama_fragment.cst_code
    );
}

#[test]
fn test_s0201t_tik_culasihanada_sutta() {
    // This test verifies that "1. Cūḷasīhanādasuttavaṇṇanā" in s0201t.tik.xml
    // correctly gets cst_code "mn1.2.1" derived from vagga id "mn1_2" and sutta title "1. ..."
    
    let xml_path = Path::new("tests/data/s0201t.tik.xml");
    let xml_content = read_xml_file(xml_path)
        .expect("Failed to read XML file");
    
    let nikaya_structure = detect_nikaya_structure(&xml_content)
        .expect("Failed to detect nikaya structure");
    
    let fragments = parse_into_fragments(&xml_content, &nikaya_structure, "s0201t.tik.xml", &ParserOverrides::default(), false)
        .expect("Failed to parse fragments");
    
    // Find the fragment containing Cūḷasīhanādasuttavaṇṇanā
    let culasihanada_fragment = fragments.iter()
        .find(|f| f.content_xml.contains("Cūḷasīhanādasuttavaṇṇanā"))
        .expect("Should find Cūḷasīhanādasuttavaṇṇanā fragment");
    
    // Verify cst_code
    assert_eq!(
        culasihanada_fragment.cst_code.as_deref(),
        Some("mn1.2.1"),
        "Cūḷasīhanādasuttavaṇṇanā should have cst_code 'mn1.2.1', got: {:?}",
        culasihanada_fragment.cst_code
    );
}
