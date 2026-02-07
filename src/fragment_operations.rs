//! Fragment Operations Module
//!
//! This module provides helper functions for fragment operations including
//! moving fragment content between adjacent fragments.

use anyhow::{anyhow, Context, Result};
use diesel::prelude::*;
use diesel::SqliteConnection;
use serde::{Deserialize, Serialize};

use crate::fragments_models::{XmlFragmentRecord, UpdateFragmentBoundary, ClearMovedFragmentMetadata};
use crate::fragments_schema::xml_fragments;

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
/// * `current_idx` - The current fragment index
/// * `direction` - Direction to search (Prev or Next)
///
/// # Returns
/// * `Ok(Some(fragment))` - Found a non-moved fragment
/// * `Ok(None)` - No non-moved fragment found (reached boundary)
/// * `Err(e)` - Database error
pub fn find_target_fragment(
    conn: &mut SqliteConnection,
    cst_file: &str,
    current_idx: i32,
    direction: Direction,
) -> Result<Option<XmlFragmentRecord>> {
    let mut search_idx = match direction {
        Direction::Prev => current_idx - 1,
        Direction::Next => current_idx + 1,
    };
    
    loop {
        // Try to load fragment at search_idx
        let fragment: Option<XmlFragmentRecord> = xml_fragments::table
            .filter(xml_fragments::cst_file.eq(cst_file))
            .filter(xml_fragments::frag_idx.eq(search_idx))
            .first(conn)
            .optional()
            .context("Failed to query for target fragment")?;
        
        match fragment {
            None => {
                // No fragment at this index, we've reached the boundary
                return Ok(None);
            }
            Some(frag) => {
                // Check if this fragment is moved
                if frag.frag_review.as_deref() == Some("moved") {
                    // Skip this fragment and continue searching
                    search_idx = match direction {
                        Direction::Prev => search_idx - 1,
                        Direction::Next => search_idx + 1,
                    };
                } else {
                    // Found a non-moved fragment
                    return Ok(Some(frag));
                }
            }
        }
    }
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
/// * `frag_idx` - The fragment index to move
/// * `direction` - Direction to move (Prev or Next)
///
/// # Returns
/// * `Ok((current_fragment, target_fragment))` - Tuple of updated fragments
/// * `Err(e)` - Error if operation fails (e.g., boundary violation, database error)
pub fn move_fragment_content(
    conn: &mut SqliteConnection,
    cst_file: &str,
    frag_idx: i32,
    direction: Direction,
) -> Result<(XmlFragmentRecord, XmlFragmentRecord)> {
    conn.transaction::<_, anyhow::Error, _>(|conn| {
        // Load the current fragment
        let current_fragment: XmlFragmentRecord = xml_fragments::table
            .filter(xml_fragments::cst_file.eq(cst_file))
            .filter(xml_fragments::frag_idx.eq(frag_idx))
            .first(conn)
            .context("Failed to load current fragment")?;
        
        // Find the target fragment (skipping any moved fragments)
        let target_fragment = find_target_fragment(conn, cst_file, frag_idx, direction)?
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
