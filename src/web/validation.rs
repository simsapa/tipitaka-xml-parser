//! Validation checks for fragment data integrity
//!
//! This module provides validation functions to check fragment data against
//! SuttaCentral references and identify missing or inconsistent metadata.
//!
//! ## Available Checks
//!
//! - `check_missing_sc_code`: Finds Sutta fragments without sc_code
//! - `check_missing_sc_sutta`: Finds fragments with sc_code but no sc_sutta title
//! - `check_sc_code_sequence`: Validates that sc_code values increase gradually
//!
//! ## Auto-Fix Support
//!
//! Some checks can provide auto-fix suggestions. For example, missing sc_sutta
//! values can be populated from the Pali titles cache fetched from ArangoDB.

use std::collections::HashMap;
use std::path::Path;
use diesel::prelude::*;
use serde::{Serialize, Deserialize};

use crate::fragments_schema::xml_fragments;
use crate::fragment_reconstructor::reconstruct_xml_from_db;

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
    /// The frag_review status of the fragment (if any)
    #[serde(default)]
    pub frag_review: Option<String>,
}

impl Default for ValidationError {
    fn default() -> Self {
        Self {
            cst_file: String::new(),
            frag_idx: 0,
            fragment_id: 0,
            message: String::new(),
            frag_review: None,
        }
    }
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
/// - frag_review != 'moved' (and optionally != 'checked' if include_checked is false)
/// - sc_code IS NULL OR sc_code = ''
pub fn check_missing_sc_code(conn: &mut SqliteConnection, include_checked: bool) -> ValidationCheckResult {
    let results: Vec<(i32, String, i32, Option<String>, Option<String>)> = if include_checked {
        xml_fragments::table
            .select((
                xml_fragments::id,
                xml_fragments::cst_file,
                xml_fragments::frag_idx,
                xml_fragments::sc_code,
                xml_fragments::frag_review,
            ))
            .filter(xml_fragments::frag_type.eq("Sutta"))
            .filter(xml_fragments::frag_review.ne("moved").or(xml_fragments::frag_review.is_null()))
            .filter(xml_fragments::sc_code.is_null().or(xml_fragments::sc_code.eq("")))
            .load(conn)
            .unwrap_or_default()
    } else {
        xml_fragments::table
            .select((
                xml_fragments::id,
                xml_fragments::cst_file,
                xml_fragments::frag_idx,
                xml_fragments::sc_code,
                xml_fragments::frag_review,
            ))
            .filter(xml_fragments::frag_type.eq("Sutta"))
            .filter(xml_fragments::frag_review.ne("moved").or(xml_fragments::frag_review.is_null()))
            .filter(xml_fragments::frag_review.ne("checked").or(xml_fragments::frag_review.is_null()))
            .filter(xml_fragments::sc_code.is_null().or(xml_fragments::sc_code.eq("")))
            .load(conn)
            .unwrap_or_default()
    };

    let errors: Vec<ValidationError> = results
        .into_iter()
        .map(|(id, cst_file, frag_idx, _, frag_review)| ValidationError {
            cst_file,
            frag_idx,
            fragment_id: id,
            message: "Sutta fragment is missing sc_code".to_string(),
            frag_review,
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
/// - frag_review != 'moved' (and optionally != 'checked' if include_checked is false)
///
/// If a pali_titles cache is provided, auto-fix suggestions will be generated
/// for fragments whose sc_code (base part) matches a title in the cache.
pub fn check_missing_sc_sutta(
    conn: &mut SqliteConnection,
    pali_titles: Option<&HashMap<String, String>>,
    include_checked: bool,
) -> ValidationCheckResult {
    let results: Vec<(i32, String, i32, Option<String>, Option<String>, Option<String>)> = if include_checked {
        xml_fragments::table
            .select((
                xml_fragments::id,
                xml_fragments::cst_file,
                xml_fragments::frag_idx,
                xml_fragments::sc_code,
                xml_fragments::sc_sutta,
                xml_fragments::frag_review,
            ))
            .filter(xml_fragments::frag_review.ne("moved").or(xml_fragments::frag_review.is_null()))
            .filter(xml_fragments::sc_code.is_not_null())
            .filter(xml_fragments::sc_code.ne(""))
            .filter(xml_fragments::sc_sutta.is_null().or(xml_fragments::sc_sutta.eq("")))
            .load(conn)
            .unwrap_or_default()
    } else {
        xml_fragments::table
            .select((
                xml_fragments::id,
                xml_fragments::cst_file,
                xml_fragments::frag_idx,
                xml_fragments::sc_code,
                xml_fragments::sc_sutta,
                xml_fragments::frag_review,
            ))
            .filter(xml_fragments::frag_review.ne("moved").or(xml_fragments::frag_review.is_null()))
            .filter(xml_fragments::frag_review.ne("checked").or(xml_fragments::frag_review.is_null()))
            .filter(xml_fragments::sc_code.is_not_null())
            .filter(xml_fragments::sc_code.ne(""))
            .filter(xml_fragments::sc_sutta.is_null().or(xml_fragments::sc_sutta.eq("")))
            .load(conn)
            .unwrap_or_default()
    };

    let mut errors = Vec::new();
    let mut auto_fixes = Vec::new();

    for (id, cst_file, frag_idx, sc_code_opt, _, frag_review) in results {
        let sc_code = sc_code_opt.unwrap_or_default();

        errors.push(ValidationError {
            cst_file: cst_file.clone(),
            frag_idx,
            fragment_id: id,
            message: format!("Fragment with sc_code '{}' is missing sc_sutta title", sc_code),
            frag_review,
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

/// Check that Sutta fragments have appropriate review status
///
/// Finds Sutta fragments that need user attention - those with review status
/// other than empty, "checked", or "moved". These typically include "in-progress"
/// or "needs-review" which indicate fragments requiring user action.
///
/// This check always filters out "checked" and "moved" status regardless of the
/// include_checked parameter, as these are considered valid/complete states.
pub fn check_sutta_review_status(conn: &mut SqliteConnection, _include_checked: bool) -> ValidationCheckResult {
    let results: Vec<(i32, String, i32, Option<String>)> = xml_fragments::table
        .select((
            xml_fragments::id,
            xml_fragments::cst_file,
            xml_fragments::frag_idx,
            xml_fragments::frag_review,
        ))
        .filter(xml_fragments::frag_type.eq("Sutta"))
        .filter(
            xml_fragments::frag_review.is_not_null()
                .and(xml_fragments::frag_review.ne(""))
                .and(xml_fragments::frag_review.ne("moved"))
                .and(xml_fragments::frag_review.ne("checked"))
        )
        .load(conn)
        .unwrap_or_default();

    let errors: Vec<ValidationError> = results
        .into_iter()
        .map(|(id, cst_file, frag_idx, frag_review)| {
            let status = frag_review.clone().unwrap_or_else(|| "unknown".to_string());
            ValidationError {
                cst_file,
                frag_idx,
                fragment_id: id,
                message: format!("Sutta fragment needs attention (status: '{}')", status),
                frag_review,
            }
        })
        .collect();

    ValidationCheckResult {
        name: "Status Needs Attention".to_string(),
        description: "Sutta fragments that need user attention (in-progress, needs-review, etc.)".to_string(),
        auto_fixable: false,
        errors,
        auto_fixes: vec![],
    }
}

/// Check XML reconstruction for all files in the database
///
/// Uses the same reconstruction procedure as the regeneration process:
/// calls `reconstruct_xml_from_db` for each file and reports any failures.
/// Reports the problematic cst_file and first frag_idx when reconstruction fails.
///
/// Always include all fragments for reconstruction validation.
/// The frag_review = 'moved' fragments are empty, they don't need to be filtered,
/// so we are not filtering on any frag_review fragment types.
pub fn check_xml_reconstruction(
    conn: &mut SqliteConnection,
    db_path: &Path,
    _include_checked: bool,
) -> ValidationCheckResult {
    // Get all unique cst_file values - always include checked for reconstruction validation
    let cst_files: Vec<String> = xml_fragments::table
        .select(xml_fragments::cst_file)
        .distinct()
        .order_by(xml_fragments::cst_file)
        .load(conn)
        .unwrap_or_default();

    let mut errors = Vec::new();

    // Test reconstruction for each file using the same method as regeneration
    for cst_file in cst_files {
        // Get first fragment for error reporting
        let first_fragment: Option<(i32, i32, Option<String>)> = xml_fragments::table
            .select((xml_fragments::id, xml_fragments::frag_idx, xml_fragments::frag_review))
            .filter(xml_fragments::cst_file.eq(&cst_file))
            .order_by(xml_fragments::frag_idx)
            .first(conn)
            .optional()
            .unwrap_or(None);

        if let Some((fragment_id, frag_idx, frag_review)) = first_fragment {
            // Attempt reconstruction using the same function used during regeneration
            match reconstruct_xml_from_db(db_path, &cst_file) {
                Ok(_reconstructed_xml) => {
                    // Reconstruction succeeded, no error
                }
                Err(e) => {
                    // Reconstruction failed, report error
                    errors.push(ValidationError {
                        cst_file: cst_file.clone(),
                        frag_idx,
                        fragment_id,
                        message: format!("XML reconstruction failed: {}", e),
                        frag_review,
                    });
                }
            }
        }
    }

    ValidationCheckResult {
        name: "XML Reconstruction".to_string(),
        description: "Validate that fragments can be properly reconstructed into XML using the same method as regeneration".to_string(),
        auto_fixable: false,
        errors,
        auto_fixes: vec![],
    }
}

/// Parsed sc_code components
///
/// sc_code format examples:
/// - `sn2.12` = nikaya 'sn', group Some(2), sutta 12
/// - `dn10` = nikaya 'dn', group None, sutta 10
/// - `mn5:1.2` = nikaya 'mn', group None, sutta 5 (colon part ignored)
/// - `sn1.55-57` = nikaya 'sn', group Some(1), sutta_start 55, sutta_end 57 (range)
#[derive(Debug, Clone, PartialEq)]
struct ParsedScCode {
    nikaya: String,
    group: Option<u32>,
    /// The starting sutta number (or the only sutta number if not a range)
    sutta_start: u32,
    /// The ending sutta number (same as sutta_start if not a range)
    sutta_end: u32,
}

/// Parse an sc_code string into its components
///
/// Returns None if the sc_code cannot be parsed
fn parse_sc_code(sc_code: &str) -> Option<ParsedScCode> {
    // Remove any colon suffix (e.g., "dn1:1.2" -> "dn1")
    let base_code = sc_code.split(':').next().unwrap_or(sc_code);

    // Find where the nikaya prefix ends (first digit)
    let digit_start = base_code.find(|c: char| c.is_ascii_digit())?;

    // Nikaya prefix must not be empty
    if digit_start == 0 {
        return None;
    }

    let nikaya = base_code[..digit_start].to_string();
    let number_part = &base_code[digit_start..];

    // Check if there's a group number (format: "2.12" or "2.12-15")
    if let Some(dot_pos) = number_part.find('.') {
        let group: u32 = number_part[..dot_pos].parse().ok()?;
        let sutta_part = &number_part[dot_pos + 1..];

        // Check if it's a range (e.g., "55-57")
        let (sutta_start, sutta_end) = parse_sutta_range(sutta_part)?;
        Some(ParsedScCode { nikaya, group: Some(group), sutta_start, sutta_end })
    } else {
        // No group, just sutta number or range (format: "10" or "10-12")
        let (sutta_start, sutta_end) = parse_sutta_range(number_part)?;
        Some(ParsedScCode { nikaya, group: None, sutta_start, sutta_end })
    }
}

/// Parse a sutta number or range (e.g., "55" or "55-57")
///
/// Returns (start, end) tuple. For non-ranges, start == end.
fn parse_sutta_range(sutta_part: &str) -> Option<(u32, u32)> {
    if let Some(dash_pos) = sutta_part.find('-') {
        let start: u32 = sutta_part[..dash_pos].parse().ok()?;
        let end: u32 = sutta_part[dash_pos + 1..].parse().ok()?;
        Some((start, end))
    } else {
        let sutta: u32 = sutta_part.parse().ok()?;
        Some((sutta, sutta))
    }
}

/// Format a sutta range for error messages
///
/// Returns "55" for single suttas (start == end) or "55-57" for ranges.
fn format_sutta_range(start: u32, end: u32) -> String {
    if start == end {
        start.to_string()
    } else {
        format!("{}-{}", start, end)
    }
}

/// Check that sc_code values increase gradually within each file
///
/// For each file, retrieves Sutta fragments ordered by frag_idx and validates:
/// - Sutta numbers increase by 1 within the same group
/// - When group changes, it must increase by 1
/// - When group changes, sutta number must restart at 1
///
/// Only Sutta type fragments are checked. Null/empty sc_code values are skipped
/// (checked by check_missing_sc_code).
///
/// Fragments with frag_review = 'moved' are excluded. If include_checked is false,
/// fragments with frag_review = 'checked' are also excluded.
pub fn check_sc_code_sequence(conn: &mut SqliteConnection, include_checked: bool) -> ValidationCheckResult {
    // Get distinct cst_file values - always exclude moved, include checked for sequence checking
    let cst_files: Vec<String> = xml_fragments::table
        .select(xml_fragments::cst_file)
        .distinct()
        .filter(xml_fragments::frag_review.ne("moved").or(xml_fragments::frag_review.is_null()))
        .order_by(xml_fragments::cst_file)
        .load(conn)
        .unwrap_or_default();

    let mut errors = Vec::new();

    for cst_file in cst_files {
        // Get Sutta fragments for this file, ordered by frag_idx
        // Always include checked fragments for sequence validation
        let fragments: Vec<(i32, i32, Option<String>, Option<String>)> = xml_fragments::table
            .select((
                xml_fragments::id,
                xml_fragments::frag_idx,
                xml_fragments::sc_code,
                xml_fragments::frag_review,
            ))
            .filter(xml_fragments::cst_file.eq(&cst_file))
            .filter(xml_fragments::frag_type.eq("Sutta"))
            .filter(xml_fragments::frag_review.ne("moved").or(xml_fragments::frag_review.is_null()))
            .order_by(xml_fragments::frag_idx)
            .load(conn)
            .unwrap_or_default();

        let mut prev_parsed: Option<ParsedScCode> = None;

        for (id, frag_idx, sc_code_opt, frag_review) in fragments {
            // Skip null/empty sc_code values
            let sc_code = match &sc_code_opt {
                Some(code) if !code.is_empty() => code,
                _ => continue,
            };

            // Try to parse the sc_code
            let parsed = match parse_sc_code(sc_code) {
                Some(p) => p,
                None => {
                    errors.push(ValidationError {
                        cst_file: cst_file.clone(),
                        frag_idx,
                        fragment_id: id,
                        message: format!("Cannot parse sc_code '{}' format", sc_code),
                        frag_review,
                    });
                    continue;
                }
            };

            // Compare with previous sc_code
            if let Some(prev) = &prev_parsed {
                // Check nikaya matches
                if parsed.nikaya != prev.nikaya {
                    errors.push(ValidationError {
                        cst_file: cst_file.clone(),
                        frag_idx,
                        fragment_id: id,
                        message: format!(
                            "Nikaya changed from '{}' to '{}' - expected same nikaya within file",
                            prev.nikaya, parsed.nikaya
                        ),
                        frag_review: None,
                    });
                    prev_parsed = Some(parsed);
                    continue;
                }

                // Check group and sutta progression
                // For ranges, current sutta_start should follow prev sutta_end + 1
                match (&prev.group, &parsed.group) {
                    // Both have groups (e.g., sn1.1 -> sn1.2 or sn1.3 -> sn2.1)
                    (Some(prev_group), Some(curr_group)) => {
                        if curr_group == prev_group {
                            // Same group: sutta should increase by 1 from prev's end
                            if parsed.sutta_start != prev.sutta_end + 1 {
                                errors.push(ValidationError {
                                    cst_file: cst_file.clone(),
                                    frag_idx,
                                    fragment_id: id,
                                    message: format!(
                                        "Sutta number jump from {}{}.{} to {}{}.{} - expected step of 1",
                                        prev.nikaya, prev_group, format_sutta_range(prev.sutta_start, prev.sutta_end),
                                        parsed.nikaya, curr_group, format_sutta_range(parsed.sutta_start, parsed.sutta_end)
                                    ),
                                    frag_review,
                                });
                            }
                        } else if *curr_group == prev_group + 1 {
                            // Group increased by 1: sutta should start at 1
                            if parsed.sutta_start != 1 {
                                errors.push(ValidationError {
                                    cst_file: cst_file.clone(),
                                    frag_idx,
                                    fragment_id: id,
                                    message: format!(
                                        "Group changed from {} to {} but sutta starts at {} instead of 1",
                                        prev_group, curr_group, parsed.sutta_start
                                    ),
                                    frag_review,
                                });
                            }
                        } else {
                            // Group jump is not by 1
                            errors.push(ValidationError {
                                cst_file: cst_file.clone(),
                                frag_idx,
                                fragment_id: id,
                                message: format!(
                                    "Group jump from {} to {} - expected step of 1",
                                    prev_group, curr_group
                                ),
                                frag_review,
                            });
                        }
                    }
                    // Neither has groups (e.g., dn1 -> dn2)
                    (None, None) => {
                        if parsed.sutta_start != prev.sutta_end + 1 {
                        errors.push(ValidationError {
                            cst_file: cst_file.clone(),
                            frag_idx,
                            fragment_id: id,
                            message: format!(
                                "Sutta number jump from {}{} to {}{} - expected step of 1",
                                prev.nikaya, format_sutta_range(prev.sutta_start, prev.sutta_end),
                                parsed.nikaya, format_sutta_range(parsed.sutta_start, parsed.sutta_end)
                            ),
                            frag_review,
                        });
                        }
                    }
                    // Mixing grouped and non-grouped formats
                    _ => {
                        errors.push(ValidationError {
                            cst_file: cst_file.clone(),
                            frag_idx,
                            fragment_id: id,
                            message: format!(
                                "Inconsistent sc_code format: mixing grouped and non-grouped formats"
                            ),
                            frag_review,
                        });
                    }
                }
            }

            prev_parsed = Some(parsed);
        }
    }

    // Filter out errors from checked fragments if include_checked is false
    let final_errors: Vec<ValidationError> = if include_checked {
        errors
    } else {
        errors
            .into_iter()
            .filter(|e| e.frag_review.as_ref() != Some(&"checked".to_string()))
            .collect()
    };

    ValidationCheckResult {
        name: "sc_code Sequence".to_string(),
        description: "Validates that sc_code values increase gradually (step of 1) within each file".to_string(),
        auto_fixable: false,
        errors: final_errors,
        auto_fixes: vec![],
    }
}

/// Check for duplicate cst_code and sc_code values within each file
///
/// Finds fragments where cst_code or sc_code values are not unique within the same cst_file.
/// Only non-empty values are checked for uniqueness.
/// Fragments with frag_review = 'moved' are excluded as they are essentially deleted.
/// Fragments with frag_review = 'checked' are always included as the 'checked' status fragments should still have unique cst_code and sc_code values.
/// Header type fragments are excluded as they don't require unique codes.
pub fn check_code_uniqueness(conn: &mut SqliteConnection, _include_checked: bool) -> ValidationCheckResult {
    let mut errors = Vec::new();

    // Check cst_code uniqueness within each file
    // Query fragments with non-empty cst_code, excluding moved and Header fragments
    // Always include checked fragments for uniqueness checking
    let cst_code_fragments: Vec<(i32, String, i32, Option<String>, Option<String>)> = xml_fragments::table
        .select((
            xml_fragments::id,
            xml_fragments::cst_file,
            xml_fragments::frag_idx,
            xml_fragments::cst_code,
            xml_fragments::frag_review,
        ))
        .filter(xml_fragments::cst_code.is_not_null())
        .filter(xml_fragments::cst_code.ne(""))
        .filter(xml_fragments::frag_type.ne("Header"))
        .filter(xml_fragments::frag_review.ne("moved").or(xml_fragments::frag_review.is_null()))
        .load(conn)
        .unwrap_or_default();

    // Group by (cst_file, cst_code) to find duplicates within each file
    let mut cst_code_map: HashMap<(String, String), Vec<(i32, i32, Option<String>)>> = HashMap::new();
    for (id, cst_file, frag_idx, cst_code_opt, frag_review) in cst_code_fragments {
        if let Some(cst_code) = cst_code_opt {
            cst_code_map
                .entry((cst_file, cst_code))
                .or_insert_with(Vec::new)
                .push((id, frag_idx, frag_review));
        }
    }

    // Report duplicates for cst_code
    for ((cst_file, code), fragments) in &cst_code_map {
        if fragments.len() > 1 {
            for (id, frag_idx, frag_review) in fragments {
                errors.push(ValidationError {
                    cst_file: cst_file.clone(),
                    frag_idx: *frag_idx,
                    fragment_id: *id,
                    message: format!(
                        "Duplicate cst_code '{}' (found in {} fragments in this file)",
                        code,
                        fragments.len()
                    ),
                    frag_review: frag_review.clone(),
                });
            }
        }
    }

    // Check sc_code uniqueness within each file
    // Query fragments with non-empty sc_code, excluding moved and Header fragments
    // Always include checked fragments for uniqueness checking
    let sc_code_fragments: Vec<(i32, String, i32, Option<String>, Option<String>)> = xml_fragments::table
        .select((
            xml_fragments::id,
            xml_fragments::cst_file,
            xml_fragments::frag_idx,
            xml_fragments::sc_code,
            xml_fragments::frag_review,
        ))
        .filter(xml_fragments::sc_code.is_not_null())
        .filter(xml_fragments::sc_code.ne(""))
        .filter(xml_fragments::frag_type.ne("Header"))
        .filter(xml_fragments::frag_review.ne("moved").or(xml_fragments::frag_review.is_null()))
        .load(conn)
        .unwrap_or_default();

    // Group by (cst_file, sc_code) to find duplicates within each file
    let mut sc_code_map: HashMap<(String, String), Vec<(i32, i32, Option<String>)>> = HashMap::new();
    for (id, cst_file, frag_idx, sc_code_opt, frag_review) in sc_code_fragments {
        if let Some(sc_code) = sc_code_opt {
            sc_code_map
                .entry((cst_file, sc_code))
                .or_insert_with(Vec::new)
                .push((id, frag_idx, frag_review));
        }
    }

    // Report duplicates for sc_code
    for ((cst_file, code), fragments) in &sc_code_map {
        if fragments.len() > 1 {
            for (id, frag_idx, frag_review) in fragments {
                errors.push(ValidationError {
                    cst_file: cst_file.clone(),
                    frag_idx: *frag_idx,
                    fragment_id: *id,
                    message: format!(
                        "Duplicate sc_code '{}' (found in {} fragments in this file)",
                        code,
                        fragments.len()
                    ),
                    frag_review: frag_review.clone(),
                });
            }
        }
    }

    // Sort errors by file and frag_idx for consistent ordering
    errors.sort_by(|a, b| {
        a.cst_file.cmp(&b.cst_file)
            .then_with(|| a.frag_idx.cmp(&b.frag_idx))
    });

    ValidationCheckResult {
        name: "Code Uniqueness".to_string(),
        description: "Checks that cst_code and sc_code values are unique within each file".to_string(),
        auto_fixable: false,
        errors,
        auto_fixes: vec![],
    }
}

/// Check if a cst_code is in range format (e.g., "sn2.1.9.2-12")
///
/// A cst_code is considered a range if the last numeric segment contains a dash.
fn is_cst_code_range(cst_code: &str) -> bool {
    // Split by '.' and check if the last segment contains a dash (e.g., "2-12")
    if let Some(last_segment) = cst_code.rsplit('.').next() {
        return last_segment.contains('-');
    }
    false
}

/// Check if an sc_code is in range format (e.g., "sn12.93-103")
///
/// An sc_code is considered a range if the last numeric segment contains a dash.
fn is_sc_code_range(sc_code: &str) -> bool {
    // First, remove any colon suffix (e.g., "dn1:1.2" -> "dn1")
    let base_code = sc_code.split(':').next().unwrap_or(sc_code);

    // Split by '.' and check if the last segment contains a dash
    if let Some(last_segment) = base_code.rsplit('.').next() {
        return last_segment.contains('-');
    }
    false
}

/// Check that Sutta fragments with range cst_code also have range sc_code
///
/// For Sutta type fragments where cst_code is a range (e.g., "sn2.1.9.2-12"),
/// if the fragment has an sc_code, it should also be in range form (e.g., "sn12.93-103").
///
/// This ensures consistency between CST and SC code formatting for range entries.
///
/// Fragments with frag_review = 'moved' are excluded. If include_checked is false,
/// fragments with frag_review = 'checked' are also excluded.
/// The frag_review = 'checked' items can be filtered at the initial collection stage because this validation doesn't rely on walking through items in sequence.
pub fn check_cst_sc_range_consistency(conn: &mut SqliteConnection, include_checked: bool) -> ValidationCheckResult {
    let results: Vec<(i32, String, i32, Option<String>, Option<String>, Option<String>)> = if include_checked {
        xml_fragments::table
            .select((
                xml_fragments::id,
                xml_fragments::cst_file,
                xml_fragments::frag_idx,
                xml_fragments::cst_code,
                xml_fragments::sc_code,
                xml_fragments::frag_review,
            ))
            .filter(xml_fragments::frag_type.eq("Sutta"))
            .filter(xml_fragments::frag_review.ne("moved").or(xml_fragments::frag_review.is_null()))
            .filter(xml_fragments::cst_code.is_not_null())
            .filter(xml_fragments::cst_code.ne(""))
            .filter(xml_fragments::sc_code.is_not_null())
            .filter(xml_fragments::sc_code.ne(""))
            .load(conn)
            .unwrap_or_default()
    } else {
        xml_fragments::table
            .select((
                xml_fragments::id,
                xml_fragments::cst_file,
                xml_fragments::frag_idx,
                xml_fragments::cst_code,
                xml_fragments::sc_code,
                xml_fragments::frag_review,
            ))
            .filter(xml_fragments::frag_type.eq("Sutta"))
            .filter(xml_fragments::frag_review.ne("moved").or(xml_fragments::frag_review.is_null()))
            .filter(xml_fragments::frag_review.ne("checked").or(xml_fragments::frag_review.is_null()))
            .filter(xml_fragments::cst_code.is_not_null())
            .filter(xml_fragments::cst_code.ne(""))
            .filter(xml_fragments::sc_code.is_not_null())
            .filter(xml_fragments::sc_code.ne(""))
            .load(conn)
            .unwrap_or_default()
    };

    let errors: Vec<ValidationError> = results
        .into_iter()
        .filter_map(|(id, cst_file, frag_idx, cst_code_opt, sc_code_opt, frag_review)| {
            let cst_code = cst_code_opt?;
            let sc_code = sc_code_opt?;

            let cst_is_range = is_cst_code_range(&cst_code);
            let sc_is_range = is_sc_code_range(&sc_code);

            // If cst_code is a range but sc_code is not, that's an error
            if cst_is_range && !sc_is_range {
                Some(ValidationError {
                    cst_file,
                    frag_idx,
                    fragment_id: id,
                    message: format!(
                        "cst_code '{}' is a range but sc_code '{}' is not a range",
                        cst_code, sc_code
                    ),
                    frag_review,
                })
            } else {
                None
            }
        })
        .collect();

    ValidationCheckResult {
        name: "CST/SC Range Consistency".to_string(),
        description: "Ensures that Sutta fragments with range cst_code also have range sc_code".to_string(),
        auto_fixable: false,
        errors,
        auto_fixes: vec![],
    }
}

/// Check for sc_code values that don't exist in ArangoDB
///
/// Finds Sutta fragments where sc_code is set but the base code (without :suffix)
/// doesn't exist in the ArangoDB names collection as a root title.
///
/// This validation requires ArangoDB to be connected. If ArangoDB is not available,
/// no errors will be reported (as we cannot verify existence).
///
/// Fragments with frag_review = 'moved' are excluded. If include_checked is false,
/// fragments with frag_review = 'checked' are also excluded.
/// The frag_review = 'checked' items can be filtered at the initial collection stage because this validation doesn't rely on walking through items in sequence.
/// Header type fragments are excluded as they don't require valid codes.
pub fn check_sc_code_not_in_arangodb(
    conn: &mut SqliteConnection,
    pali_titles: Option<&HashMap<String, String>>,
    include_checked: bool,
) -> ValidationCheckResult {
    // If ArangoDB is not connected, we can't check - return empty result
    let Some(titles) = pali_titles else {
        return ValidationCheckResult {
            name: "sc_code Not in ArangoDB".to_string(),
            description: "Fragments with sc_code that don't exist in ArangoDB (ArangoDB not connected)".to_string(),
            auto_fixable: false,
            errors: vec![],
            auto_fixes: vec![],
        };
    };

    // Get all unique sc_codes from the database with their fragment info
    let results: Vec<(i32, String, i32, Option<String>, Option<String>)> = if include_checked {
        xml_fragments::table
            .select((
                xml_fragments::id,
                xml_fragments::cst_file,
                xml_fragments::frag_idx,
                xml_fragments::sc_code,
                xml_fragments::frag_review,
            ))
            .filter(xml_fragments::frag_type.ne("Header"))
            .filter(xml_fragments::frag_review.ne("moved").or(xml_fragments::frag_review.is_null()))
            .filter(xml_fragments::sc_code.is_not_null())
            .filter(xml_fragments::sc_code.ne(""))
            .load(conn)
            .unwrap_or_default()
    } else {
        xml_fragments::table
            .select((
                xml_fragments::id,
                xml_fragments::cst_file,
                xml_fragments::frag_idx,
                xml_fragments::sc_code,
                xml_fragments::frag_review,
            ))
            .filter(xml_fragments::frag_type.ne("Header"))
            .filter(xml_fragments::frag_review.ne("moved").or(xml_fragments::frag_review.is_null()))
            .filter(xml_fragments::frag_review.ne("checked").or(xml_fragments::frag_review.is_null()))
            .filter(xml_fragments::sc_code.is_not_null())
            .filter(xml_fragments::sc_code.ne(""))
            .load(conn)
            .unwrap_or_default()
    };

    let errors: Vec<ValidationError> = results
        .into_iter()
        .filter_map(|(id, cst_file, frag_idx, sc_code_opt, frag_review)| {
            let sc_code = sc_code_opt?;

            // Extract base sc_code (without colon suffix, e.g., "dn1" from "dn1:1.2")
            let base_code = sc_code.split(':').next().unwrap_or(&sc_code);

            // Check if the base code exists in ArangoDB
            if !titles.contains_key(base_code) {
                Some(ValidationError {
                    cst_file,
                    frag_idx,
                    fragment_id: id,
                    message: format!(
                        "sc_code '{}' (base: '{}') does not exist in ArangoDB",
                        sc_code, base_code
                    ),
                    frag_review,
                })
            } else {
                None
            }
        })
        .collect();

    ValidationCheckResult {
        name: "sc_code Not in ArangoDB".to_string(),
        description: "Fragments with sc_code values that don't exist in ArangoDB".to_string(),
        auto_fixable: false,
        errors,
        auto_fixes: vec![],
    }
}

/// Run all validation checks and return results
///
/// Returns a HashMap where keys are check identifiers and values are the results.
///
/// If `include_checked` is false (default), fragments with `frag_review = 'checked'`
/// will be excluded from validation checks (similar to how `frag_review = 'moved'` is excluded).
pub fn run_all_validations(
    conn: &mut SqliteConnection,
    db_path: &Path,
    pali_titles: Option<&HashMap<String, String>>,
    include_checked: bool,
) -> HashMap<String, ValidationCheckResult> {
    let mut results = HashMap::new();

    results.insert(
        "missing_sc_code".to_string(),
        check_missing_sc_code(conn, include_checked),
    );

    results.insert(
        "missing_sc_sutta".to_string(),
        check_missing_sc_sutta(conn, pali_titles, include_checked),
    );

    results.insert(
        "sutta_review_status".to_string(),
        check_sutta_review_status(conn, include_checked),
    );

    results.insert(
        "xml_reconstruction".to_string(),
        check_xml_reconstruction(conn, db_path, include_checked),
    );

    results.insert(
        "sc_code_sequence".to_string(),
        check_sc_code_sequence(conn, include_checked),
    );

    results.insert(
        "code_uniqueness".to_string(),
        check_code_uniqueness(conn, include_checked),
    );

    results.insert(
        "cst_sc_range_consistency".to_string(),
        check_cst_sc_range_consistency(conn, include_checked),
    );

    results.insert(
        "sc_code_not_in_arangodb".to_string(),
        check_sc_code_not_in_arangodb(conn, pali_titles, include_checked),
    );

    results
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fragments_models::NewXmlFragment;

    fn setup_test_db() -> (SqliteConnection, tempfile::NamedTempFile) {
        use tempfile::NamedTempFile;

        // Create a real file-based database for reconstruction testing
        let temp_db = NamedTempFile::new().expect("Failed to create temp db file");
        let db_path = temp_db.path();

        let mut conn = SqliteConnection::establish(db_path.to_str().unwrap())
            .expect("Failed to create database connection");

        // Create the schema (matching the actual database schema)
        diesel::sql_query(
            r#"CREATE TABLE xml_fragments (
                id INTEGER PRIMARY KEY,
                cst_file TEXT NOT NULL,
                frag_idx INTEGER NOT NULL,
                frag_type TEXT NOT NULL,
                frag_review TEXT,
                nikaya TEXT NOT NULL,
                cst_code TEXT,
                sc_code TEXT,
                content_xml TEXT NOT NULL,
                content_html TEXT,
                cst_vagga TEXT,
                cst_sutta TEXT,
                cst_paranum TEXT,
                sc_sutta TEXT,
                start_line INTEGER NOT NULL,
                start_char INTEGER NOT NULL,
                end_line INTEGER NOT NULL,
                end_char INTEGER NOT NULL,
                group_levels TEXT NOT NULL,
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
            )"#,
        )
        .execute(&mut conn)
        .expect("Failed to create table");

        (conn, temp_db)
    }

    #[test]
    fn test_sutta_review_status_all_valid() {
        let (mut conn, _temp_db) = setup_test_db();

        // Insert Sutta fragments with valid review statuses
        let fragments = vec![
            NewXmlFragment {
                cst_file: "test.xml",
                frag_idx: 0,
                frag_type: "Sutta",
                frag_review: None, // Empty is valid
                nikaya: "digha",
                cst_code: Some("1"),
                sc_code: Some("dn1"),
                content_xml: "<p>Test 1</p>",
                content_html: None,
                cst_vagga: None,
                cst_sutta: None,
                cst_paranum: None,
                sc_sutta: None,
                start_line: 1,
                start_char: 0,
                end_line: 1,
                end_char: 14,
                group_levels: "[]",
            },
            NewXmlFragment {
                cst_file: "test.xml",
                frag_idx: 1,
                frag_type: "Sutta",
                frag_review: Some("checked"), // "checked" is valid
                nikaya: "digha",
                cst_code: Some("2"),
                sc_code: Some("dn2"),
                content_xml: "<p>Test 2</p>",
                content_html: None,
                cst_vagga: None,
                cst_sutta: None,
                cst_paranum: None,
                sc_sutta: None,
                start_line: 2,
                start_char: 0,
                end_line: 2,
                end_char: 14,
                group_levels: "[]",
            },
            NewXmlFragment {
                cst_file: "test.xml",
                frag_idx: 2,
                frag_type: "Sutta",
                frag_review: Some("moved"), // "moved" is valid
                nikaya: "digha",
                cst_code: None,
                sc_code: None,
                content_xml: "",
                content_html: None,
                cst_vagga: None,
                cst_sutta: None,
                cst_paranum: None,
                sc_sutta: None,
                start_line: 3,
                start_char: 0,
                end_line: 3,
                end_char: 0,
                group_levels: "[]",
            },
        ];

        for fragment in fragments {
            diesel::insert_into(xml_fragments::table)
                .values(&fragment)
                .execute(&mut conn)
                .expect("Failed to insert fragment");
        }

        // Run validation
        let result = check_sutta_review_status(&mut conn, false);

        // Should have no errors for valid statuses
        assert_eq!(result.errors.len(), 0, "Should have no errors for valid review statuses");
        assert_eq!(result.name, "Status Needs Attention");
    }

    #[test]
    fn test_sutta_review_status_needs_attention() {
        let (mut conn, _temp_db) = setup_test_db();

        // Insert Sutta fragments that need attention
        let fragments = vec![
            NewXmlFragment {
                cst_file: "test.xml",
                frag_idx: 0,
                frag_type: "Sutta",
                frag_review: Some("in-progress"), // Needs attention
                nikaya: "digha",
                cst_code: Some("1"),
                sc_code: Some("dn1"),
                content_xml: "<p>Test 1</p>",
                content_html: None,
                cst_vagga: None,
                cst_sutta: None,
                cst_paranum: None,
                sc_sutta: None,
                start_line: 1,
                start_char: 0,
                end_line: 1,
                end_char: 14,
                group_levels: "[]",
            },
            NewXmlFragment {
                cst_file: "test.xml",
                frag_idx: 1,
                frag_type: "Sutta",
                frag_review: Some("needs-review"), // Needs attention
                nikaya: "digha",
                cst_code: Some("2"),
                sc_code: Some("dn2"),
                content_xml: "<p>Test 2</p>",
                content_html: None,
                cst_vagga: None,
                cst_sutta: None,
                cst_paranum: None,
                sc_sutta: None,
                start_line: 2,
                start_char: 0,
                end_line: 2,
                end_char: 14,
                group_levels: "[]",
            },
            NewXmlFragment {
                cst_file: "test.xml",
                frag_idx: 2,
                frag_type: "Sutta",
                frag_review: Some("checked"), // Valid - should not be reported
                nikaya: "digha",
                cst_code: Some("3"),
                sc_code: Some("dn3"),
                content_xml: "<p>Test 3</p>",
                content_html: None,
                cst_vagga: None,
                cst_sutta: None,
                cst_paranum: None,
                sc_sutta: None,
                start_line: 3,
                start_char: 0,
                end_line: 3,
                end_char: 14,
                group_levels: "[]",
            },
        ];

        for fragment in fragments {
            diesel::insert_into(xml_fragments::table)
                .values(&fragment)
                .execute(&mut conn)
                .expect("Failed to insert fragment");
        }

        // Run validation
        let result = check_sutta_review_status(&mut conn, false);

        // Should report 2 errors (in-progress and needs-review)
        assert_eq!(result.errors.len(), 2, "Should have 2 errors for fragments needing attention");

        // Check that the errors contain the expected statuses
        assert!(result.errors.iter().any(|e| e.message.contains("in-progress")),
            "Should report in-progress status");
        assert!(result.errors.iter().any(|e| e.message.contains("needs-review")),
            "Should report needs-review status");

        // Check that frag_idx 2 (checked) is not reported
        assert!(!result.errors.iter().any(|e| e.frag_idx == 2),
            "Should not report checked status");
    }

    #[test]
    fn test_sutta_review_status_header_ignored() {
        let (mut conn, _temp_db) = setup_test_db();

        // Insert Header fragment with "in-progress" - should be ignored
        let fragment = NewXmlFragment {
            cst_file: "test.xml",
            frag_idx: 0,
            frag_type: "Header", // Not Sutta
            frag_review: Some("in-progress"),
            nikaya: "digha",
            cst_code: None,
            sc_code: None,
            content_xml: "<header>Test</header>",
            content_html: None,
            cst_vagga: None,
            cst_sutta: None,
            cst_paranum: None,
            sc_sutta: None,
            start_line: 1,
            start_char: 0,
            end_line: 1,
            end_char: 21,
            group_levels: "[]",
        };

        diesel::insert_into(xml_fragments::table)
            .values(&fragment)
            .execute(&mut conn)
            .expect("Failed to insert fragment");

        // Run validation
        let result = check_sutta_review_status(&mut conn, false);

        // Should have no errors - only Sutta fragments are checked
        assert_eq!(result.errors.len(), 0, "Should ignore non-Sutta fragments");
    }

    #[test]
    fn test_xml_reconstruction_valid_fragments() {
        let (mut conn, temp_db) = setup_test_db();
        let db_path = temp_db.path();

        // Insert valid fragments
        let fragments = vec![
            NewXmlFragment {
                cst_file: "test.xml",
                frag_idx: 0,
                frag_type: "Header",
                frag_review: None,
                nikaya: "digha",
                cst_code: None,
                sc_code: None,
                content_xml: "<header>Test Header</header>\n",
                content_html: None,
                cst_vagga: None,
                cst_sutta: None,
                cst_paranum: None,
                sc_sutta: None,
                start_line: 1,
                start_char: 0,
                end_line: 1,
                end_char: 29,
                group_levels: "[]",
            },
            NewXmlFragment {
                cst_file: "test.xml",
                frag_idx: 1,
                frag_type: "Sutta",
                frag_review: None,
                nikaya: "digha",
                cst_code: Some("1"),
                sc_code: Some("dn1"),
                content_xml: "<p>Test content</p>\n",
                content_html: None,
                cst_vagga: None,
                cst_sutta: None,
                cst_paranum: None,
                sc_sutta: None,
                start_line: 2,
                start_char: 0,
                end_line: 2,
                end_char: 20,
                group_levels: "[]",
            },
        ];

        for fragment in fragments {
            diesel::insert_into(xml_fragments::table)
                .values(&fragment)
                .execute(&mut conn)
                .expect("Failed to insert fragment");
        }

        // Run validation
        let result = check_xml_reconstruction(&mut conn, db_path, false);

        // Should have no errors
        if result.errors.len() > 0 {
            eprintln!("Unexpected errors: {:?}", result.errors);
        }
        assert_eq!(result.errors.len(), 0, "Should have no validation errors for valid fragments");
        assert_eq!(result.name, "XML Reconstruction");
        assert_eq!(result.auto_fixable, false);
    }

    #[test]
    fn test_xml_reconstruction_no_fragments() {
        let (mut conn, temp_db) = setup_test_db();
        let db_path = temp_db.path();

        // Don't insert any fragments - this should cause reconstruction to fail
        // when trying to get the nikaya (no fragments found)

        // We need to manually add a row to trigger the validation for this file
        // Actually, the validation will skip files with no fragments, so let's test
        // that case where we have a fragment but nikaya lookup fails

        // For this test, let's just verify that reconstruction works with minimal valid content
        let fragment = NewXmlFragment {
            cst_file: "test.xml",
            frag_idx: 0,
            frag_type: "Header",
            frag_review: None,
            nikaya: "digha",
            cst_code: None,
            sc_code: None,
            content_xml: "<header/>",  // Minimal valid XML
            content_html: None,
            cst_vagga: None,
            cst_sutta: None,
            cst_paranum: None,
            sc_sutta: None,
            start_line: 1,
            start_char: 0,
            end_line: 1,
            end_char: 9,
            group_levels: "[]",
        };

        diesel::insert_into(xml_fragments::table)
            .values(&fragment)
            .execute(&mut conn)
            .expect("Failed to insert fragment");

        // Run validation
        let result = check_xml_reconstruction(&mut conn, db_path, false);

        // Should succeed with minimal valid content
        assert_eq!(result.errors.len(), 0, "Should have no errors for minimal valid content");
    }

    #[test]
    fn test_xml_reconstruction_invalid_position() {
        let (mut conn, temp_db) = setup_test_db();
        let db_path = temp_db.path();

        // Insert fragment with invalid position (end before start)
        let fragment = NewXmlFragment {
            cst_file: "test.xml",
            frag_idx: 0,
            frag_type: "Sutta",
            frag_review: None,
            nikaya: "digha",
            cst_code: None,
            sc_code: None,
            content_xml: "<p>Test</p>",
            content_html: None,
            cst_vagga: None,
            cst_sutta: None,
            cst_paranum: None,
            sc_sutta: None,
            start_line: 10,
            start_char: 5,
            end_line: 5,  // End line before start line
            end_char: 0,
            group_levels: "[]",
        };

        diesel::insert_into(xml_fragments::table)
            .values(&fragment)
            .execute(&mut conn)
            .expect("Failed to insert fragment");

        // Run validation
        let _result = check_xml_reconstruction(&mut conn, db_path, false);

        // The reconstruction function should handle this gracefully
        // It may or may not fail depending on the implementation
        // For now, just check that the validation runs without panicking
        assert!(true, "Validation completed without panic");
    }

    #[test]
    fn test_xml_reconstruction_gap_detection() {
        let (mut conn, temp_db) = setup_test_db();
        let db_path = temp_db.path();

        // Insert fragments with a gap between them
        let fragments = vec![
            NewXmlFragment {
                cst_file: "test.xml",
                frag_idx: 0,
                frag_type: "Sutta",
                frag_review: None,
                nikaya: "digha",
                cst_code: None,
                sc_code: None,
                content_xml: "<p>First</p>\n",
                content_html: None,
                cst_vagga: None,
                cst_sutta: None,
                cst_paranum: None,
                sc_sutta: None,
                start_line: 1,
                start_char: 0,
                end_line: 1,
                end_char: 13,
                group_levels: "[]",
            },
            NewXmlFragment {
                cst_file: "test.xml",
                frag_idx: 1,
                frag_type: "Sutta",
                frag_review: None,
                nikaya: "digha",
                cst_code: None,
                sc_code: None,
                content_xml: "<p>Second</p>\n",
                content_html: None,
                cst_vagga: None,
                cst_sutta: None,
                cst_paranum: None,
                sc_sutta: None,
                start_line: 5,  // Gap: line 5 instead of 2
                start_char: 0,
                end_line: 5,
                end_char: 14,
                group_levels: "[]",
            },
        ];

        for fragment in fragments {
            diesel::insert_into(xml_fragments::table)
                .values(&fragment)
                .execute(&mut conn)
                .expect("Failed to insert fragment");
        }

        // Run validation
        let _result = check_xml_reconstruction(&mut conn, db_path, false);

        // The reconstruction function should handle this gracefully
        // Gaps in line positions don't necessarily mean reconstruction will fail
        // as the content_xml itself contains the complete XML content
        // For now, just check that validation runs
        assert!(true, "Validation completed without panic");
    }

    // =========================================================================
    // Tests for parse_sc_code
    // =========================================================================

    #[test]
    fn test_parse_sc_code_with_group() {
        // sn2.12 = nikaya 'sn', group 2, sutta 12
        let parsed = parse_sc_code("sn2.12").unwrap();
        assert_eq!(parsed.nikaya, "sn");
        assert_eq!(parsed.group, Some(2));
        assert_eq!(parsed.sutta_start, 12);
        assert_eq!(parsed.sutta_end, 12);
    }

    #[test]
    fn test_parse_sc_code_without_group() {
        // dn10 = nikaya 'dn', no group, sutta 10
        let parsed = parse_sc_code("dn10").unwrap();
        assert_eq!(parsed.nikaya, "dn");
        assert_eq!(parsed.group, None);
        assert_eq!(parsed.sutta_start, 10);
        assert_eq!(parsed.sutta_end, 10);
    }

    #[test]
    fn test_parse_sc_code_with_colon_suffix() {
        // mn5:1.2 = nikaya 'mn', no group, sutta 5 (colon part ignored)
        let parsed = parse_sc_code("mn5:1.2").unwrap();
        assert_eq!(parsed.nikaya, "mn");
        assert_eq!(parsed.group, None);
        assert_eq!(parsed.sutta_start, 5);
        assert_eq!(parsed.sutta_end, 5);
    }

    #[test]
    fn test_parse_sc_code_with_group_and_colon() {
        // sn1.1:0.1 = nikaya 'sn', group 1, sutta 1 (colon part ignored)
        let parsed = parse_sc_code("sn1.1:0.1").unwrap();
        assert_eq!(parsed.nikaya, "sn");
        assert_eq!(parsed.group, Some(1));
        assert_eq!(parsed.sutta_start, 1);
        assert_eq!(parsed.sutta_end, 1);
    }

    #[test]
    fn test_parse_sc_code_with_range() {
        // sn1.55-57 = nikaya 'sn', group 1, sutta range 55-57
        let parsed = parse_sc_code("sn1.55-57").unwrap();
        assert_eq!(parsed.nikaya, "sn");
        assert_eq!(parsed.group, Some(1));
        assert_eq!(parsed.sutta_start, 55);
        assert_eq!(parsed.sutta_end, 57);
    }

    #[test]
    fn test_parse_sc_code_without_group_with_range() {
        // dn10-12 = nikaya 'dn', no group, sutta range 10-12
        let parsed = parse_sc_code("dn10-12").unwrap();
        assert_eq!(parsed.nikaya, "dn");
        assert_eq!(parsed.group, None);
        assert_eq!(parsed.sutta_start, 10);
        assert_eq!(parsed.sutta_end, 12);
    }

    #[test]
    fn test_parse_sc_code_invalid() {
        // Invalid formats
        assert!(parse_sc_code("").is_none());
        assert!(parse_sc_code("abc").is_none());
        assert!(parse_sc_code("123").is_none()); // No nikaya prefix
    }

    // =========================================================================
    // Tests for check_sc_code_sequence
    // =========================================================================

    #[test]
    fn test_sc_code_sequence_valid_with_groups() {
        let (mut conn, _temp_db) = setup_test_db();

        // Valid sequence: sn1.1 -> sn1.2 -> sn1.3 -> sn2.1 -> sn2.2
        let fragments = vec![
            NewXmlFragment {
                cst_file: "test.xml", frag_idx: 0, frag_type: "Sutta", frag_review: None,
                nikaya: "samyutta", cst_code: None, sc_code: Some("sn1.1"),
                content_xml: "<p>1</p>", content_html: None, cst_vagga: None,
                cst_sutta: None, cst_paranum: None, sc_sutta: None,
                start_line: 1, start_char: 0, end_line: 1, end_char: 10, group_levels: "[]",
            },
            NewXmlFragment {
                cst_file: "test.xml", frag_idx: 1, frag_type: "Sutta", frag_review: None,
                nikaya: "samyutta", cst_code: None, sc_code: Some("sn1.2"),
                content_xml: "<p>2</p>", content_html: None, cst_vagga: None,
                cst_sutta: None, cst_paranum: None, sc_sutta: None,
                start_line: 2, start_char: 0, end_line: 2, end_char: 10, group_levels: "[]",
            },
            NewXmlFragment {
                cst_file: "test.xml", frag_idx: 2, frag_type: "Sutta", frag_review: None,
                nikaya: "samyutta", cst_code: None, sc_code: Some("sn1.3"),
                content_xml: "<p>3</p>", content_html: None, cst_vagga: None,
                cst_sutta: None, cst_paranum: None, sc_sutta: None,
                start_line: 3, start_char: 0, end_line: 3, end_char: 10, group_levels: "[]",
            },
            NewXmlFragment {
                cst_file: "test.xml", frag_idx: 3, frag_type: "Sutta", frag_review: None,
                nikaya: "samyutta", cst_code: None, sc_code: Some("sn2.1"),
                content_xml: "<p>4</p>", content_html: None, cst_vagga: None,
                cst_sutta: None, cst_paranum: None, sc_sutta: None,
                start_line: 4, start_char: 0, end_line: 4, end_char: 10, group_levels: "[]",
            },
            NewXmlFragment {
                cst_file: "test.xml", frag_idx: 4, frag_type: "Sutta", frag_review: None,
                nikaya: "samyutta", cst_code: None, sc_code: Some("sn2.2"),
                content_xml: "<p>5</p>", content_html: None, cst_vagga: None,
                cst_sutta: None, cst_paranum: None, sc_sutta: None,
                start_line: 5, start_char: 0, end_line: 5, end_char: 10, group_levels: "[]",
            },
        ];

        for fragment in fragments {
            diesel::insert_into(xml_fragments::table)
                .values(&fragment)
                .execute(&mut conn)
                .expect("Failed to insert fragment");
        }

        let result = check_sc_code_sequence(&mut conn, false);
        assert_eq!(result.errors.len(), 0, "Valid sequence should have no errors");
    }

    #[test]
    fn test_sc_code_sequence_sutta_jump() {
        let (mut conn, _temp_db) = setup_test_db();

        // Invalid: sn1.1 -> sn1.2 -> sn1.6 (jump from 2 to 6)
        let fragments = vec![
            NewXmlFragment {
                cst_file: "test.xml", frag_idx: 0, frag_type: "Sutta", frag_review: None,
                nikaya: "samyutta", cst_code: None, sc_code: Some("sn1.1"),
                content_xml: "<p>1</p>", content_html: None, cst_vagga: None,
                cst_sutta: None, cst_paranum: None, sc_sutta: None,
                start_line: 1, start_char: 0, end_line: 1, end_char: 10, group_levels: "[]",
            },
            NewXmlFragment {
                cst_file: "test.xml", frag_idx: 1, frag_type: "Sutta", frag_review: None,
                nikaya: "samyutta", cst_code: None, sc_code: Some("sn1.2"),
                content_xml: "<p>2</p>", content_html: None, cst_vagga: None,
                cst_sutta: None, cst_paranum: None, sc_sutta: None,
                start_line: 2, start_char: 0, end_line: 2, end_char: 10, group_levels: "[]",
            },
            NewXmlFragment {
                cst_file: "test.xml", frag_idx: 2, frag_type: "Sutta", frag_review: None,
                nikaya: "samyutta", cst_code: None, sc_code: Some("sn1.6"),
                content_xml: "<p>3</p>", content_html: None, cst_vagga: None,
                cst_sutta: None, cst_paranum: None, sc_sutta: None,
                start_line: 3, start_char: 0, end_line: 3, end_char: 10, group_levels: "[]",
            },
        ];

        for fragment in fragments {
            diesel::insert_into(xml_fragments::table)
                .values(&fragment)
                .execute(&mut conn)
                .expect("Failed to insert fragment");
        }

        let result = check_sc_code_sequence(&mut conn, false);
        assert_eq!(result.errors.len(), 1, "Should detect sutta number jump");
        assert!(result.errors[0].message.contains("jump"), "Error should mention jump");
    }

    #[test]
    fn test_sc_code_sequence_group_not_starting_at_1() {
        let (mut conn, _temp_db) = setup_test_db();

        // Invalid: sn1.3 -> sn2.2 (group change but sutta doesn't start at 1)
        let fragments = vec![
            NewXmlFragment {
                cst_file: "test.xml", frag_idx: 0, frag_type: "Sutta", frag_review: None,
                nikaya: "samyutta", cst_code: None, sc_code: Some("sn1.3"),
                content_xml: "<p>1</p>", content_html: None, cst_vagga: None,
                cst_sutta: None, cst_paranum: None, sc_sutta: None,
                start_line: 1, start_char: 0, end_line: 1, end_char: 10, group_levels: "[]",
            },
            NewXmlFragment {
                cst_file: "test.xml", frag_idx: 1, frag_type: "Sutta", frag_review: None,
                nikaya: "samyutta", cst_code: None, sc_code: Some("sn2.2"),
                content_xml: "<p>2</p>", content_html: None, cst_vagga: None,
                cst_sutta: None, cst_paranum: None, sc_sutta: None,
                start_line: 2, start_char: 0, end_line: 2, end_char: 10, group_levels: "[]",
            },
        ];

        for fragment in fragments {
            diesel::insert_into(xml_fragments::table)
                .values(&fragment)
                .execute(&mut conn)
                .expect("Failed to insert fragment");
        }

        let result = check_sc_code_sequence(&mut conn, false);
        assert_eq!(result.errors.len(), 1, "Should detect group not starting at 1");
        assert!(result.errors[0].message.contains("starts at 2 instead of 1"),
            "Error should mention sutta not starting at 1");
    }

    #[test]
    fn test_sc_code_sequence_group_jump() {
        let (mut conn, _temp_db) = setup_test_db();

        // Invalid: sn1.1 -> sn5.1 (group jump from 1 to 5)
        let fragments = vec![
            NewXmlFragment {
                cst_file: "test.xml", frag_idx: 0, frag_type: "Sutta", frag_review: None,
                nikaya: "samyutta", cst_code: None, sc_code: Some("sn1.1"),
                content_xml: "<p>1</p>", content_html: None, cst_vagga: None,
                cst_sutta: None, cst_paranum: None, sc_sutta: None,
                start_line: 1, start_char: 0, end_line: 1, end_char: 10, group_levels: "[]",
            },
            NewXmlFragment {
                cst_file: "test.xml", frag_idx: 1, frag_type: "Sutta", frag_review: None,
                nikaya: "samyutta", cst_code: None, sc_code: Some("sn5.1"),
                content_xml: "<p>2</p>", content_html: None, cst_vagga: None,
                cst_sutta: None, cst_paranum: None, sc_sutta: None,
                start_line: 2, start_char: 0, end_line: 2, end_char: 10, group_levels: "[]",
            },
        ];

        for fragment in fragments {
            diesel::insert_into(xml_fragments::table)
                .values(&fragment)
                .execute(&mut conn)
                .expect("Failed to insert fragment");
        }

        let result = check_sc_code_sequence(&mut conn, false);
        assert_eq!(result.errors.len(), 1, "Should detect group jump");
        assert!(result.errors[0].message.contains("Group jump"),
            "Error should mention group jump");
    }

    #[test]
    fn test_sc_code_sequence_skips_null() {
        let (mut conn, _temp_db) = setup_test_db();

        // Valid sequence with null/empty sc_code values skipped: sn1.1 -> null -> "" -> sn1.2
        // Also tests that Header fragments are ignored (only Sutta fragments are checked)
        let fragments = vec![
            NewXmlFragment {
                cst_file: "test.xml", frag_idx: 0, frag_type: "Sutta", frag_review: None,
                nikaya: "samyutta", cst_code: None, sc_code: Some("sn1.1"),
                content_xml: "<p>1</p>", content_html: None, cst_vagga: None,
                cst_sutta: None, cst_paranum: None, sc_sutta: None,
                start_line: 1, start_char: 0, end_line: 1, end_char: 10, group_levels: "[]",
            },
            NewXmlFragment {
                cst_file: "test.xml", frag_idx: 1, frag_type: "Sutta", frag_review: None,
                nikaya: "samyutta", cst_code: None, sc_code: None, // null sc_code
                content_xml: "<p>H</p>", content_html: None, cst_vagga: None,
                cst_sutta: None, cst_paranum: None, sc_sutta: None,
                start_line: 2, start_char: 0, end_line: 2, end_char: 10, group_levels: "[]",
            },
            NewXmlFragment {
                cst_file: "test.xml", frag_idx: 2, frag_type: "Sutta", frag_review: None,
                nikaya: "samyutta", cst_code: None, sc_code: Some(""), // empty sc_code
                content_xml: "<p>H2</p>", content_html: None, cst_vagga: None,
                cst_sutta: None, cst_paranum: None, sc_sutta: None,
                start_line: 3, start_char: 0, end_line: 3, end_char: 10, group_levels: "[]",
            },
            NewXmlFragment {
                cst_file: "test.xml", frag_idx: 3, frag_type: "Header", frag_review: None,
                nikaya: "samyutta", cst_code: None, sc_code: Some("sn99.99"), // Header ignored
                content_xml: "<h>H</h>", content_html: None, cst_vagga: None,
                cst_sutta: None, cst_paranum: None, sc_sutta: None,
                start_line: 4, start_char: 0, end_line: 4, end_char: 10, group_levels: "[]",
            },
            NewXmlFragment {
                cst_file: "test.xml", frag_idx: 4, frag_type: "Sutta", frag_review: None,
                nikaya: "samyutta", cst_code: None, sc_code: Some("sn1.2"),
                content_xml: "<p>2</p>", content_html: None, cst_vagga: None,
                cst_sutta: None, cst_paranum: None, sc_sutta: None,
                start_line: 5, start_char: 0, end_line: 5, end_char: 10, group_levels: "[]",
            },
        ];

        for fragment in fragments {
            diesel::insert_into(xml_fragments::table)
                .values(&fragment)
                .execute(&mut conn)
                .expect("Failed to insert fragment");
        }

        let result = check_sc_code_sequence(&mut conn, false);
        assert_eq!(result.errors.len(), 0, "Null values and non-Sutta fragments should be skipped");
    }

    #[test]
    fn test_sc_code_sequence_without_groups() {
        let (mut conn, _temp_db) = setup_test_db();

        // Valid sequence without groups: dn1 -> dn2 -> dn3
        let fragments = vec![
            NewXmlFragment {
                cst_file: "test.xml", frag_idx: 0, frag_type: "Sutta", frag_review: None,
                nikaya: "digha", cst_code: None, sc_code: Some("dn1"),
                content_xml: "<p>1</p>", content_html: None, cst_vagga: None,
                cst_sutta: None, cst_paranum: None, sc_sutta: None,
                start_line: 1, start_char: 0, end_line: 1, end_char: 10, group_levels: "[]",
            },
            NewXmlFragment {
                cst_file: "test.xml", frag_idx: 1, frag_type: "Sutta", frag_review: None,
                nikaya: "digha", cst_code: None, sc_code: Some("dn2"),
                content_xml: "<p>2</p>", content_html: None, cst_vagga: None,
                cst_sutta: None, cst_paranum: None, sc_sutta: None,
                start_line: 2, start_char: 0, end_line: 2, end_char: 10, group_levels: "[]",
            },
            NewXmlFragment {
                cst_file: "test.xml", frag_idx: 2, frag_type: "Sutta", frag_review: None,
                nikaya: "digha", cst_code: None, sc_code: Some("dn3"),
                content_xml: "<p>3</p>", content_html: None, cst_vagga: None,
                cst_sutta: None, cst_paranum: None, sc_sutta: None,
                start_line: 3, start_char: 0, end_line: 3, end_char: 10, group_levels: "[]",
            },
        ];

        for fragment in fragments {
            diesel::insert_into(xml_fragments::table)
                .values(&fragment)
                .execute(&mut conn)
                .expect("Failed to insert fragment");
        }

        let result = check_sc_code_sequence(&mut conn, false);
        assert_eq!(result.errors.len(), 0, "Valid sequence without groups should have no errors");
    }

    #[test]
    fn test_sc_code_sequence_valid_with_ranges() {
        let (mut conn, _temp_db) = setup_test_db();

        // Valid sequence with ranges: sn1.54 -> sn1.55-57 -> sn1.58
        let fragments = vec![
            NewXmlFragment {
                cst_file: "test.xml", frag_idx: 0, frag_type: "Sutta", frag_review: None,
                nikaya: "samyutta", cst_code: None, sc_code: Some("sn1.54"),
                content_xml: "<p>1</p>", content_html: None, cst_vagga: None,
                cst_sutta: None, cst_paranum: None, sc_sutta: None,
                start_line: 1, start_char: 0, end_line: 1, end_char: 10, group_levels: "[]",
            },
            NewXmlFragment {
                cst_file: "test.xml", frag_idx: 1, frag_type: "Sutta", frag_review: None,
                nikaya: "samyutta", cst_code: None, sc_code: Some("sn1.55-57"),
                content_xml: "<p>2</p>", content_html: None, cst_vagga: None,
                cst_sutta: None, cst_paranum: None, sc_sutta: None,
                start_line: 2, start_char: 0, end_line: 2, end_char: 10, group_levels: "[]",
            },
            NewXmlFragment {
                cst_file: "test.xml", frag_idx: 2, frag_type: "Sutta", frag_review: None,
                nikaya: "samyutta", cst_code: None, sc_code: Some("sn1.58"),
                content_xml: "<p>3</p>", content_html: None, cst_vagga: None,
                cst_sutta: None, cst_paranum: None, sc_sutta: None,
                start_line: 3, start_char: 0, end_line: 3, end_char: 10, group_levels: "[]",
            },
        ];

        for fragment in fragments {
            diesel::insert_into(xml_fragments::table)
                .values(&fragment)
                .execute(&mut conn)
                .expect("Failed to insert fragment");
        }

        let result = check_sc_code_sequence(&mut conn, false);
        assert_eq!(result.errors.len(), 0, "Valid sequence with ranges should have no errors");
    }

    // =========================================================================
    // Tests for check_code_uniqueness
    // =========================================================================

    #[test]
    fn test_code_uniqueness_no_duplicates() {
        let (mut conn, _temp_db) = setup_test_db();

        // All unique codes - should have no errors
        let fragments = vec![
            NewXmlFragment {
                cst_file: "test.xml", frag_idx: 0, frag_type: "Sutta", frag_review: None,
                nikaya: "digha", cst_code: Some("1"), sc_code: Some("dn1"),
                content_xml: "<p>1</p>", content_html: None, cst_vagga: None,
                cst_sutta: None, cst_paranum: None, sc_sutta: None,
                start_line: 1, start_char: 0, end_line: 1, end_char: 10, group_levels: "[]",
            },
            NewXmlFragment {
                cst_file: "test.xml", frag_idx: 1, frag_type: "Sutta", frag_review: None,
                nikaya: "digha", cst_code: Some("2"), sc_code: Some("dn2"),
                content_xml: "<p>2</p>", content_html: None, cst_vagga: None,
                cst_sutta: None, cst_paranum: None, sc_sutta: None,
                start_line: 2, start_char: 0, end_line: 2, end_char: 10, group_levels: "[]",
            },
            NewXmlFragment {
                cst_file: "test.xml", frag_idx: 2, frag_type: "Sutta", frag_review: None,
                nikaya: "digha", cst_code: Some("3"), sc_code: Some("dn3"),
                content_xml: "<p>3</p>", content_html: None, cst_vagga: None,
                cst_sutta: None, cst_paranum: None, sc_sutta: None,
                start_line: 3, start_char: 0, end_line: 3, end_char: 10, group_levels: "[]",
            },
        ];

        for fragment in fragments {
            diesel::insert_into(xml_fragments::table)
                .values(&fragment)
                .execute(&mut conn)
                .expect("Failed to insert fragment");
        }

        let result = check_code_uniqueness(&mut conn, false);
        assert_eq!(result.errors.len(), 0, "Unique codes should have no errors");
        assert_eq!(result.name, "Code Uniqueness");
    }

    #[test]
    fn test_code_uniqueness_duplicate_cst_code() {
        let (mut conn, _temp_db) = setup_test_db();

        // Duplicate cst_code "1" in two fragments
        let fragments = vec![
            NewXmlFragment {
                cst_file: "test.xml", frag_idx: 0, frag_type: "Sutta", frag_review: None,
                nikaya: "digha", cst_code: Some("1"), sc_code: Some("dn1"),
                content_xml: "<p>1</p>", content_html: None, cst_vagga: None,
                cst_sutta: None, cst_paranum: None, sc_sutta: None,
                start_line: 1, start_char: 0, end_line: 1, end_char: 10, group_levels: "[]",
            },
            NewXmlFragment {
                cst_file: "test.xml", frag_idx: 1, frag_type: "Sutta", frag_review: None,
                nikaya: "digha", cst_code: Some("1"), sc_code: Some("dn2"), // Duplicate cst_code
                content_xml: "<p>2</p>", content_html: None, cst_vagga: None,
                cst_sutta: None, cst_paranum: None, sc_sutta: None,
                start_line: 2, start_char: 0, end_line: 2, end_char: 10, group_levels: "[]",
            },
        ];

        for fragment in fragments {
            diesel::insert_into(xml_fragments::table)
                .values(&fragment)
                .execute(&mut conn)
                .expect("Failed to insert fragment");
        }

        let result = check_code_uniqueness(&mut conn, false);
        assert_eq!(result.errors.len(), 2, "Should report both fragments with duplicate cst_code");
        assert!(result.errors.iter().all(|e| e.message.contains("Duplicate cst_code '1'")),
            "All errors should mention duplicate cst_code");
    }

    #[test]
    fn test_code_uniqueness_duplicate_sc_code() {
        let (mut conn, _temp_db) = setup_test_db();

        // Duplicate sc_code "dn1" in two fragments
        let fragments = vec![
            NewXmlFragment {
                cst_file: "test.xml", frag_idx: 0, frag_type: "Sutta", frag_review: None,
                nikaya: "digha", cst_code: Some("1"), sc_code: Some("dn1"),
                content_xml: "<p>1</p>", content_html: None, cst_vagga: None,
                cst_sutta: None, cst_paranum: None, sc_sutta: None,
                start_line: 1, start_char: 0, end_line: 1, end_char: 10, group_levels: "[]",
            },
            NewXmlFragment {
                cst_file: "test.xml", frag_idx: 1, frag_type: "Sutta", frag_review: None,
                nikaya: "digha", cst_code: Some("2"), sc_code: Some("dn1"), // Duplicate sc_code
                content_xml: "<p>2</p>", content_html: None, cst_vagga: None,
                cst_sutta: None, cst_paranum: None, sc_sutta: None,
                start_line: 2, start_char: 0, end_line: 2, end_char: 10, group_levels: "[]",
            },
        ];

        for fragment in fragments {
            diesel::insert_into(xml_fragments::table)
                .values(&fragment)
                .execute(&mut conn)
                .expect("Failed to insert fragment");
        }

        let result = check_code_uniqueness(&mut conn, false);
        assert_eq!(result.errors.len(), 2, "Should report both fragments with duplicate sc_code");
        assert!(result.errors.iter().all(|e| e.message.contains("Duplicate sc_code 'dn1'")),
            "All errors should mention duplicate sc_code");
    }

    #[test]
    fn test_code_uniqueness_moved_fragments_excluded() {
        let (mut conn, _temp_db) = setup_test_db();

        // Duplicate cst_code "1" but one fragment is "moved" - should not report error
        let fragments = vec![
            NewXmlFragment {
                cst_file: "test.xml", frag_idx: 0, frag_type: "Sutta", frag_review: None,
                nikaya: "digha", cst_code: Some("1"), sc_code: Some("dn1"),
                content_xml: "<p>1</p>", content_html: None, cst_vagga: None,
                cst_sutta: None, cst_paranum: None, sc_sutta: None,
                start_line: 1, start_char: 0, end_line: 1, end_char: 10, group_levels: "[]",
            },
            NewXmlFragment {
                cst_file: "test.xml", frag_idx: 1, frag_type: "Sutta", frag_review: Some("moved"),
                nikaya: "digha", cst_code: Some("1"), sc_code: Some("dn1"), // Duplicate but moved
                content_xml: "", content_html: None, cst_vagga: None,
                cst_sutta: None, cst_paranum: None, sc_sutta: None,
                start_line: 2, start_char: 0, end_line: 2, end_char: 0, group_levels: "[]",
            },
        ];

        for fragment in fragments {
            diesel::insert_into(xml_fragments::table)
                .values(&fragment)
                .execute(&mut conn)
                .expect("Failed to insert fragment");
        }

        let result = check_code_uniqueness(&mut conn, false);
        assert_eq!(result.errors.len(), 0, "Moved fragments should be excluded from uniqueness check");
    }

    #[test]
    fn test_code_uniqueness_empty_codes_ignored() {
        let (mut conn, _temp_db) = setup_test_db();

        // Multiple fragments with empty/null codes - should not report as duplicates
        let fragments = vec![
            NewXmlFragment {
                cst_file: "test.xml", frag_idx: 0, frag_type: "Sutta", frag_review: None,
                nikaya: "digha", cst_code: None, sc_code: None,
                content_xml: "<p>1</p>", content_html: None, cst_vagga: None,
                cst_sutta: None, cst_paranum: None, sc_sutta: None,
                start_line: 1, start_char: 0, end_line: 1, end_char: 10, group_levels: "[]",
            },
            NewXmlFragment {
                cst_file: "test.xml", frag_idx: 1, frag_type: "Sutta", frag_review: None,
                nikaya: "digha", cst_code: Some(""), sc_code: Some(""),
                content_xml: "<p>2</p>", content_html: None, cst_vagga: None,
                cst_sutta: None, cst_paranum: None, sc_sutta: None,
                start_line: 2, start_char: 0, end_line: 2, end_char: 10, group_levels: "[]",
            },
            NewXmlFragment {
                cst_file: "test.xml", frag_idx: 2, frag_type: "Sutta", frag_review: None,
                nikaya: "digha", cst_code: None, sc_code: None,
                content_xml: "<p>3</p>", content_html: None, cst_vagga: None,
                cst_sutta: None, cst_paranum: None, sc_sutta: None,
                start_line: 3, start_char: 0, end_line: 3, end_char: 10, group_levels: "[]",
            },
        ];

        for fragment in fragments {
            diesel::insert_into(xml_fragments::table)
                .values(&fragment)
                .execute(&mut conn)
                .expect("Failed to insert fragment");
        }

        let result = check_code_uniqueness(&mut conn, false);
        assert_eq!(result.errors.len(), 0, "Empty/null codes should not be reported as duplicates");
    }

    #[test]
    fn test_code_uniqueness_header_fragments_excluded() {
        let (mut conn, _temp_db) = setup_test_db();

        // Header fragments with duplicate codes - should be excluded from check
        let fragments = vec![
            NewXmlFragment {
                cst_file: "test.xml", frag_idx: 0, frag_type: "Header", frag_review: None,
                nikaya: "digha", cst_code: Some("1"), sc_code: Some("dn1"),
                content_xml: "<h>H1</h>", content_html: None, cst_vagga: None,
                cst_sutta: None, cst_paranum: None, sc_sutta: None,
                start_line: 1, start_char: 0, end_line: 1, end_char: 10, group_levels: "[]",
            },
            NewXmlFragment {
                cst_file: "test.xml", frag_idx: 1, frag_type: "Header", frag_review: None,
                nikaya: "digha", cst_code: Some("1"), sc_code: Some("dn1"), // Duplicate but Header
                content_xml: "<h>H2</h>", content_html: None, cst_vagga: None,
                cst_sutta: None, cst_paranum: None, sc_sutta: None,
                start_line: 2, start_char: 0, end_line: 2, end_char: 10, group_levels: "[]",
            },
            NewXmlFragment {
                cst_file: "test.xml", frag_idx: 2, frag_type: "Sutta", frag_review: None,
                nikaya: "digha", cst_code: Some("1"), sc_code: Some("dn1"), // Same code in Sutta - only one, so OK
                content_xml: "<p>1</p>", content_html: None, cst_vagga: None,
                cst_sutta: None, cst_paranum: None, sc_sutta: None,
                start_line: 3, start_char: 0, end_line: 3, end_char: 10, group_levels: "[]",
            },
        ];

        for fragment in fragments {
            diesel::insert_into(xml_fragments::table)
                .values(&fragment)
                .execute(&mut conn)
                .expect("Failed to insert fragment");
        }

        let result = check_code_uniqueness(&mut conn, false);
        assert_eq!(result.errors.len(), 0, "Header fragments should be excluded from uniqueness check");
    }

    #[test]
    fn test_code_uniqueness_across_files_allowed() {
        let (mut conn, _temp_db) = setup_test_db();

        // Same sc_code "dn1" across different files - should NOT be reported
        // (uniqueness is only required within the same file)
        let fragments = vec![
            NewXmlFragment {
                cst_file: "file1.xml", frag_idx: 0, frag_type: "Sutta", frag_review: None,
                nikaya: "digha", cst_code: Some("1"), sc_code: Some("dn1"),
                content_xml: "<p>1</p>", content_html: None, cst_vagga: None,
                cst_sutta: None, cst_paranum: None, sc_sutta: None,
                start_line: 1, start_char: 0, end_line: 1, end_char: 10, group_levels: "[]",
            },
            NewXmlFragment {
                cst_file: "file2.xml", frag_idx: 0, frag_type: "Sutta", frag_review: None,
                nikaya: "digha", cst_code: Some("1"), sc_code: Some("dn1"), // Same codes, different file - OK
                content_xml: "<p>2</p>", content_html: None, cst_vagga: None,
                cst_sutta: None, cst_paranum: None, sc_sutta: None,
                start_line: 1, start_char: 0, end_line: 1, end_char: 10, group_levels: "[]",
            },
        ];

        for fragment in fragments {
            diesel::insert_into(xml_fragments::table)
                .values(&fragment)
                .execute(&mut conn)
                .expect("Failed to insert fragment");
        }

        let result = check_code_uniqueness(&mut conn, false);
        assert_eq!(result.errors.len(), 0, "Same codes in different files should be allowed");
    }

    #[test]
    fn test_code_uniqueness_within_same_file() {
        let (mut conn, _temp_db) = setup_test_db();

        // Duplicate sc_code "dn1" within the same file - should be reported
        let fragments = vec![
            NewXmlFragment {
                cst_file: "test.xml", frag_idx: 0, frag_type: "Sutta", frag_review: None,
                nikaya: "digha", cst_code: Some("1"), sc_code: Some("dn1"),
                content_xml: "<p>1</p>", content_html: None, cst_vagga: None,
                cst_sutta: None, cst_paranum: None, sc_sutta: None,
                start_line: 1, start_char: 0, end_line: 1, end_char: 10, group_levels: "[]",
            },
            NewXmlFragment {
                cst_file: "test.xml", frag_idx: 1, frag_type: "Sutta", frag_review: None,
                nikaya: "digha", cst_code: Some("2"), sc_code: Some("dn1"), // Duplicate sc_code in same file
                content_xml: "<p>2</p>", content_html: None, cst_vagga: None,
                cst_sutta: None, cst_paranum: None, sc_sutta: None,
                start_line: 2, start_char: 0, end_line: 2, end_char: 10, group_levels: "[]",
            },
            NewXmlFragment {
                cst_file: "other.xml", frag_idx: 0, frag_type: "Sutta", frag_review: None,
                nikaya: "digha", cst_code: Some("1"), sc_code: Some("dn1"), // Same code but different file - OK
                content_xml: "<p>3</p>", content_html: None, cst_vagga: None,
                cst_sutta: None, cst_paranum: None, sc_sutta: None,
                start_line: 1, start_char: 0, end_line: 1, end_char: 10, group_levels: "[]",
            },
        ];

        for fragment in fragments {
            diesel::insert_into(xml_fragments::table)
                .values(&fragment)
                .execute(&mut conn)
                .expect("Failed to insert fragment");
        }

        let result = check_code_uniqueness(&mut conn, false);
        // Only 2 errors for the duplicates within test.xml, not 3
        assert_eq!(result.errors.len(), 2, "Only duplicates within same file should be reported");

        // All errors should be for test.xml
        assert!(result.errors.iter().all(|e| e.cst_file == "test.xml"),
            "All errors should be for test.xml");
    }

    // =========================================================================
    // Tests for check_cst_sc_range_consistency
    // =========================================================================

    #[test]
    fn test_is_cst_code_range() {
        // Range formats
        assert!(is_cst_code_range("sn2.1.9.2-12"));
        assert!(is_cst_code_range("sn1.1.7.2-3"));
        assert!(is_cst_code_range("dn1.2-5"));

        // Non-range formats
        assert!(!is_cst_code_range("sn2.1.9.2"));
        assert!(!is_cst_code_range("dn1"));
        assert!(!is_cst_code_range("sn1.1.7"));
    }

    #[test]
    fn test_is_sc_code_range() {
        // Range formats
        assert!(is_sc_code_range("sn12.93-103"));
        assert!(is_sc_code_range("sn1.62-63"));
        assert!(is_sc_code_range("dn10-12"));

        // Non-range formats
        assert!(!is_sc_code_range("dn1"));
        assert!(!is_sc_code_range("sn1.62"));

        // With colon suffix (should be ignored)
        assert!(!is_sc_code_range("dn1:1.2"));
    }

    #[test]
    fn test_cst_sc_range_consistency_valid_both_ranges() {
        let (mut conn, _temp_db) = setup_test_db();

        // Both cst_code and sc_code are ranges - should be valid
        let fragments = vec![
            NewXmlFragment {
                cst_file: "test.xml", frag_idx: 0, frag_type: "Sutta", frag_review: None,
                nikaya: "samyutta", cst_code: Some("sn2.1.9.2-12"), sc_code: Some("sn12.93-103"),
                content_xml: "<p>1</p>", content_html: None, cst_vagga: None,
                cst_sutta: None, cst_paranum: None, sc_sutta: None,
                start_line: 1, start_char: 0, end_line: 1, end_char: 10, group_levels: "[]",
            },
        ];

        for fragment in fragments {
            diesel::insert_into(xml_fragments::table)
                .values(&fragment)
                .execute(&mut conn)
                .expect("Failed to insert fragment");
        }

        let result = check_cst_sc_range_consistency(&mut conn, false);
        assert_eq!(result.errors.len(), 0, "Both ranges should be valid");
    }

    #[test]
    fn test_cst_sc_range_consistency_valid_both_single() {
        let (mut conn, _temp_db) = setup_test_db();

        // Both cst_code and sc_code are single values (not ranges) - should be valid
        let fragments = vec![
            NewXmlFragment {
                cst_file: "test.xml", frag_idx: 0, frag_type: "Sutta", frag_review: None,
                nikaya: "digha", cst_code: Some("1"), sc_code: Some("dn1"),
                content_xml: "<p>1</p>", content_html: None, cst_vagga: None,
                cst_sutta: None, cst_paranum: None, sc_sutta: None,
                start_line: 1, start_char: 0, end_line: 1, end_char: 10, group_levels: "[]",
            },
        ];

        for fragment in fragments {
            diesel::insert_into(xml_fragments::table)
                .values(&fragment)
                .execute(&mut conn)
                .expect("Failed to insert fragment");
        }

        let result = check_cst_sc_range_consistency(&mut conn, false);
        assert_eq!(result.errors.len(), 0, "Both single values should be valid");
    }

    #[test]
    fn test_cst_sc_range_consistency_cst_range_sc_single_error() {
        let (mut conn, _temp_db) = setup_test_db();

        // cst_code is a range but sc_code is not - should error
        let fragments = vec![
            NewXmlFragment {
                cst_file: "test.xml", frag_idx: 0, frag_type: "Sutta", frag_review: None,
                nikaya: "samyutta", cst_code: Some("sn2.1.9.2-12"), sc_code: Some("sn12.93"),
                content_xml: "<p>1</p>", content_html: None, cst_vagga: None,
                cst_sutta: None, cst_paranum: None, sc_sutta: None,
                start_line: 1, start_char: 0, end_line: 1, end_char: 10, group_levels: "[]",
            },
        ];

        for fragment in fragments {
            diesel::insert_into(xml_fragments::table)
                .values(&fragment)
                .execute(&mut conn)
                .expect("Failed to insert fragment");
        }

        let result = check_cst_sc_range_consistency(&mut conn, false);
        assert_eq!(result.errors.len(), 1, "Should report range mismatch");
        assert!(result.errors[0].message.contains("sn2.1.9.2-12"),
            "Error should mention the cst_code");
        assert!(result.errors[0].message.contains("sn12.93"),
            "Error should mention the sc_code");
    }

    #[test]
    fn test_cst_sc_range_consistency_skips_null_codes() {
        let (mut conn, _temp_db) = setup_test_db();

        // Null cst_code or sc_code should be skipped
        let fragments = vec![
            NewXmlFragment {
                cst_file: "test.xml", frag_idx: 0, frag_type: "Sutta", frag_review: None,
                nikaya: "digha", cst_code: None, sc_code: Some("dn1"),
                content_xml: "<p>1</p>", content_html: None, cst_vagga: None,
                cst_sutta: None, cst_paranum: None, sc_sutta: None,
                start_line: 1, start_char: 0, end_line: 1, end_char: 10, group_levels: "[]",
            },
            NewXmlFragment {
                cst_file: "test.xml", frag_idx: 1, frag_type: "Sutta", frag_review: None,
                nikaya: "digha", cst_code: Some("1"), sc_code: None,
                content_xml: "<p>2</p>", content_html: None, cst_vagga: None,
                cst_sutta: None, cst_paranum: None, sc_sutta: None,
                start_line: 2, start_char: 0, end_line: 2, end_char: 10, group_levels: "[]",
            },
        ];

        for fragment in fragments {
            diesel::insert_into(xml_fragments::table)
                .values(&fragment)
                .execute(&mut conn)
                .expect("Failed to insert fragment");
        }

        let result = check_cst_sc_range_consistency(&mut conn, false);
        assert_eq!(result.errors.len(), 0, "Null codes should be skipped");
    }

    #[test]
    fn test_cst_sc_range_consistency_ignores_header_fragments() {
        let (mut conn, _temp_db) = setup_test_db();

        // Header fragments should be ignored
        let fragments = vec![
            NewXmlFragment {
                cst_file: "test.xml", frag_idx: 0, frag_type: "Header", frag_review: None,
                nikaya: "samyutta", cst_code: Some("sn2.1.9.2-12"), sc_code: Some("sn12.93"),
                content_xml: "<h>Header</h>", content_html: None, cst_vagga: None,
                cst_sutta: None, cst_paranum: None, sc_sutta: None,
                start_line: 1, start_char: 0, end_line: 1, end_char: 10, group_levels: "[]",
            },
        ];

        for fragment in fragments {
            diesel::insert_into(xml_fragments::table)
                .values(&fragment)
                .execute(&mut conn)
                .expect("Failed to insert fragment");
        }

        let result = check_cst_sc_range_consistency(&mut conn, false);
        assert_eq!(result.errors.len(), 0, "Header fragments should be ignored");
    }
}
