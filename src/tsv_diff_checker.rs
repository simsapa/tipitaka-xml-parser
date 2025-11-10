//! TSV diff checker for validating improvements in fragment exports
//!
//! This module provides functionality to compare two TSV exports of fragments
//! and validate that differences are improvements by checking against the
//! CST vs SC reference data.

use anyhow::{Result, Context};
use std::collections::HashMap;
use std::path::Path;
use serde::{Deserialize, Deserializer};
use crate::sutta_builder::load_tsv_mapping;

/// Deserialize empty strings as None for Option<String>
fn deserialize_option_string<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    let s: String = Deserialize::deserialize(deserializer)?;
    if s.trim().is_empty() {
        Ok(None)
    } else {
        Ok(Some(s))
    }
}

/// Fragment record from TSV export
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct FragmentTsvRow {
    pub cst_file: String,
    pub frag_idx: i32,
    #[serde(deserialize_with = "deserialize_option_string")]
    pub cst_code: Option<String>,
    #[serde(deserialize_with = "deserialize_option_string")]
    pub cst_vagga: Option<String>,
    #[serde(deserialize_with = "deserialize_option_string")]
    pub cst_sutta: Option<String>,
    #[serde(deserialize_with = "deserialize_option_string")]
    pub sc_code: Option<String>,
    #[serde(deserialize_with = "deserialize_option_string")]
    pub sc_sutta: Option<String>,
}

/// Result of comparing two TSV versions
#[derive(Debug)]
pub struct DiffCheckResult {
    pub total_rows: usize,
    pub differing_rows: usize,
    pub validation_errors: Vec<ValidationError>,
}

/// Validation error for a specific fragment
#[derive(Debug)]
pub struct ValidationError {
    pub cst_file: String,
    pub frag_idx: i32,
    pub cst_code: String,
    pub actual_cst_sutta: Option<String>,
    pub expected_cst_sutta: String,
}

/// Load CST-vs-SC reference data using existing load_tsv_mapping
///
/// Returns a HashMap mapping cst_code to cst_sutta
fn load_cst_sc_references() -> Result<HashMap<String, String>> {
    let records = load_tsv_mapping()
        .context("Failed to load TSV mapping")?;
    
    let mut references = HashMap::new();
    
    for record in records {
        // Skip empty cst_code
        if !record.tsv_cst_code.is_empty() {
            references.insert(
                record.tsv_cst_code.clone(),
                record.tsv_cst_sutta.clone(),
            );
        }
    }
    
    Ok(references)
}

/// Parse a TSV file into FragmentTsvRow records using serde
fn parse_tsv_file(path: &Path) -> Result<Vec<FragmentTsvRow>> {
    let file = std::fs::File::open(path)
        .context(format!("Failed to open TSV file: {:?}", path))?;
    
    let mut reader = csv::ReaderBuilder::new()
        .delimiter(b'\t')
        .from_reader(file);
    
    let records: Result<Vec<FragmentTsvRow>, csv::Error> = reader
        .deserialize()
        .collect();
    
    records.context("Failed to deserialize TSV records")
}

/// Check if differences between two TSV versions are improvements
///
/// Compares the new TSV against the old TSV, and for any rows that differ,
/// validates the new cst_sutta value against the CST-vs-SC reference data.
///
/// # Arguments
/// * `old_tsv_path` - Path to the older TSV export
/// * `new_tsv_path` - Path to the newer TSV export
///
/// # Returns
/// DiffCheckResult with validation errors (if any)
pub fn check_tsv_improvements(old_tsv_path: &Path, new_tsv_path: &Path) -> Result<DiffCheckResult> {
    // Load reference data
    let references = load_cst_sc_references()
        .context("Failed to load CST-vs-SC reference data")?;
    
    // Parse both TSV files
    let old_rows = parse_tsv_file(old_tsv_path)
        .context("Failed to parse old TSV file")?;
    let new_rows = parse_tsv_file(new_tsv_path)
        .context("Failed to parse new TSV file")?;
    
    // Create lookup maps (cst_file, frag_idx) -> row
    let old_map: HashMap<(String, i32), &FragmentTsvRow> = old_rows.iter()
        .map(|row| ((row.cst_file.clone(), row.frag_idx), row))
        .collect();
    
    let mut differing_rows = 0;
    let mut validation_errors = Vec::new();
    
    // Check each row in new TSV
    for new_row in &new_rows {
        let key = (new_row.cst_file.clone(), new_row.frag_idx);
        
        // Check if this row differs from the old version
        if let Some(old_row) = old_map.get(&key) {
            // Check if the row differs in any relevant field
            if new_row != *old_row {
                differing_rows += 1;
                
                // Check for regressions: if the old value was correct, the new value must also be correct
                if let Some(ref old_cst_code) = old_row.cst_code {
                    if let Some(reference_sutta) = references.get(old_cst_code) {
                        // Check if the OLD cst_sutta was correct (matched reference)
                        let old_was_correct = match &old_row.cst_sutta {
                            Some(sutta) => sutta == reference_sutta,
                            None => false,
                        };
                        
                        // If the old value was correct, check if new value is still correct
                        if old_was_correct {
                            // The new row should have the same cst_code and correct cst_sutta
                            let new_is_correct = match (&new_row.cst_code, &new_row.cst_sutta) {
                                (Some(new_code), Some(new_sutta)) => {
                                    // Check if cst_code matches and sutta is correct for that code
                                    if new_code == old_cst_code {
                                        new_sutta == reference_sutta
                                    } else {
                                        // cst_code changed, check if new sutta is correct for new code
                                        if let Some(new_ref_sutta) = references.get(new_code) {
                                            new_sutta == new_ref_sutta
                                        } else {
                                            // New code not in reference, can't validate
                                            true
                                        }
                                    }
                                }
                                _ => false,
                            };
                            
                            if !new_is_correct {
                                validation_errors.push(ValidationError {
                                    cst_file: new_row.cst_file.clone(),
                                    frag_idx: new_row.frag_idx,
                                    cst_code: old_cst_code.clone(),
                                    actual_cst_sutta: new_row.cst_sutta.clone(),
                                    expected_cst_sutta: reference_sutta.clone(),
                                });
                            }
                        }
                    }
                }
            }
        }
        // Note: We don't check new rows that didn't exist in the old version,
        // as there's no baseline to check for regressions
    }
    
    Ok(DiffCheckResult {
        total_rows: new_rows.len(),
        differing_rows,
        validation_errors,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_load_cst_sc_references() {
        let references = load_cst_sc_references().unwrap();
        
        // Check that we loaded some references
        assert!(!references.is_empty());
        
        // Check a known reference
        assert!(references.contains_key("dn1.1"));
        let dn1_sutta = references.get("dn1.1").unwrap();
        assert_eq!(dn1_sutta, "1. Brahmajālasuttaṃ");
    }

    #[test]
    fn test_parse_tsv_file() {
        // Note: The TSV format requires all 16 fields to be present
        let tsv_content = "cst_file\tfrag_idx\tfrag_type\tfrag_review\tnikaya\tcst_code\tsc_code\tcst_vagga\tcst_sutta\tcst_paranum\tsc_sutta\tstart_line\tstart_char\tend_line\tend_char\tgroup_levels
s0101m.mul.xml\t0\tHeader\t\tdigha\t\t\t\t\t\t\t1\t0\t6\t42\t[]
s0101m.mul.xml\t1\tSutta\t\tdigha\tdn1.1\tdn1\t\t1. Brahmajālasuttaṃ\t1\tBrahmajālasutta\t6\t42\t100\t0\t[]
";
        
        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file.write_all(tsv_content.as_bytes()).unwrap();
        temp_file.flush().unwrap();
        
        let rows = parse_tsv_file(temp_file.path()).unwrap();
        
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].cst_file, "s0101m.mul.xml");
        assert_eq!(rows[0].frag_idx, 0);
        assert_eq!(rows[0].cst_code, None);
        
        assert_eq!(rows[1].cst_file, "s0101m.mul.xml");
        assert_eq!(rows[1].frag_idx, 1);
        assert_eq!(rows[1].cst_code, Some("dn1.1".to_string()));
        assert_eq!(rows[1].cst_sutta, Some("1. Brahmajālasuttaṃ".to_string()));
    }

    #[test]
    fn test_check_tsv_improvements_old_wrong_new_fixed() {
        // Test: Old was wrong, new is fixed - should have NO errors
        let old_tsv = "cst_file\tfrag_idx\tfrag_type\tfrag_review\tnikaya\tcst_code\tsc_code\tcst_vagga\tcst_sutta\tcst_paranum\tsc_sutta\tstart_line\tstart_char\tend_line\tend_char\tgroup_levels
s0101m.mul.xml\t1\tSutta\t\tdigha\tdn1.1\tdn1\t\tWrong Title\t1\tBrahmajālasutta\t6\t42\t100\t0\t[]
";
        
        let new_tsv = "cst_file\tfrag_idx\tfrag_type\tfrag_review\tnikaya\tcst_code\tsc_code\tcst_vagga\tcst_sutta\tcst_paranum\tsc_sutta\tstart_line\tstart_char\tend_line\tend_char\tgroup_levels
s0101m.mul.xml\t1\tSutta\t\tdigha\tdn1.1\tdn1\t\t1. Brahmajālasuttaṃ\t1\tBrahmajālasutta\t6\t42\t100\t0\t[]
";
        
        let mut old_file = NamedTempFile::new().unwrap();
        old_file.write_all(old_tsv.as_bytes()).unwrap();
        old_file.flush().unwrap();
        
        let mut new_file = NamedTempFile::new().unwrap();
        new_file.write_all(new_tsv.as_bytes()).unwrap();
        new_file.flush().unwrap();
        
        let result = check_tsv_improvements(old_file.path(), new_file.path()).unwrap();
        
        assert_eq!(result.total_rows, 1);
        assert_eq!(result.differing_rows, 1);
        assert_eq!(result.validation_errors.len(), 0); // No errors - old was wrong, new is fixed
    }

    #[test]
    fn test_check_tsv_regression() {
        // Test: Old was correct, new is wrong - should have 1 error (regression!)
        let old_tsv = "cst_file\tfrag_idx\tfrag_type\tfrag_review\tnikaya\tcst_code\tsc_code\tcst_vagga\tcst_sutta\tcst_paranum\tsc_sutta\tstart_line\tstart_char\tend_line\tend_char\tgroup_levels
s0101m.mul.xml\t1\tSutta\t\tdigha\tdn1.1\tdn1\t\t1. Brahmajālasuttaṃ\t1\tBrahmajālasutta\t6\t42\t100\t0\t[]
";
        
        let new_tsv = "cst_file\tfrag_idx\tfrag_type\tfrag_review\tnikaya\tcst_code\tsc_code\tcst_vagga\tcst_sutta\tcst_paranum\tsc_sutta\tstart_line\tstart_char\tend_line\tend_char\tgroup_levels
s0101m.mul.xml\t1\tSutta\t\tdigha\tdn1.1\tdn1\t\tWrong Title\t1\tBrahmajālasutta\t6\t42\t100\t0\t[]
";
        
        let mut old_file = NamedTempFile::new().unwrap();
        old_file.write_all(old_tsv.as_bytes()).unwrap();
        old_file.flush().unwrap();
        
        let mut new_file = NamedTempFile::new().unwrap();
        new_file.write_all(new_tsv.as_bytes()).unwrap();
        new_file.flush().unwrap();
        
        let result = check_tsv_improvements(old_file.path(), new_file.path()).unwrap();
        
        assert_eq!(result.total_rows, 1);
        assert_eq!(result.differing_rows, 1);
        assert_eq!(result.validation_errors.len(), 1); // 1 error - regression detected!
        
        let error = &result.validation_errors[0];
        assert_eq!(error.cst_code, "dn1.1");
        assert_eq!(error.actual_cst_sutta, Some("Wrong Title".to_string()));
        assert_eq!(error.expected_cst_sutta, "1. Brahmajālasuttaṃ");
    }
}
