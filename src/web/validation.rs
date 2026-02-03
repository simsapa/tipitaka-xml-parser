//! Validation checks for fragment data integrity
//!
//! This module provides validation functions to check fragment data against
//! SuttaCentral references and identify missing or inconsistent metadata.
//!
//! ## Available Checks
//!
//! - `check_missing_sc_code`: Finds Sutta fragments without sc_code
//! - `check_missing_sc_sutta`: Finds fragments with sc_code but no sc_sutta title
//!
//! ## Auto-Fix Support
//!
//! Some checks can provide auto-fix suggestions. For example, missing sc_sutta
//! values can be populated from the Pali titles cache fetched from ArangoDB.

use std::collections::HashMap;
use diesel::prelude::*;
use serde::{Serialize, Deserialize};

use crate::fragments_schema::xml_fragments;

/// A validation error found during a check
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationError {
    /// The CST XML filename containing the fragment
    pub cst_file: String,
    /// The fragment index within the file
    pub frag_idx: i32,
    /// The database ID of the fragment
    pub fragment_id: i32,
    /// Human-readable description of the error
    pub message: String,
}

/// An auto-fix suggestion for a validation error
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoFix {
    /// The database ID of the fragment to fix
    pub fragment_id: i32,
    /// The CST XML filename containing the fragment
    pub cst_file: String,
    /// The fragment index within the file
    pub frag_idx: i32,
    /// The sc_code of the fragment (for display purposes)
    pub sc_code: String,
    /// The suggested value to apply
    pub suggested_value: String,
}

/// Result of running a single validation check
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationCheckResult {
    /// Display name of the check
    pub name: String,
    /// Description of what the check validates
    pub description: String,
    /// Whether this check supports auto-fixing
    pub auto_fixable: bool,
    /// List of validation errors found
    pub errors: Vec<ValidationError>,
    /// List of auto-fix suggestions (empty if not auto_fixable)
    pub auto_fixes: Vec<AutoFix>,
}

/// Check for Sutta fragments missing sc_code
///
/// Finds fragments where:
/// - frag_type = 'Sutta'
/// - frag_review != 'moved'
/// - sc_code IS NULL OR sc_code = ''
pub fn check_missing_sc_code(conn: &mut SqliteConnection) -> ValidationCheckResult {
    let results: Vec<(i32, String, i32, Option<String>)> = xml_fragments::table
        .select((
            xml_fragments::id,
            xml_fragments::cst_file,
            xml_fragments::frag_idx,
            xml_fragments::sc_code,
        ))
        .filter(xml_fragments::frag_type.eq("Sutta"))
        .filter(xml_fragments::frag_review.ne("moved").or(xml_fragments::frag_review.is_null()))
        .filter(xml_fragments::sc_code.is_null().or(xml_fragments::sc_code.eq("")))
        .load(conn)
        .unwrap_or_default();

    let errors: Vec<ValidationError> = results
        .into_iter()
        .map(|(id, cst_file, frag_idx, _)| ValidationError {
            cst_file,
            frag_idx,
            fragment_id: id,
            message: "Sutta fragment is missing sc_code".to_string(),
        })
        .collect();

    ValidationCheckResult {
        name: "Missing sc_code".to_string(),
        description: "Sutta fragments that don't have an sc_code assigned".to_string(),
        auto_fixable: false,
        errors,
        auto_fixes: vec![],
    }
}

/// Check for fragments with sc_code but missing sc_sutta title
///
/// Finds fragments where:
/// - sc_code IS NOT NULL AND sc_code != ''
/// - sc_sutta IS NULL OR sc_sutta = ''
///
/// If a pali_titles cache is provided, auto-fix suggestions will be generated
/// for fragments whose sc_code (base part) matches a title in the cache.
pub fn check_missing_sc_sutta(
    conn: &mut SqliteConnection,
    pali_titles: Option<&HashMap<String, String>>,
) -> ValidationCheckResult {
    let results: Vec<(i32, String, i32, Option<String>, Option<String>)> = xml_fragments::table
        .select((
            xml_fragments::id,
            xml_fragments::cst_file,
            xml_fragments::frag_idx,
            xml_fragments::sc_code,
            xml_fragments::sc_sutta,
        ))
        .filter(xml_fragments::sc_code.is_not_null())
        .filter(xml_fragments::sc_code.ne(""))
        .filter(xml_fragments::sc_sutta.is_null().or(xml_fragments::sc_sutta.eq("")))
        .load(conn)
        .unwrap_or_default();

    let mut errors = Vec::new();
    let mut auto_fixes = Vec::new();

    for (id, cst_file, frag_idx, sc_code_opt, _) in results {
        let sc_code = sc_code_opt.unwrap_or_default();

        errors.push(ValidationError {
            cst_file: cst_file.clone(),
            frag_idx,
            fragment_id: id,
            message: format!("Fragment with sc_code '{}' is missing sc_sutta title", sc_code),
        });

        // Try to find auto-fix from pali_titles cache
        if let Some(titles) = pali_titles {
            // Extract base sc_code (e.g., "dn1" from "dn1:1.1")
            let base_code = sc_code.split(':').next().unwrap_or(&sc_code);

            if let Some(title) = titles.get(base_code) {
                auto_fixes.push(AutoFix {
                    fragment_id: id,
                    cst_file,
                    frag_idx,
                    sc_code: sc_code.clone(),
                    suggested_value: title.clone(),
                });
            }
        }
    }

    let has_titles = pali_titles.is_some();

    ValidationCheckResult {
        name: "Missing sc_sutta".to_string(),
        description: "Fragments with sc_code but no sc_sutta title".to_string(),
        auto_fixable: has_titles,
        errors,
        auto_fixes,
    }
}

/// Run all validation checks and return results
///
/// Returns a HashMap where keys are check identifiers and values are the results.
pub fn run_all_validations(
    conn: &mut SqliteConnection,
    pali_titles: Option<&HashMap<String, String>>,
) -> HashMap<String, ValidationCheckResult> {
    let mut results = HashMap::new();

    results.insert(
        "missing_sc_code".to_string(),
        check_missing_sc_code(conn),
    );

    results.insert(
        "missing_sc_sutta".to_string(),
        check_missing_sc_sutta(conn, pali_titles),
    );

    results
}
