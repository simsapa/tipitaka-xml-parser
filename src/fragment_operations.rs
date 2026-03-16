//! Fragment Operations Module
//!
//! This module provides helper functions for fragment operations including
//! moving fragment content between adjacent fragments.

use anyhow::{anyhow, Context, Result};
use diesel::prelude::*;
use diesel::SqliteConnection;
use serde::{Deserialize, Serialize};

use crate::fragments_models::{XmlFragmentRecord, UpdateFragmentBoundary, ClearMovedFragmentMetadata, NewXmlFragment};
use crate::fragments_schema::xml_fragments;
use crate::types::{compare_frag_idx_code, parse_frag_idx_code, next_sub_index};

/// Direction enum for specifying which adjacent fragment to move content to
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Direction {
    /// Move content to the previous fragment
    Prev,
    /// Move content to the next fragment
    Next,
}

/// Find the next non-moved fragment in a given direction
///
/// This function searches for the adjacent fragment that doesn't have frag_review="moved",
/// skipping over any fragments that are already marked as moved.
///
/// This is a public helper function that can be used by both the move operation
/// and the fragment detail display logic.
///
/// # Arguments
/// * `conn` - Database connection
/// * `cst_file` - The XML file identifier
/// * `current_frag_idx_code` - The current fragment's frag_idx_code (e.g., "21.0")
/// * `direction` - Direction to search (Prev or Next)
///
/// # Returns
/// * `Ok(Some(fragment))` - Found a non-moved fragment
/// * `Ok(None)` - No non-moved fragment found (reached boundary)
/// * `Err(e)` - Database error
pub fn find_target_fragment(
    conn: &mut SqliteConnection,
    cst_file: &str,
    current_frag_idx_code: &str,
    direction: Direction,
) -> Result<Option<XmlFragmentRecord>> {
    // Load all fragments for this file and sort them by frag_idx_code
    let all_fragments: Vec<XmlFragmentRecord> = xml_fragments::table
        .filter(xml_fragments::cst_file.eq(cst_file))
        .load(conn)
        .context("Failed to load fragments for file")?;

    // Sort by frag_idx_code using version-style comparison
    let mut sorted_fragments = all_fragments;
    sorted_fragments.sort_by(|a, b| compare_frag_idx_code(&a.frag_idx_code, &b.frag_idx_code));

    // Find current fragment's position in sorted list
    let current_pos = sorted_fragments
        .iter()
        .position(|f| f.frag_idx_code == current_frag_idx_code);

    let current_pos = match current_pos {
        Some(pos) => pos,
        None => return Ok(None), // Current fragment not found
    };

    // Search in the specified direction
    let mut search_pos = match direction {
        Direction::Prev => {
            if current_pos == 0 {
                return Ok(None); // Already at boundary
            }
            current_pos - 1
        }
        Direction::Next => current_pos + 1,
    };

    loop {
        if search_pos >= sorted_fragments.len() {
            return Ok(None); // Reached boundary
        }

        let frag = &sorted_fragments[search_pos];

        // Check if this fragment is moved
        if frag.frag_review.as_deref() == Some("moved") {
            // Skip this fragment and continue searching
            match direction {
                Direction::Prev => {
                    if search_pos == 0 {
                        return Ok(None);
                    }
                    search_pos -= 1;
                }
                Direction::Next => {
                    search_pos += 1;
                }
            };
        } else {
            // Found a non-moved fragment
            return Ok(Some(frag.clone()));
        }
    }
}

/// Increment the major version of a frag_idx_code string
/// e.g., "5.0" -> "6.0", "10.3" -> "11.0"
pub fn increment_frag_idx_code(s: &str) -> String {
    let (major, _) = parse_frag_idx_code(s);
    format!("{}.0", major + 1)
}

/// Move fragment content to an adjacent fragment
///
/// This function transfers the content of the specified fragment to an adjacent fragment
/// (either previous or next), updates boundaries, clears metadata, and marks the source
/// fragment as "moved".
///
/// The function will skip over any fragments that are already marked as "moved" to find
/// the next valid target fragment.
///
/// # Arguments
/// * `conn` - Database connection
/// * `cst_file` - The XML file identifier
/// * `frag_idx_code` - The fragment index code to move (e.g., "21.0")
/// * `direction` - Direction to move (Prev or Next)
///
/// # Returns
/// * `Ok((current_fragment, target_fragment))` - Tuple of updated fragments
/// * `Err(e)` - Error if operation fails (e.g., boundary violation, database error)
pub fn move_fragment_content(
    conn: &mut SqliteConnection,
    cst_file: &str,
    frag_idx_code: &str,
    direction: Direction,
) -> Result<(XmlFragmentRecord, XmlFragmentRecord)> {
    conn.transaction::<_, anyhow::Error, _>(|conn| {
        // Load the current fragment
        let current_fragment: XmlFragmentRecord = xml_fragments::table
            .filter(xml_fragments::cst_file.eq(cst_file))
            .filter(xml_fragments::frag_idx_code.eq(frag_idx_code))
            .first(conn)
            .context("Failed to load current fragment")?;

        // Find the target fragment (skipping any moved fragments)
        let target_fragment = find_target_fragment(conn, cst_file, frag_idx_code, direction)?
            .ok_or_else(|| anyhow!(
                "Cannot move to {}: no valid target fragment found (boundary reached or all adjacent fragments are moved)",
                match direction {
                    Direction::Prev => "previous",
                    Direction::Next => "next",
                }
            ))?;
        
        // Prepare content and boundary updates based on direction
        let (new_content, new_start_line, new_start_char, new_end_line, new_end_char) = match direction {
            Direction::Prev => {
                // Moving to previous: append current content to target, use target start and current end
                (
                    format!("{}\n{}", target_fragment.content_xml, current_fragment.content_xml),
                    target_fragment.start_line,
                    target_fragment.start_char,
                    current_fragment.end_line,
                    current_fragment.end_char,
                )
            }
            Direction::Next => {
                // Moving to next: prepend current content to target, use current start and target end
                (
                    format!("{}\n{}", current_fragment.content_xml, target_fragment.content_xml),
                    current_fragment.start_line,
                    current_fragment.start_char,
                    target_fragment.end_line,
                    target_fragment.end_char,
                )
            }
        };
        
        // Update target fragment with merged content and new boundaries
        let target_update = UpdateFragmentBoundary {
            start_line: new_start_line,
            start_char: new_start_char,
            end_line: new_end_line,
            end_char: new_end_char,
            content_xml: new_content,
        };
        
        diesel::update(xml_fragments::table.find(target_fragment.id))
            .set(&target_update)
            .execute(conn)
            .context("Failed to update target fragment")?;
        
        // Empty current fragment content, clear metadata, and null out boundary fields.
        // Boundary fields are set to 0 (sentinel for "no valid boundary" since lines are 1-indexed)
        // to prevent stale boundaries from being picked up as overrides during reparse.
        let current_content_update = UpdateFragmentBoundary {
            start_line: 0,
            start_char: 0,
            end_line: 0,
            end_char: 0,
            content_xml: String::new(),
        };
        
        diesel::update(xml_fragments::table.find(current_fragment.id))
            .set(&current_content_update)
            .execute(conn)
            .context("Failed to clear current fragment content")?;
        
        // Clear metadata fields and set frag_review to "moved"
        let current_metadata_update = ClearMovedFragmentMetadata {
            frag_review: Some("moved".to_string()),
            cst_code: Some(None),
            sc_code: Some(None),
            cst_vagga: Some(None),
            cst_sutta: Some(None),
            cst_paranum: Some(None),
            sc_sutta: Some(None),
        };
        
        diesel::update(xml_fragments::table.find(current_fragment.id))
            .set(&current_metadata_update)
            .execute(conn)
            .context("Failed to update current fragment metadata")?;
        
        // Reload the updated fragments to return
        let updated_current: XmlFragmentRecord = xml_fragments::table
            .find(current_fragment.id)
            .first(conn)
            .context("Failed to reload current fragment")?;
        
        let updated_target: XmlFragmentRecord = xml_fragments::table
            .find(target_fragment.id)
            .first(conn)
            .context("Failed to reload target fragment")?;
        
        Ok((updated_current, updated_target))
    })
}

/// Delete a sub-fragment and merge its content into an adjacent fragment
///
/// This is similar to `move_fragment_content` but instead of marking the source as "moved",
/// it deletes the source fragment entirely from the database.
///
/// # Arguments
/// * `conn` - Database connection
/// * `cst_file` - The XML file name
/// * `frag_idx_code` - The fragment to delete (must be a sub-fragment, i.e. minor index > 0)
/// * `direction` - Which adjacent fragment to merge content into
///
/// # Returns
/// * `Ok(target_fragment)` - The updated target fragment after merge
/// * `Err(e)` - Error if operation fails
pub fn merge_delete_fragment(
    conn: &mut SqliteConnection,
    cst_file: &str,
    frag_idx_code: &str,
    direction: Direction,
) -> Result<XmlFragmentRecord> {
    // Validate that this is a sub-fragment (minor index > 0)
    let (_major, minor) = parse_frag_idx_code(frag_idx_code);
    if minor == 0 {
        return Err(anyhow!("Cannot merge-delete a primary fragment (minor index is 0). Only sub-fragments (e.g. 12.1, 12.2) can be merge-deleted."));
    }

    conn.transaction::<_, anyhow::Error, _>(|conn| {
        // Load the current fragment
        let current_fragment: XmlFragmentRecord = xml_fragments::table
            .filter(xml_fragments::cst_file.eq(cst_file))
            .filter(xml_fragments::frag_idx_code.eq(frag_idx_code))
            .first(conn)
            .context("Failed to load current fragment")?;

        // Find the target fragment (skipping any moved fragments)
        let target_fragment = find_target_fragment(conn, cst_file, frag_idx_code, direction)?
            .ok_or_else(|| anyhow!(
                "Cannot merge to {}: no valid target fragment found",
                match direction {
                    Direction::Prev => "previous",
                    Direction::Next => "next",
                }
            ))?;

        // Prepare content and boundary updates based on direction
        let (new_content, new_start_line, new_start_char, new_end_line, new_end_char) = match direction {
            Direction::Prev => {
                // Merging into previous: append current content to target
                (
                    format!("{}\n{}", target_fragment.content_xml, current_fragment.content_xml),
                    target_fragment.start_line,
                    target_fragment.start_char,
                    current_fragment.end_line,
                    current_fragment.end_char,
                )
            }
            Direction::Next => {
                // Merging into next: prepend current content to target
                (
                    format!("{}\n{}", current_fragment.content_xml, target_fragment.content_xml),
                    current_fragment.start_line,
                    current_fragment.start_char,
                    target_fragment.end_line,
                    target_fragment.end_char,
                )
            }
        };

        // Update target fragment with merged content and new boundaries
        let target_update = UpdateFragmentBoundary {
            start_line: new_start_line,
            start_char: new_start_char,
            end_line: new_end_line,
            end_char: new_end_char,
            content_xml: new_content,
        };

        diesel::update(xml_fragments::table.find(target_fragment.id))
            .set(&target_update)
            .execute(conn)
            .context("Failed to update target fragment")?;

        // Delete the current fragment
        diesel::delete(xml_fragments::table.find(current_fragment.id))
            .execute(conn)
            .context("Failed to delete current fragment")?;

        // Reload the updated target fragment
        let updated_target: XmlFragmentRecord = xml_fragments::table
            .find(target_fragment.id)
            .first(conn)
            .context("Failed to reload target fragment")?;

        Ok(updated_target)
    })
}

/// Insert a new empty fragment before or after the specified fragment
///
/// This function creates a new fragment with:
/// - `frag_idx_code`: derived from the insertion position (e.g., "21.1" between "21.0" and "22.0")
/// - `content_xml`: empty string
/// - `frag_review`: "checked"
/// - Zero-width boundaries at the correct position
/// - Metadata copied from the adjacent fragment
///
/// # Arguments
/// * `conn` - Database connection
/// * `cst_file` - The XML file identifier
/// * `frag_idx_code` - The currently selected fragment's frag_idx_code
/// * `direction` - Direction to insert (Prev = before current, Next = after current)
///
/// # Returns
/// * `Ok(fragment)` - The newly created fragment record
/// * `Err(e)` - Error if operation fails
pub fn insert_fragment(
    conn: &mut SqliteConnection,
    cst_file: &str,
    frag_idx_code: &str,
    direction: Direction,
) -> Result<XmlFragmentRecord> {
    conn.transaction::<_, anyhow::Error, _>(|conn| {
        // Load the current fragment
        let current_fragment: XmlFragmentRecord = xml_fragments::table
            .filter(xml_fragments::cst_file.eq(cst_file))
            .filter(xml_fragments::frag_idx_code.eq(frag_idx_code))
            .first(conn)
            .context("Failed to load current fragment")?;

        // Check if current fragment is moved - cannot insert adjacent to moved fragments
        if current_fragment.frag_review.as_deref() == Some("moved") {
            return Err(anyhow!("Cannot insert adjacent to a moved fragment"));
        }

        // Find the neighbor fragment (skipping moved fragments)
        let neighbor_fragment = find_target_fragment(conn, cst_file, frag_idx_code, direction)?;

        // Get all existing frag_idx_codes for this file to determine the next sub-index
        let all_fragments: Vec<XmlFragmentRecord> = xml_fragments::table
            .filter(xml_fragments::cst_file.eq(cst_file))
            .load(conn)
            .context("Failed to load all fragments")?;

        let existing_codes: Vec<&str> = all_fragments.iter().map(|f| f.frag_idx_code.as_str()).collect();

        // Determine the new frag_idx_code and boundaries based on direction
        let (new_frag_idx_code, boundary_line, boundary_char, source_fragment) = match direction {
            Direction::Prev => {
                // Insert BEFORE current fragment
                // The new fragment goes after the previous fragment (if any) but before current
                // Zero-width boundary is at current fragment's start position
                let boundary_line = current_fragment.start_line;
                let boundary_char = current_fragment.start_char;

                // The new code is based on the previous fragment's major index
                let new_code = if let Some(ref prev) = neighbor_fragment {
                    let (prev_major, _) = parse_frag_idx_code(&prev.frag_idx_code);
                    next_sub_index(&existing_codes, prev_major)
                } else {
                    // No previous fragment - this is inserting at the very beginning
                    // Use major index 0 minus 1... but that doesn't make sense
                    // Actually, if there's no previous, we're at the start of the file
                    // Use the current fragment's major index - 1, with sub-index 1
                    // But this is an edge case - typically there's always a header fragment at index 0
                    let (current_major, _) = parse_frag_idx_code(&current_fragment.frag_idx_code);
                    if current_major > 0 {
                        next_sub_index(&existing_codes, current_major - 1)
                    } else {
                        // Can't insert before fragment 0.0 in a meaningful way
                        return Err(anyhow!("Cannot insert before the first fragment"));
                    }
                };

                // Source for metadata is the previous fragment if available, else current
                let source = neighbor_fragment.as_ref().unwrap_or(&current_fragment);
                (new_code, boundary_line, boundary_char, source.clone())
            }
            Direction::Next => {
                // Insert AFTER current fragment
                // Zero-width boundary is at current fragment's end position
                let boundary_line = current_fragment.end_line;
                let boundary_char = current_fragment.end_char;

                // The new code is based on the current fragment's major index
                let (current_major, _) = parse_frag_idx_code(&current_fragment.frag_idx_code);
                let new_code = next_sub_index(&existing_codes, current_major);

                // Source for metadata is the current fragment
                (new_code, boundary_line, boundary_char, current_fragment.clone())
            }
        };

        // Create the new fragment with copied metadata and zero-width boundaries
        let new_fragment = NewXmlFragment {
            cst_file,
            frag_idx_code: &new_frag_idx_code,
            frag_type: &source_fragment.frag_type,
            frag_review: Some("checked"),
            nikaya: &source_fragment.nikaya,
            cst_code: source_fragment.cst_code.as_deref(),
            sc_code: source_fragment.sc_code.as_deref(),
            content_xml: "",
            content_html: None,
            cst_vagga: source_fragment.cst_vagga.as_deref(),
            cst_sutta: source_fragment.cst_sutta.as_deref(),
            cst_paranum: source_fragment.cst_paranum.as_deref(),
            sc_sutta: source_fragment.sc_sutta.as_deref(),
            start_line: boundary_line,
            start_char: boundary_char,
            end_line: boundary_line,
            end_char: boundary_char,
            group_levels: &source_fragment.group_levels,
        };

        diesel::insert_into(xml_fragments::table)
            .values(&new_fragment)
            .execute(conn)
            .context("Failed to insert new fragment")?;

        // When inserting "after", update the NEXT fragment's start boundary
        // AND mark it as needing an override during regeneration.
        // Without frag_review set, the next fragment won't get a boundary override
        // during regeneration, causing content duplication.
        if matches!(direction, Direction::Next) {
            if let Some(ref next_frag) = neighbor_fragment {
                // The next fragment's start should match the new fragment's boundary
                // (which is currently zero-width at boundary_line/boundary_char)
                // Also set frag_review to ensure it gets an override during regeneration
                diesel::update(xml_fragments::table.find(next_frag.id))
                    .set((
                        xml_fragments::start_line.eq(boundary_line as i32),
                        xml_fragments::start_char.eq(boundary_char as i32),
                        // Mark as boundary-adjusted so it gets extracted as an override
                        // Only set if currently NULL to avoid overwriting user's review status
                        xml_fragments::frag_review.eq(
                            next_frag.frag_review.clone().or(Some("checked".to_string()))
                        ),
                    ))
                    .execute(conn)
                    .context("Failed to update next fragment's start boundary")?;
            }
        }

        // Retrieve and return the newly created fragment
        let created_fragment: XmlFragmentRecord = xml_fragments::table
            .filter(xml_fragments::cst_file.eq(cst_file))
            .filter(xml_fragments::frag_idx_code.eq(&new_frag_idx_code))
            .first(conn)
            .context("Failed to retrieve newly created fragment")?;

        Ok(created_fragment)
    })
}

/// Boundary discontinuity information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoundaryDiscontinuity {
    pub frag_idx_code: String,
    pub expected_start_line: i32,
    pub expected_start_char: i32,
    pub actual_start_line: i32,
    pub actual_start_char: i32,
}

/// Validate that fragment boundaries form a contiguous chain
///
/// Checks that each fragment's start position matches the previous fragment's end position.
/// Skips fragments with frag_review="moved" (which have sentinel boundaries).
///
/// # Arguments
/// * `conn` - Database connection
/// * `cst_file` - The XML file identifier
///
/// # Returns
/// * `Ok(vec![])` - Boundaries are contiguous (empty vector means no errors)
/// * `Ok(vec![discontinuities])` - List of boundary mismatches found
/// * `Err(e)` - Database error
pub fn validate_boundary_chain(
    conn: &mut SqliteConnection,
    cst_file: &str,
) -> Result<Vec<BoundaryDiscontinuity>> {
    // Load all fragments for this file and sort them by frag_idx_code
    let all_fragments: Vec<XmlFragmentRecord> = xml_fragments::table
        .filter(xml_fragments::cst_file.eq(cst_file))
        .load(conn)
        .context("Failed to load fragments for validation")?;

    // Sort by frag_idx_code using version-style comparison
    let mut sorted_fragments = all_fragments;
    sorted_fragments.sort_by(|a, b| compare_frag_idx_code(&a.frag_idx_code, &b.frag_idx_code));

    let mut discontinuities = Vec::new();
    let mut prev_end_line: Option<i32> = None;
    let mut prev_end_char: Option<i32> = None;

    for frag in &sorted_fragments {
        // Skip moved fragments (they have sentinel boundaries of 0)
        if frag.frag_review.as_deref() == Some("moved") {
            continue;
        }

        if let (Some(expected_line), Some(expected_char)) = (prev_end_line, prev_end_char) {
            if frag.start_line != expected_line || frag.start_char != expected_char {
                discontinuities.push(BoundaryDiscontinuity {
                    frag_idx_code: frag.frag_idx_code.clone(),
                    expected_start_line: expected_line,
                    expected_start_char: expected_char,
                    actual_start_line: frag.start_line,
                    actual_start_char: frag.start_char,
                });
            }
        }

        prev_end_line = Some(frag.end_line);
        prev_end_char = Some(frag.end_char);
    }

    Ok(discontinuities)
}

/// Recalculate boundary positions based on content_xml
///
/// This function recalculates start_line, start_char, end_line, end_char for all fragments
/// in a file based on their actual content_xml, ensuring boundaries form a contiguous chain.
///
/// # Arguments
/// * `conn` - Database connection
/// * `cst_file` - The XML file identifier
///
/// # Returns
/// * `Ok(())` - Boundaries recalculated successfully
/// * `Err(e)` - Error during recalculation
pub fn recalculate_boundaries(
    conn: &mut SqliteConnection,
    cst_file: &str,
) -> Result<()> {
    conn.transaction::<_, anyhow::Error, _>(|conn| {
        // Load all fragments for this file and sort them by frag_idx_code
        let all_fragments: Vec<XmlFragmentRecord> = xml_fragments::table
            .filter(xml_fragments::cst_file.eq(cst_file))
            .load(conn)
            .context("Failed to load fragments for boundary recalculation")?;

        // Sort by frag_idx_code using version-style comparison
        let mut sorted_fragments = all_fragments;
        sorted_fragments.sort_by(|a, b| compare_frag_idx_code(&a.frag_idx_code, &b.frag_idx_code));

        // Track current position
        let mut current_line: i32 = 1;  // Lines are 1-indexed
        let mut current_char: i32 = 0;

        for frag in &sorted_fragments {
            // Skip moved fragments (keep their sentinel boundaries)
            if frag.frag_review.as_deref() == Some("moved") {
                continue;
            }

            let start_line = current_line;
            let start_char = current_char;

            // Calculate end position based on content_xml
            let (end_line, end_char) = calculate_end_position(&frag.content_xml, start_line, start_char);

            // Update the fragment's boundaries
            diesel::update(xml_fragments::table.find(frag.id))
                .set((
                    xml_fragments::start_line.eq(start_line),
                    xml_fragments::start_char.eq(start_char),
                    xml_fragments::end_line.eq(end_line),
                    xml_fragments::end_char.eq(end_char),
                ))
                .execute(conn)
                .context(format!("Failed to update boundaries for fragment {}", frag.frag_idx_code))?;

            // Next fragment starts where this one ends
            current_line = end_line;
            current_char = end_char;
        }

        Ok(())
    })
}

/// Calculate end line and char position given content and start position
fn calculate_end_position(content: &str, start_line: i32, start_char: i32) -> (i32, i32) {
    if content.is_empty() {
        return (start_line, start_char);
    }

    let mut line = start_line;
    let mut char_pos = start_char;

    for ch in content.chars() {
        if ch == '\n' {
            line += 1;
            char_pos = 0;
        } else {
            char_pos += 1;
        }
    }

    (line, char_pos)
}

/// Content integrity error information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentIntegrityError {
    pub expected_bytes: usize,
    pub actual_bytes: usize,
    pub difference: i64,
    pub message: String,
}

/// Validate that total content_xml bytes matches the original XML file size
///
/// This function checks that boundary adjustments haven't caused content
/// duplication or loss. It reads the original XML file and compares its
/// size to the total content_xml stored in the database.
///
/// # Arguments
/// * `conn` - Database connection
/// * `cst_file` - The XML file identifier
/// * `xml_dir` - Path to the XML directory
///
/// # Returns
/// * `Ok(None)` - Content integrity is valid
/// * `Ok(Some(error))` - Content integrity error found
/// * `Err(e)` - Error reading file or database
pub fn validate_content_integrity(
    conn: &mut SqliteConnection,
    cst_file: &str,
    xml_dir: &std::path::Path,
) -> Result<Option<ContentIntegrityError>> {
    use crate::encoding::read_xml_file;

    // Read original file to get expected size
    let xml_path = xml_dir.join(cst_file);
    let original_content = read_xml_file(&xml_path)
        .context(format!("Failed to read original XML file: {:?}", xml_path))?;
    let expected_bytes = original_content.len();

    // Load all fragments and sum content_xml lengths
    let all_fragments: Vec<XmlFragmentRecord> = xml_fragments::table
        .filter(xml_fragments::cst_file.eq(cst_file))
        .load(conn)
        .context("Failed to load fragments for content integrity check")?;

    let actual_bytes: usize = all_fragments.iter()
        .filter(|f| f.frag_review.as_deref() != Some("moved"))
        .map(|f| f.content_xml.len())
        .sum();

    let difference = actual_bytes as i64 - expected_bytes as i64;

    if difference != 0 {
        let message = if difference > 0 {
            format!(
                "Content DUPLICATION detected: stored content is {} bytes larger than original file. \
                 Expected {} bytes, found {} bytes.",
                difference, expected_bytes, actual_bytes
            )
        } else {
            format!(
                "Content LOSS detected: stored content is {} bytes smaller than original file. \
                 Expected {} bytes, found {} bytes.",
                -difference, expected_bytes, actual_bytes
            )
        };

        Ok(Some(ContentIntegrityError {
            expected_bytes,
            actual_bytes,
            difference,
            message,
        }))
    } else {
        Ok(None)
    }
}
