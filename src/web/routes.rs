/// API endpoint handlers for the web UI
/// 
/// This module contains Rocket route handlers for serving the fragment
/// review API endpoints.

use rocket::{Route, State, get, patch, post, delete, routes};
use rocket::response::content::RawHtml;
use rocket::serde::json::Json;
use std::fs;
use std::path::PathBuf;
use diesel::prelude::*;

use crate::web::state::DbState;
use crate::web::models::{
    FileListItem, FragmentListItem, FragmentDetail, AdjacentFragment,
    UpdateMetadataRequest, BoundaryAdjustmentRequest, BoundaryAdjustmentResponse, BoundaryAction, CreateFragmentRequest, CreateFragmentResponse, MoveFragmentRequest, MoveFragmentResponse, AppSettings, NikayaGroup,
    ArangoStatusResponse, PaliTitlesResponse,
    ValidationRunRequest, ValidationRunResponse, AutoFixRequest, AutoFixResponse,
};
use crate::web::settings;
use crate::web::arangodb;
use crate::web::validation;
use crate::fragments_schema::xml_fragments;
use crate::fragments_models::{
    XmlFragmentRecord, UpdateFragmentMetadata, UpdateFragmentBoundary, UpdateFragmentIndexCode, NewXmlFragment
};
use crate::fragment_operations::{Direction, move_fragment_content, find_target_fragment, increment_frag_idx_code};

/// Serve the main index.html page
#[get("/")]
fn index() -> RawHtml<String> {
    let static_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/static");
    let index_path = static_dir.join("index.html");
    
    match fs::read_to_string(index_path) {
        Ok(content) => RawHtml(content),
        Err(_) => RawHtml("<h1>Error loading index.html</h1>".to_string()),
    }
}

/// GET /api/files - Return list of distinct cst_file values grouped by nikaya with fragment counts
#[get("/api/files")]
fn get_files(db_state: &State<DbState>) -> Result<Json<Vec<NikayaGroup>>, String> {
    let mut conn = db_state.connect()
        .map_err(|e| format!("Database connection failed: {}", e))?;
    
    // Get distinct filenames with their nikaya (first occurrence for each file)
    let files_with_nikaya: Vec<(String, String)> = xml_fragments::table
        .select((xml_fragments::cst_file, xml_fragments::nikaya))
        .distinct()
        .order_by((xml_fragments::nikaya, xml_fragments::cst_file))
        .load(&mut conn)
        .map_err(|e| format!("Query failed: {}", e))?;
    
    // Group files by nikaya and get fragment counts
    let mut nikaya_groups: std::collections::HashMap<String, Vec<FileListItem>> = std::collections::HashMap::new();
    
    for (filename, nikaya) in files_with_nikaya {
        // Count fragments for this file
        let count: i64 = xml_fragments::table
            .filter(xml_fragments::cst_file.eq(&filename))
            .count()
            .get_result(&mut conn)
            .map_err(|e| format!("Count query failed for file {}: {}", filename, e))?;
        
        let file_item = FileListItem {
            filename: filename.clone(),
            fragment_count: count as i32,
            nikaya: nikaya.clone(),
        };
        
        nikaya_groups.entry(nikaya).or_insert_with(Vec::new).push(file_item);
    }
    
    // Convert to NikayaGroup structures with display names
    let mut groups: Vec<NikayaGroup> = nikaya_groups
        .into_iter()
        .map(|(nikaya, files)| {
            let display_name = match nikaya.as_str() {
                "digha" => "Dīgha Nikāya".to_string(),
                "majjhima" => "Majjhima Nikāya".to_string(),
                "samyutta" => "Saṃyutta Nikāya".to_string(),
                "anguttara" => "Aṅguttara Nikāya".to_string(),
                "khuddaka" => "Khuddaka Nikāya".to_string(),
                _ => format!("{} (Unknown)", nikaya),
            };
            
            NikayaGroup {
                nikaya,
                display_name,
                files,
            }
        })
        .collect();
    
    // Sort groups by traditional nikaya order
    groups.sort_by(|a, b| {
        let order = ["digha", "majjhima", "samyutta", "anguttara", "khuddaka"];
        let a_pos = order.iter().position(|&x| x == a.nikaya).unwrap_or(999);
        let b_pos = order.iter().position(|&x| x == b.nikaya).unwrap_or(999);
        a_pos.cmp(&b_pos)
    });
    
    Ok(Json(groups))
}

/// GET /api/files/:filename/fragments - Return fragments for a specific file
#[get("/api/files/<filename>/fragments")]
fn get_file_fragments(
    filename: String,
    db_state: &State<DbState>
) -> Result<Json<Vec<FragmentListItem>>, String> {
    let mut conn = db_state.connect()
        .map_err(|e| format!("Database connection failed: {}", e))?;
    
    let results: Vec<XmlFragmentRecord> = xml_fragments::table
        .filter(xml_fragments::cst_file.eq(&filename))
        .order_by(xml_fragments::frag_idx_code)
        .load(&mut conn)
        .map_err(|e| format!("Query failed: {}", e))?;
    
    let fragments: Vec<FragmentListItem> = results
        .into_iter()
        .map(|r| FragmentListItem {
            id: r.id,
            frag_idx_code: r.frag_idx_code,
            frag_type: r.frag_type,
            frag_review: r.frag_review,
            cst_code: r.cst_code,
            sc_code: r.sc_code,
        })
        .collect();
    
    Ok(Json(fragments))
}

/// GET /api/fragments/:id - Return fragment details with adjacent fragments
#[get("/api/fragments/<fragment_id>")]
fn get_fragment_detail(
    fragment_id: i32,
    db_state: &State<DbState>
) -> Result<Json<FragmentDetail>, String> {
    let mut conn = db_state.connect()
        .map_err(|e| format!("Database connection failed: {}", e))?;
    
    // Get the current fragment
    let current: XmlFragmentRecord = xml_fragments::table
        .find(fragment_id)
        .first(&mut conn)
        .map_err(|e| format!("Fragment not found: {}", e))?;
    
    // Get previous fragment (skip over moved fragments)
    use crate::fragment_operations::{find_target_fragment, Direction};
    
    let prev_fragment: Option<AdjacentFragment> = find_target_fragment(
        &mut conn,
        &current.cst_file,
        &current.frag_idx_code,
        Direction::Prev,
    )
        .map_err(|e| format!("Failed to find previous fragment: {}", e))?
        .map(|r| AdjacentFragment {
            id: r.id,
            frag_idx_code: r.frag_idx_code,
            frag_type: r.frag_type,
            content_xml: r.content_xml,
            cst_code: r.cst_code,
            sc_code: r.sc_code,
            cst_vagga: r.cst_vagga,
            cst_sutta: r.cst_sutta,
            sc_sutta: r.sc_sutta,
        });

    // Get next fragment (skip over moved fragments)
    let next_fragment: Option<AdjacentFragment> = find_target_fragment(
        &mut conn,
        &current.cst_file,
        &current.frag_idx_code,
        Direction::Next,
    )
        .map_err(|e| format!("Failed to find next fragment: {}", e))?
        .map(|r| AdjacentFragment {
            id: r.id,
            frag_idx_code: r.frag_idx_code,
            frag_type: r.frag_type,
            content_xml: r.content_xml,
            cst_code: r.cst_code,
            sc_code: r.sc_code,
            cst_vagga: r.cst_vagga,
            cst_sutta: r.cst_sutta,
            sc_sutta: r.sc_sutta,
        });
    
    let detail = FragmentDetail {
        id: current.id,
        cst_file: current.cst_file,
        frag_idx_code: current.frag_idx_code,
        frag_type: current.frag_type,
        frag_review: current.frag_review,
        nikaya: current.nikaya,
        cst_code: current.cst_code,
        sc_code: current.sc_code,
        content_xml: current.content_xml,
        cst_vagga: current.cst_vagga,
        cst_sutta: current.cst_sutta,
        cst_paranum: current.cst_paranum,
        sc_sutta: current.sc_sutta,
        start_line: current.start_line,
        start_char: current.start_char,
        end_line: current.end_line,
        end_char: current.end_char,
        group_levels: current.group_levels,
        prev_fragment,
        next_fragment,
    };
    
    Ok(Json(detail))
}

/// PATCH /api/fragments/:id - Update fragment metadata
#[patch("/api/fragments/<fragment_id>", data = "<update_request>")]
fn update_fragment_metadata(
    fragment_id: i32,
    update_request: Json<UpdateMetadataRequest>,
    db_state: &State<DbState>
) -> Result<Json<FragmentListItem>, String> {
    let mut conn = db_state.connect()
        .map_err(|e| format!("Database connection failed: {}", e))?;

    let changeset = UpdateFragmentMetadata {
        frag_review: update_request.frag_review.clone(),
        cst_code: update_request.cst_code.clone(),
        sc_code: update_request.sc_code.clone(),
        cst_vagga: update_request.cst_vagga.clone(),
        cst_sutta: update_request.cst_sutta.clone(),
        cst_paranum: update_request.cst_paranum.clone(),
        sc_sutta: update_request.sc_sutta.clone(),
    };
    
    diesel::update(xml_fragments::table.find(fragment_id))
        .set(&changeset)
        .execute(&mut conn)
        .map_err(|e| format!("Update failed: {}", e))?;
    
    // Fetch and return the updated fragment
    let updated: XmlFragmentRecord = xml_fragments::table
        .find(fragment_id)
        .first(&mut conn)
        .map_err(|e| format!("Failed to fetch updated fragment: {}", e))?;
    
    let fragment_item = FragmentListItem {
        id: updated.id,
        frag_idx_code: updated.frag_idx_code,
        frag_type: updated.frag_type,
        frag_review: updated.frag_review,
        cst_code: updated.cst_code,
        sc_code: updated.sc_code,
    };
    
    Ok(Json(fragment_item))
}

/// POST /api/fragments/:id/adjust-boundary - Adjust fragment boundaries
#[post("/api/fragments/<fragment_id>/adjust-boundary", data = "<request>")]
fn adjust_fragment_boundary(
    fragment_id: i32,
    request: Json<BoundaryAdjustmentRequest>,
    db_state: &State<DbState>
) -> Result<Json<BoundaryAdjustmentResponse>, String> {
    let mut conn = db_state.connect()
        .map_err(|e| format!("Database connection failed: {}", e))?;
    
    // Start a transaction
    conn.transaction::<_, diesel::result::Error, _>(|conn| {
        // Get the current fragment
        let current: XmlFragmentRecord = xml_fragments::table
            .find(fragment_id)
            .first(conn)?;
        
        // Determine which fragment to adjust (previous or next)
        let (target_fragment, other_fragment): (XmlFragmentRecord, XmlFragmentRecord) =
            if request.direction == "prev" {
                // Adjusting boundary with previous fragment
                let prev = find_target_fragment(conn, &current.cst_file, &current.frag_idx_code, Direction::Prev)
                    .map_err(|e| diesel::result::Error::QueryBuilderError(Box::new(std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))))?
                    .ok_or_else(|| diesel::result::Error::NotFound)?;
                (prev, current)
            } else {
                // Adjusting boundary with next fragment
                let next = find_target_fragment(conn, &current.cst_file, &current.frag_idx_code, Direction::Next)
                    .map_err(|e| diesel::result::Error::QueryBuilderError(Box::new(std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))))?
                    .ok_or_else(|| diesel::result::Error::NotFound)?;
                (current, next)
            };
        
         // Calculate new boundaries and update content_xml based on action
        let (new_target_end_line, new_target_end_char, new_other_start_line, new_other_start_char, 
             new_target_content, new_other_content) = 
            match request.action {
                BoundaryAction::LineUp => {
                    // Line Up: DECREASE line number (move boundary up in file)
                    // This shrinks the target fragment and grows the other fragment
                    // Remove the last line from target and add it to the beginning of other
                    // Preserve original whitespace and line endings
                    
                    // Find the last newline position to split the content
                    if let Some(last_newline_pos) = target_fragment.content_xml.rfind('\n') {
                        // Split at the last newline, preserving the newline with the moved content
                        let new_target = target_fragment.content_xml[..last_newline_pos].to_string();
                        let moved_content = &target_fragment.content_xml[last_newline_pos..];
                        let new_other = format!("{}{}", moved_content, other_fragment.content_xml);
                        
                        (target_fragment.end_line - 1, target_fragment.end_char, 
                         other_fragment.start_line - 1, 0, new_target, new_other)
                    } else {
                        // No newline found, can't move line
                        (target_fragment.end_line, target_fragment.end_char,
                         other_fragment.start_line, other_fragment.start_char,
                         target_fragment.content_xml.clone(), other_fragment.content_xml.clone())
                    }
                }
                BoundaryAction::LineDown => {
                    // Line Down: INCREASE line number (move boundary down in file)
                    // This grows the target fragment and shrinks the other fragment
                    // Move the first line from other to the end of target
                    // Preserve original whitespace and line endings
                    
                    // Find the first newline position to split the content
                    if let Some(first_newline_pos) = other_fragment.content_xml.find('\n') {
                        // Split at the first newline, preserving the newline with the moved content
                        let moved_content = &other_fragment.content_xml[..=first_newline_pos]; // Include newline
                        let new_other = &other_fragment.content_xml[first_newline_pos + 1..];
                        let new_target = format!("{}{}", target_fragment.content_xml, moved_content);
                        
                        (target_fragment.end_line + 1, 0, 
                         other_fragment.start_line + 1, 0, new_target, new_other.to_string())
                    } else {
                        // No newline found, can't move line
                        (target_fragment.end_line, target_fragment.end_char,
                         other_fragment.start_line, other_fragment.start_char,
                         target_fragment.content_xml.clone(), other_fragment.content_xml.clone())
                    }
                }
                BoundaryAction::CharLeft => {
                    // Move character left (decrease char position)
                    if target_fragment.end_char > 0 {
                        // Remove last character from target and add to beginning of other
                        // Preserve all whitespace exactly as it appears
                        
                        if !target_fragment.content_xml.is_empty() {
                            if let Some(last_char_start) = target_fragment.content_xml.char_indices().rev().next() {
                                let (last_char_pos, _) = last_char_start;
                                let new_target = &target_fragment.content_xml[..last_char_pos];
                                let moved_char = &target_fragment.content_xml[last_char_pos..];
                                let new_other = format!("{}{}", moved_char, other_fragment.content_xml);
                                
                                (target_fragment.end_line, target_fragment.end_char - 1,
                                 other_fragment.start_line, other_fragment.start_char - 1, 
                                 new_target.to_string(), new_other)
                            } else {
                                (target_fragment.end_line, target_fragment.end_char,
                                 other_fragment.start_line, other_fragment.start_char,
                                 target_fragment.content_xml.clone(), other_fragment.content_xml.clone())
                            }
                        } else {
                            (target_fragment.end_line, target_fragment.end_char,
                             other_fragment.start_line, other_fragment.start_char,
                             target_fragment.content_xml.clone(), other_fragment.content_xml.clone())
                        }
                    } else {
                        (target_fragment.end_line, target_fragment.end_char,
                         other_fragment.start_line, other_fragment.start_char,
                         target_fragment.content_xml.clone(), other_fragment.content_xml.clone())
                    }
                }
                BoundaryAction::CharRight => {
                    // Move character right (increase char position)
                    // Move first character from other to end of target
                    // Preserve all whitespace exactly as it appears
                    
                    if !other_fragment.content_xml.is_empty() {
                        if let Some((first_char_end, _)) = other_fragment.content_xml.char_indices().next() {
                            let moved_char = &other_fragment.content_xml[..=first_char_end]; // Include full char
                            let new_other = &other_fragment.content_xml[first_char_end + 1..];
                            let new_target = format!("{}{}", target_fragment.content_xml, moved_char);
                            
                            (target_fragment.end_line, target_fragment.end_char + 1,
                             other_fragment.start_line, other_fragment.start_char + 1, 
                             new_target, new_other.to_string())
                        } else {
                            (target_fragment.end_line, target_fragment.end_char,
                             other_fragment.start_line, other_fragment.start_char,
                             target_fragment.content_xml.clone(), other_fragment.content_xml.clone())
                        }
                    } else {
                        (target_fragment.end_line, target_fragment.end_char,
                         other_fragment.start_line, other_fragment.start_char,
                         target_fragment.content_xml.clone(), other_fragment.content_xml.clone())
                    }
                }
            };
        
        // Update target fragment
        let target_update = UpdateFragmentBoundary {
            start_line: target_fragment.start_line,
            start_char: target_fragment.start_char,
            end_line: new_target_end_line,
            end_char: new_target_end_char,
            content_xml: new_target_content,
        };
        
        diesel::update(xml_fragments::table.find(target_fragment.id))
            .set(&target_update)
            .execute(conn)?;
        
        // Update other fragment
        let other_update = UpdateFragmentBoundary {
            start_line: new_other_start_line,
            start_char: new_other_start_char,
            end_line: other_fragment.end_line,
            end_char: other_fragment.end_char,
            content_xml: new_other_content,
        };
        
        diesel::update(xml_fragments::table.find(other_fragment.id))
            .set(&other_update)
            .execute(conn)?;
        
        Ok(())
    }).map_err(|e| format!("Transaction failed: {}", e))?;
    
    Ok(Json(BoundaryAdjustmentResponse {
        success: true,
        message: Some("Boundary adjusted successfully".to_string()),
        deleted_fragment_id: None,
    }))
}

/// DELETE /api/fragments/:id - Delete a fragment and merge into adjacent fragment
/// The fragment with fragment_id will be DELETED
/// Its content will be merged into the adjacent fragment (prev or next based on what exists)
#[delete("/api/fragments/<fragment_id>")]
fn delete_fragment(
    fragment_id: i32,
    db_state: &State<DbState>
) -> Result<Json<String>, String> {
    let mut conn = db_state.connect()
        .map_err(|e| format!("Database connection failed: {}", e))?;
    
    conn.transaction::<_, diesel::result::Error, _>(|conn| {
        // Get the fragment to delete
        let fragment_to_delete: XmlFragmentRecord = xml_fragments::table
            .find(fragment_id)
            .first(conn)?;
        
        // Always try to merge with the PREVIOUS fragment first (if it exists)
        // This ensures that when we delete a fragment, its content goes to the one before it
        let prev_fragment = find_target_fragment(conn, &fragment_to_delete.cst_file, &fragment_to_delete.frag_idx_code, Direction::Prev)
            .map_err(|e| diesel::result::Error::QueryBuilderError(Box::new(std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))))?;

        if let Some(prev_frag) = prev_fragment {
            // Extend the previous fragment's end boundary to include the deleted fragment
            let merge_update = UpdateFragmentBoundary {
                start_line: prev_frag.start_line,
                start_char: prev_frag.start_char,
                end_line: fragment_to_delete.end_line,
                end_char: fragment_to_delete.end_char,
                // Combine content: previous first, then deleted
                content_xml: format!("{}\n{}", prev_frag.content_xml, fragment_to_delete.content_xml),
            };

            diesel::update(xml_fragments::table.find(prev_frag.id))
                .set(&merge_update)
                .execute(conn)?;
        } else {
            // If no previous fragment, merge with the next fragment
            let next_fragment = find_target_fragment(conn, &fragment_to_delete.cst_file, &fragment_to_delete.frag_idx_code, Direction::Next)
                .map_err(|e| diesel::result::Error::QueryBuilderError(Box::new(std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))))?;

            if let Some(next_frag) = next_fragment {
                // Extend the next fragment's start boundary to include the deleted fragment
                let merge_update = UpdateFragmentBoundary {
                    start_line: fragment_to_delete.start_line,
                    start_char: fragment_to_delete.start_char,
                    end_line: next_frag.end_line,
                    end_char: next_frag.end_char,
                    // Combine content: deleted first, then next
                    content_xml: format!("{}\n{}", fragment_to_delete.content_xml, next_frag.content_xml),
                };

                diesel::update(xml_fragments::table.find(next_frag.id))
                    .set(&merge_update)
                    .execute(conn)?;
            }
        }

        // Delete the fragment
        diesel::delete(xml_fragments::table.find(fragment_id))
            .execute(conn)?;

        // Note: With frag_idx_code (string identifiers like "21.0"), we don't need to
        // reindex subsequent fragments. The codes are stable identifiers.
        
        Ok(())
    }).map_err(|e| format!("Delete transaction failed: {}", e))?;
    
    Ok(Json("Fragment deleted and merged successfully".to_string()))
}

/// POST /api/fragments/move - Move fragment content to an adjacent fragment
///
/// This endpoint moves the content of a fragment to an adjacent (previous or next) fragment,
/// empties the source fragment, clears its metadata, and marks it as "moved".
/// The function will skip over any fragments that are already marked as "moved".
#[post("/api/fragments/move", data = "<request>")]
fn move_fragment(
    request: Json<MoveFragmentRequest>,
    db_state: &State<DbState>
) -> Result<Json<MoveFragmentResponse>, String> {
    // Parse direction string to Direction enum
    let direction = match request.direction.as_str() {
        "prev" => Direction::Prev,
        "next" => Direction::Next,
        _ => return Err(format!("Invalid direction: {}. Must be 'prev' or 'next'", request.direction)),
    };
    
    // Get database connection
    let mut conn = db_state.connect()
        .map_err(|e| format!("Database connection failed: {}", e))?;
    
    // Call the helper function
    let (current_fragment, target_fragment) = move_fragment_content(
        &mut conn,
        &request.xml_file,
        &request.frag_idx_code,
        direction,
    ).map_err(|e| format!("Move operation failed: {}", e))?;
    
    // Map XmlFragmentRecord to FragmentListItem DTOs
    let current_item = FragmentListItem {
        id: current_fragment.id,
        frag_idx_code: current_fragment.frag_idx_code,
        frag_type: current_fragment.frag_type,
        frag_review: current_fragment.frag_review,
        cst_code: current_fragment.cst_code,
        sc_code: current_fragment.sc_code,
    };
    
    let target_item = FragmentListItem {
        id: target_fragment.id,
        frag_idx_code: target_fragment.frag_idx_code,
        frag_type: target_fragment.frag_type,
        frag_review: target_fragment.frag_review,
        cst_code: target_fragment.cst_code,
        sc_code: target_fragment.sc_code,
    };
    
    Ok(Json(MoveFragmentResponse {
        current_fragment: current_item,
        target_fragment: target_item,
    }))
}

/// POST /api/fragments/:id/create - Create a new Sutta fragment before or after the current one
#[post("/api/fragments/<fragment_id>/create", data = "<request>")]
fn create_fragment(
    fragment_id: i32,
    request: Json<CreateFragmentRequest>,
    db_state: &State<DbState>
) -> Result<Json<CreateFragmentResponse>, String> {
    let mut conn = db_state.connect()
        .map_err(|e| format!("Database connection failed: {}", e))?;
    
    let new_fragment_id = conn.transaction::<_, diesel::result::Error, _>(|conn| {
        // Get the current fragment
        let current: XmlFragmentRecord = xml_fragments::table
            .find(fragment_id)
            .first(conn)?;
        
        let direction = request.direction.as_str();
        
        if direction == "prev" {
            // Create a new fragment BEFORE the current one
            // Save original frag_idx_code before any modifications
            let original_frag_idx_code = current.frag_idx_code.clone();

            // 1. Increment frag_idx_code for current and all subsequent fragments
            let to_update: Vec<XmlFragmentRecord> = xml_fragments::table
                .filter(xml_fragments::cst_file.eq(&current.cst_file))
                .filter(xml_fragments::frag_idx_code.ge(&original_frag_idx_code))
                .load(conn)?;

            for frag in to_update {
                let update = UpdateFragmentIndexCode {
                    frag_idx_code: increment_frag_idx_code(&frag.frag_idx_code),
                };
                diesel::update(xml_fragments::table.find(frag.id))
                    .set(&update)
                    .execute(conn)?;
            }

            // 2. Create new fragment at current's original frag_idx_code
            // Split the current fragment's content in half (approximately)
            let midpoint_line = (current.start_line + current.end_line) / 2;

            let new_fragment = NewXmlFragment {
                cst_file: &current.cst_file,
                frag_idx_code: &original_frag_idx_code,
                frag_type: "Sutta",
                frag_review: None,
                nikaya: &current.nikaya,
                cst_code: current.cst_code.as_deref(),
                sc_code: current.sc_code.as_deref(),
                content_xml: "<!-- New fragment content -->",
                content_html: None,
                cst_vagga: current.cst_vagga.as_deref(),
                cst_sutta: current.cst_sutta.as_deref(),
                cst_paranum: None,
                sc_sutta: current.sc_sutta.as_deref(),
                start_line: current.start_line,
                start_char: current.start_char,
                end_line: midpoint_line,
                end_char: 0,
                group_levels: &current.group_levels,
            };
            
            diesel::insert_into(xml_fragments::table)
                .values(&new_fragment)
                .execute(conn)?;
            
            // Get the ID of the newly created fragment
            let new_frag: XmlFragmentRecord = xml_fragments::table
                .filter(xml_fragments::cst_file.eq(&current.cst_file))
                .filter(xml_fragments::frag_idx_code.eq(current.frag_idx_code))
                .first(conn)?;
            
            // Update the (now next) current fragment's start boundary
            let update_current = UpdateFragmentBoundary {
                start_line: midpoint_line,
                start_char: 0,
                end_line: current.end_line,
                end_char: current.end_char,
                content_xml: current.content_xml.clone(),
            };
            diesel::update(xml_fragments::table.find(fragment_id))
                .set(&update_current)
                .execute(conn)?;
            
            Ok(new_frag.id)
        } else {
            // Create a new fragment AFTER the current one
            // 1. Increment frag_idx_code for all subsequent fragments
            let to_update: Vec<XmlFragmentRecord> = xml_fragments::table
                .filter(xml_fragments::cst_file.eq(&current.cst_file))
                .filter(xml_fragments::frag_idx_code.gt(&current.frag_idx_code))
                .load(conn)?;

            for frag in to_update {
                let update = UpdateFragmentIndexCode {
                    frag_idx_code: increment_frag_idx_code(&frag.frag_idx_code),
                };
                diesel::update(xml_fragments::table.find(frag.id))
                    .set(&update)
                    .execute(conn)?;
            }

            // 2. Create new fragment after current
            let new_frag_idx_code = increment_frag_idx_code(&current.frag_idx_code);
            let midpoint_line = (current.start_line + current.end_line) / 2;

            let new_fragment = NewXmlFragment {
                cst_file: &current.cst_file,
                frag_idx_code: &new_frag_idx_code,
                frag_type: "Sutta",
                frag_review: None,
                nikaya: &current.nikaya,
                cst_code: current.cst_code.as_deref(),
                sc_code: current.sc_code.as_deref(),
                content_xml: "<!-- New fragment content -->",
                content_html: None,
                cst_vagga: current.cst_vagga.as_deref(),
                cst_sutta: current.cst_sutta.as_deref(),
                cst_paranum: None,
                sc_sutta: current.sc_sutta.as_deref(),
                start_line: midpoint_line,
                start_char: 0,
                end_line: current.end_line,
                end_char: current.end_char,
                group_levels: &current.group_levels,
            };
            
            diesel::insert_into(xml_fragments::table)
                .values(&new_fragment)
                .execute(conn)?;
            
            // Get the ID of the newly created fragment
            let new_frag: XmlFragmentRecord = xml_fragments::table
                .filter(xml_fragments::cst_file.eq(&current.cst_file))
                .filter(xml_fragments::frag_idx_code.eq(&new_frag_idx_code))
                .first(conn)?;
            
            // Update current fragment's end boundary
            let update_current = UpdateFragmentBoundary {
                start_line: current.start_line,
                start_char: current.start_char,
                end_line: midpoint_line,
                end_char: 0,
                content_xml: current.content_xml.clone(),
            };
            diesel::update(xml_fragments::table.find(fragment_id))
                .set(&update_current)
                .execute(conn)?;
            
            Ok(new_frag.id)
        }
    }).map_err(|e| format!("Create fragment transaction failed: {}", e))?;
    
    Ok(Json(CreateFragmentResponse {
        success: true,
        new_fragment_id,
        message: Some("New fragment created successfully".to_string()),
    }))
}

/// GET /api/settings - Get current application settings
#[get("/api/settings")]
fn get_settings() -> Result<Json<AppSettings>, String> {
    let settings = settings::load_settings()
        .map_err(|e| format!("Failed to load settings: {}", e))?;
    
    Ok(Json(settings))
}

/// POST /api/settings - Save application settings
#[post("/api/settings", data = "<settings_data>")]
fn save_settings_endpoint(mut settings_data: Json<AppSettings>) -> Result<Json<String>, String> {
    // Generate default paths if not provided
    settings::generate_default_paths(&mut settings_data);
    
    settings::save_settings(&settings_data)
        .map_err(|e| format!("Failed to save settings: {}", e))?;
    
    Ok(Json("Settings saved successfully".to_string()))
}

/// Request for regenerate operation
#[derive(serde::Deserialize)]
struct RegenerateRequest {
    use_reference_db: bool,
}

/// Response for regenerate operation
#[derive(serde::Serialize)]
struct RegenerateResponse {
    success: bool,
    output: String,
    db_replaced: bool,
}

/// Request for single-file reparse operation
#[derive(serde::Deserialize)]
struct ReparseFileRequest {
    cst_file: String,
}

/// Response for single-file reparse operation
#[derive(serde::Serialize)]
struct ReparseFileResponse {
    success: bool,
    output: String,
    fragments_count: usize,
    review_status_restored: usize,
}

/// POST /api/regenerate - Run regeneration process
#[post("/api/regenerate", data = "<request>")]
async fn regenerate(request: Json<RegenerateRequest>) -> Json<RegenerateResponse> {
    use crate::regenerate::{RegenerateConfig, regenerate_fragments_db};
    use crate::logger;
    use crate::web::arangodb;

    // Load settings
    let mut settings = match settings::load_settings() {
        Ok(s) => s,
        Err(e) => {
            return Json(RegenerateResponse {
                success: false,
                output: format!("ERROR: Failed to load settings: {}", e),
                db_replaced: false,
            });
        }
    };

    // Generate default paths
    settings::generate_default_paths(&mut settings);

    // Validate required settings
    if settings.xml_dir.is_empty() {
        return Json(RegenerateResponse {
            success: false,
            output: "ERROR: XML directory not configured. Please configure settings first.".to_string(),
            db_replaced: false,
        });
    }
    if settings.xml_filenames.is_empty() {
        return Json(RegenerateResponse {
            success: false,
            output: "ERROR: No XML filenames configured. Please configure settings first.".to_string(),
            db_replaced: false,
        });
    }

    let new_db_path = match settings.new_fragments_db_path.as_ref() {
        Some(p) => p.clone(),
        None => {
            return Json(RegenerateResponse {
                success: false,
                output: "ERROR: New fragments DB path not configured".to_string(),
                db_replaced: false,
            });
        }
    };

    let ref_db_path = match settings.reference_fragments_db_path.as_ref() {
        Some(p) => p.clone(),
        None => {
            return Json(RegenerateResponse {
                success: false,
                output: "ERROR: Reference fragments DB path not configured".to_string(),
                db_replaced: false,
            });
        }
    };

    let new_tsv_path = match settings.new_fragments_tsv_path.as_ref() {
        Some(p) => p.clone(),
        None => {
            return Json(RegenerateResponse {
                success: false,
                output: "ERROR: New fragments TSV path not configured".to_string(),
                db_replaced: false,
            });
        }
    };

    let ref_tsv_path = match settings.reference_fragments_tsv_path.as_ref() {
        Some(p) => p.clone(),
        None => {
            return Json(RegenerateResponse {
                success: false,
                output: "ERROR: Reference fragments TSV path not configured".to_string(),
                db_replaced: false,
            });
        }
    };

    // Build regeneration config from settings
    let pali_titles = match arangodb::get_pali_titles().await {
        Ok(titles) => {
            logger::info(&format!("Loaded {} Pali titles from ArangoDB", titles.len()));
            Some(titles)
        }
        Err(e) => {
            logger::warn(&format!("Failed to load Pali titles from ArangoDB: {}", e));
            None
        }
    };

    let config = RegenerateConfig {
        db_path: PathBuf::from(&settings.db_path),
        new_db_path: PathBuf::from(&new_db_path),
        reference_db_path: PathBuf::from(&ref_db_path),
        new_tsv_path: PathBuf::from(&new_tsv_path),
        reference_tsv_path: PathBuf::from(&ref_tsv_path),
        xml_dir: PathBuf::from(&settings.xml_dir),
        xml_filenames: settings.xml_filenames.clone(),
        use_reference_db: request.use_reference_db,
        pali_titles,
    };

    // Run regeneration using the helper function
    let result = regenerate_fragments_db(&config);

    Json(RegenerateResponse {
        success: result.success,
        output: result.output,
        db_replaced: result.db_replaced,
    })
}

/// POST /api/reparse-file - Reparse a single XML file using current DB as reference
///
/// This endpoint reparses a single file while preserving checked fragment overrides
/// and frag_review status from the current database.
#[post("/api/reparse-file", data = "<request>")]
async fn reparse_file(
    request: Json<ReparseFileRequest>,
    db_state: &State<DbState>
) -> Json<ReparseFileResponse> {
    use std::path::Path;
    use crate::encoding::read_xml_file;
    use crate::nikaya_detector::detect_nikaya_structure;
    use crate::xml_parser::parse_into_fragments;
    use crate::fragment_exporter::{export_fragments_to_db, extract_correction_overrides};
    use crate::types::ParserOverrides;

    let cst_file = &request.cst_file;
    let mut output = String::new();

    output.push_str(&format!("=== Reparsing file: {} ===\n\n", cst_file));

    // Load settings
    let mut settings = match settings::load_settings() {
        Ok(s) => s,
        Err(e) => {
            return Json(ReparseFileResponse {
                success: false,
                output: format!("ERROR: Failed to load settings: {}", e),
                fragments_count: 0,
                review_status_restored: 0,
            });
        }
    };
    settings::generate_default_paths(&mut settings);

    // Validate XML directory is configured
    if settings.xml_dir.is_empty() {
        return Json(ReparseFileResponse {
            success: false,
            output: "ERROR: XML directory not configured. Please configure settings first.".to_string(),
            fragments_count: 0,
            review_status_restored: 0,
        });
    }

    // Step 1: Validate the file exists in the database
    output.push_str("Step 1: Validating file exists in database...\n");
    {
        let mut conn = match db_state.connect() {
            Ok(c) => c,
            Err(e) => {
                return Json(ReparseFileResponse {
                    success: false,
                    output: format!("{}ERROR: Database connection failed: {}", output, e),
                    fragments_count: 0,
                    review_status_restored: 0,
                });
            }
        };

        let file_exists: i64 = match xml_fragments::table
            .filter(xml_fragments::cst_file.eq(cst_file))
            .count()
            .get_result(&mut conn)
        {
            Ok(count) => count,
            Err(e) => {
                return Json(ReparseFileResponse {
                    success: false,
                    output: format!("{}ERROR: Failed to check file existence: {}", output, e),
                    fragments_count: 0,
                    review_status_restored: 0,
                });
            }
        };

        if file_exists == 0 {
            return Json(ReparseFileResponse {
                success: false,
                output: format!("{}ERROR: File '{}' not found in database", output, cst_file),
                fragments_count: 0,
                review_status_restored: 0,
            });
        }
        output.push_str(&format!("  Found {} existing fragments\n\n", file_exists));
    }

    // Step 2: Extract correction overrides from current DB
    // Note: frag_review status is now included in correction_overrides and will be applied
    // during parsing via apply_sc_overrides(), no need for separate restoration step
    output.push_str("Step 2: Extracting correction overrides from current database...\n");
    let db_path = Path::new(&settings.db_path);
    let (correction_overrides, _review_status) = match extract_correction_overrides(db_path, cst_file) {
        Ok(result) => result,
        Err(e) => {
            return Json(ReparseFileResponse {
                success: false,
                output: format!("{}ERROR: Failed to extract correction overrides: {}", output, e),
                fragments_count: 0,
                review_status_restored: 0,
            });
        }
    };
    output.push_str(&format!("  Extracted {} correction overrides (includes frag_review status)\n\n", correction_overrides.len()));

    // Step 3: Fetch Pali titles from ArangoDB (for sc_sutta population)
    output.push_str("Step 3: Fetching Pali titles from ArangoDB...\n");
    let pali_titles = match arangodb::get_pali_titles().await {
        Ok(titles) => {
            output.push_str(&format!("  Loaded {} Pali titles from ArangoDB\n\n", titles.len()));
            Some(titles)
        }
        Err(e) => {
            output.push_str(&format!("  Warning: Could not fetch Pali titles: {}\n", e));
            output.push_str("  Note: sc_sutta titles will not be populated for propagated sc_codes\n\n");
            None
        }
    };

    // Step 4: Construct ParserOverrides
    output.push_str("Step 4: Constructing parser overrides...\n");
    let overrides = ParserOverrides {
        correction_overrides: if correction_overrides.is_empty() { None } else { Some(correction_overrides) },
        pali_titles,
    };
    output.push_str("  ParserOverrides constructed\n\n");

    // Step 5: Read and parse the XML file
    output.push_str("Step 5: Reading and parsing XML file...\n");
    let xml_path = Path::new(&settings.xml_dir).join(cst_file);

    if !xml_path.exists() {
        return Json(ReparseFileResponse {
            success: false,
            output: format!("{}ERROR: XML file not found: {:?}", output, xml_path),
            fragments_count: 0,
            review_status_restored: 0,
        });
    }

    let xml_content = match read_xml_file(&xml_path) {
        Ok(content) => {
            output.push_str(&format!("  Read {} bytes from XML file\n", content.len()));
            content
        }
        Err(e) => {
            return Json(ReparseFileResponse {
                success: false,
                output: format!("{}ERROR: Failed to read XML file: {}", output, e),
                fragments_count: 0,
                review_status_restored: 0,
            });
        }
    };

    let nikaya_structure = match detect_nikaya_structure(&xml_content) {
        Ok(ns) => {
            output.push_str(&format!("  Detected nikaya: {}\n", ns.nikaya));
            ns
        }
        Err(e) => {
            return Json(ReparseFileResponse {
                success: false,
                output: format!("{}ERROR: Failed to detect nikaya structure: {}", output, e),
                fragments_count: 0,
                review_status_restored: 0,
            });
        }
    };

    let fragments = match parse_into_fragments(&xml_content, &nikaya_structure, cst_file, &overrides, true) {
        Ok(frags) => {
            output.push_str(&format!("  Parsed {} fragments\n\n", frags.len()));
            frags
        }
        Err(e) => {
            return Json(ReparseFileResponse {
                success: false,
                output: format!("{}ERROR: Failed to parse fragments: {}", output, e),
                fragments_count: 0,
                review_status_restored: 0,
            });
        }
    };

    // Step 6: Export fragments to DB (replaces existing)
    // Note: frag_review status is already set on fragments during parsing and will be
    // exported along with all other fragment fields
    output.push_str("Step 6: Exporting fragments to database...\n");
    let fragments_count = match export_fragments_to_db(&fragments, &nikaya_structure, db_path) {
        Ok(count) => {
            output.push_str(&format!("  Exported {} fragments (replaced existing, frag_review status preserved)\n\n", count));
            count
        }
        Err(e) => {
            return Json(ReparseFileResponse {
                success: false,
                output: format!("{}ERROR: Failed to export fragments: {}", output, e),
                fragments_count: 0,
                review_status_restored: 0,
            });
        }
    };

    output.push_str("=== Reparse completed successfully ===\n");

    // Count how many fragments have review status for reporting
    let review_status_count = fragments.iter().filter(|f| f.frag_review.is_some()).count();

    Json(ReparseFileResponse {
        success: true,
        output,
        fragments_count,
        review_status_restored: review_status_count,
    })
}

// ============================================================================
// ArangoDB Integration Endpoints
// ============================================================================

/// GET /api/arangodb/status - Check ArangoDB connection status
#[get("/api/arangodb/status")]
async fn get_arangodb_status() -> Json<ArangoStatusResponse> {
    let (connected, error) = arangodb::check_connection_status().await;
    Json(ArangoStatusResponse { connected, error })
}

/// GET /api/arangodb/pali-titles - Get all Pali root titles from ArangoDB
///
/// Returns a JSON object mapping uid (sc_code) to name (title).
/// Example: {"dn1": "Brahmajālasutta", "dn2": "Sāmaññaphalasutta", ...}
#[get("/api/arangodb/pali-titles")]
async fn get_pali_titles() -> Result<Json<PaliTitlesResponse>, String> {
    let titles = arangodb::get_pali_titles()
        .await
        .map_err(|e| format!("Failed to fetch Pali titles: {}", e))?;
    Ok(Json(titles))
}

// ============================================================================
// Validation Endpoints
// ============================================================================

/// POST /api/validation/run - Run all validation checks
///
/// Runs all validation checks and returns results. If ArangoDB is connected,
/// auto-fix suggestions will be included for applicable checks.
///
/// Request body (JSON):
/// {
///     "include_checked": false  // optional, defaults to false
/// }
#[post("/api/validation/run", data = "<request>")]
async fn run_validation(
    db_state: &State<DbState>,
    request: Json<ValidationRunRequest>,
) -> Result<Json<ValidationRunResponse>, String> {
    let mut conn = db_state.connect()
        .map_err(|e| format!("Database connection failed: {}", e))?;

    // Try to get Pali titles from ArangoDB for auto-fix suggestions
    let (arango_connected, pali_titles) = match arangodb::get_pali_titles().await {
        Ok(titles) => (true, Some(titles)),
        Err(_) => (false, None),
    };

    let checks = validation::run_all_validations(
        &mut conn,
        &db_state.db_path,
        pali_titles.as_ref(),
        request.include_checked,
    );

    Ok(Json(ValidationRunResponse {
        checks,
        arango_connected,
    }))
}

/// POST /api/validation/auto-fix/missing-sc-sutta - Apply auto-fixes for missing sc_sutta
///
/// Accepts a list of fixes and updates the sc_sutta field for each fragment.
#[post("/api/validation/auto-fix/missing-sc-sutta", data = "<request>")]
fn apply_missing_sc_sutta_fixes(
    db_state: &State<DbState>,
    request: Json<AutoFixRequest>,
) -> Json<AutoFixResponse> {
    let mut conn = match db_state.connect() {
        Ok(c) => c,
        Err(e) => return Json(AutoFixResponse {
            success: false,
            updated_count: 0,
            error: Some(format!("Database connection failed: {}", e)),
        }),
    };

    let mut updated_count = 0;

    for fix in &request.fixes {
        let result = diesel::update(xml_fragments::table.filter(xml_fragments::id.eq(fix.fragment_id)))
            .set(xml_fragments::sc_sutta.eq(&fix.suggested_value))
            .execute(&mut conn);

        match result {
            Ok(count) => updated_count += count as i32,
            Err(e) => return Json(AutoFixResponse {
                success: false,
                updated_count,
                error: Some(format!("Failed to update fragment {}: {}", fix.fragment_id, e)),
            }),
        }
    }

    Json(AutoFixResponse {
        success: true,
        updated_count,
        error: None,
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_line_up_content_transfer() {
        let target_content = "line1\nline2\nline3";
        let other_content = "other1\nother2";
        
        // Test the actual implementation logic
        let (new_target, new_other) = if let Some(last_newline_pos) = target_content.rfind('\n') {
            let new_target = target_content[..last_newline_pos].to_string();
            let moved_content = &target_content[last_newline_pos..];
            let new_other = format!("{}{}", moved_content, other_content);
            (new_target, new_other)
        } else {
            (target_content.to_string(), other_content.to_string())
        };
        
        assert_eq!(new_target, "line1\nline2");
        assert_eq!(new_other, "\nline3other1\nother2");
    }
    
    #[test]
    fn test_line_up_whitespace_preservation() {
        let target_content = "  <tag>content</tag>\n  \n  <next>data</next>";
        let other_content = "  <other>more</other>";
        
        let (new_target, new_other) = if let Some(last_newline_pos) = target_content.rfind('\n') {
            let new_target = target_content[..last_newline_pos].to_string();
            let moved_content = &target_content[last_newline_pos..];
            let new_other = format!("{}{}", moved_content, other_content);
            (new_target, new_other)
        } else {
            (target_content.to_string(), other_content.to_string())
        };
        
        assert_eq!(new_target, "  <tag>content</tag>\n  ");
        assert_eq!(new_other, "\n  <next>data</next>  <other>more</other>");
    }
    
    #[test]
    fn test_line_down_content_transfer() {
        let target_content = "line1\nline2";
        let other_content = "other1\nother2\nother3";
        
        // Test the actual implementation logic
        let (new_target, new_other) = if let Some(first_newline_pos) = other_content.find('\n') {
            let moved_content = &other_content[..=first_newline_pos];
            let new_other = &other_content[first_newline_pos + 1..];
            let new_target = format!("{}{}", target_content, moved_content);
            (new_target, new_other.to_string())
        } else {
            (target_content.to_string(), other_content.to_string())
        };
        
        assert_eq!(new_target, "line1\nline2other1\n");
        assert_eq!(new_other, "other2\nother3");
    }
    
    #[test]
    fn test_line_down_whitespace_preservation() {
        let target_content = "  <tag>content</tag>";
        let other_content = "  \n  <other>more</other>\n  ";
        
        let (new_target, new_other) = if let Some(first_newline_pos) = other_content.find('\n') {
            let moved_content = &other_content[..=first_newline_pos];
            let new_other = &other_content[first_newline_pos + 1..];
            let new_target = format!("{}{}", target_content, moved_content);
            (new_target, new_other.to_string())
        } else {
            (target_content.to_string(), other_content.to_string())
        };
        
        assert_eq!(new_target, "  <tag>content</tag>  \n");
        assert_eq!(new_other, "  <other>more</other>\n  ");
    }
    
    #[test]
    fn test_char_left_content_transfer() {
        let target_content = "hello";
        let other_content = "world";
        
        // Test actual implementation logic
        if !target_content.is_empty() {
            if let Some((last_char_pos, _)) = target_content.char_indices().rev().next() {
                let new_target = &target_content[..last_char_pos];
                let moved_char = &target_content[last_char_pos..];
                let new_other = format!("{}{}", moved_char, other_content);
                
                assert_eq!(new_target, "hell");
                assert_eq!(new_other, "oworld");
            }
        }
    }
    
    #[test]
    fn test_char_left_whitespace_preservation() {
        let target_content = "  <tag>content</tag>  ";
        let other_content = "  <other>data</other>";
        
        if !target_content.is_empty() {
            if let Some((last_char_pos, _)) = target_content.char_indices().rev().next() {
                let new_target = &target_content[..last_char_pos];
                let moved_char = &target_content[last_char_pos..];
                let new_other = format!("{}{}", moved_char, other_content);
                
                // The last character is a space, so it gets moved to other fragment
                assert_eq!(new_target, "  <tag>content</tag> ");
                assert_eq!(new_other, "   <other>data</other>");
            }
        }
    }
    
    #[test]
    fn test_char_right_content_transfer() {
        let target_content = "hello";
        let other_content = "world";
        
        // Test actual implementation logic
        if !other_content.is_empty() {
            if let Some((first_char_end, _)) = other_content.char_indices().next() {
                let moved_char = &other_content[..=first_char_end];
                let new_other = &other_content[first_char_end + 1..];
                let new_target = format!("{}{}", target_content, moved_char);
                
                assert_eq!(new_target, "hellow");
                assert_eq!(new_other, "orld");
            }
        }
    }
    
    #[test]
    fn test_char_right_whitespace_preservation() {
        let target_content = "  <tag>content</tag>";
        let other_content = "  \n  <other>data</other>";
        
        if !other_content.is_empty() {
            if let Some((first_char_end, _)) = other_content.char_indices().next() {
                let moved_char = &other_content[..=first_char_end];
                let new_other = &other_content[first_char_end + 1..];
                let new_target = format!("{}{}", target_content, moved_char);
                
                // The first character is a space, so it gets moved to target fragment
                // The remaining content still starts with a space and newline
                assert_eq!(new_target, "  <tag>content</tag> ");
                assert_eq!(new_other, " \n  <other>data</other>");
            }
        }
    }
}

/// Get all routes for the web application
pub fn get_routes() -> Vec<Route> {
    routes![
        index,
        get_files,
        get_file_fragments,
        get_fragment_detail,
        update_fragment_metadata,
        adjust_fragment_boundary,
        delete_fragment,
        move_fragment,
        create_fragment,
        get_settings,
        save_settings_endpoint,
        regenerate,
        reparse_file,
        get_arangodb_status,
        get_pali_titles,
        run_validation,
        apply_missing_sc_sutta_fixes
    ]
}
