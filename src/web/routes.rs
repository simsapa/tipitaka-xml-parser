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
    UpdateMetadataRequest, BoundaryAdjustmentRequest, BoundaryAdjustmentResponse, BoundaryAction
};
use crate::fragments_schema::xml_fragments;
use crate::fragments_models::{
    XmlFragmentRecord, UpdateFragmentMetadata, UpdateFragmentBoundary, UpdateFragmentIndex
};

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

/// GET /api/files - Return list of distinct cst_file values with fragment counts
#[get("/api/files")]
fn get_files(db_state: &State<DbState>) -> Result<Json<Vec<FileListItem>>, String> {
    let mut conn = db_state.connect()
        .map_err(|e| format!("Database connection failed: {}", e))?;
    
    // Get distinct filenames
    let filenames: Vec<String> = xml_fragments::table
        .select(xml_fragments::cst_file)
        .distinct()
        .order_by(xml_fragments::cst_file)
        .load(&mut conn)
        .map_err(|e| format!("Query failed: {}", e))?;
    
    // For each filename, count fragments
    let mut files: Vec<FileListItem> = Vec::new();
    for filename in filenames {
        let count: i64 = xml_fragments::table
            .filter(xml_fragments::cst_file.eq(&filename))
            .count()
            .get_result(&mut conn)
            .map_err(|e| format!("Count query failed: {}", e))?;
        
        files.push(FileListItem {
            filename,
            fragment_count: count as i32,
        });
    }
    
    Ok(Json(files))
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
        .order_by(xml_fragments::frag_idx)
        .load(&mut conn)
        .map_err(|e| format!("Query failed: {}", e))?;
    
    let fragments: Vec<FragmentListItem> = results
        .into_iter()
        .map(|r| FragmentListItem {
            id: r.id,
            frag_idx: r.frag_idx,
            frag_type: r.frag_type,
            frag_review: r.frag_review,
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
    
    // Get previous fragment (same file, frag_idx - 1)
    let prev_fragment: Option<AdjacentFragment> = xml_fragments::table
        .filter(xml_fragments::cst_file.eq(&current.cst_file))
        .filter(xml_fragments::frag_idx.eq(current.frag_idx - 1))
        .first::<XmlFragmentRecord>(&mut conn)
        .optional()
        .map_err(|e| format!("Query failed for previous fragment: {}", e))?
        .map(|r| AdjacentFragment {
            id: r.id,
            frag_idx: r.frag_idx,
            frag_type: r.frag_type,
            content_xml: r.content_xml,
        });
    
    // Get next fragment (same file, frag_idx + 1)
    let next_fragment: Option<AdjacentFragment> = xml_fragments::table
        .filter(xml_fragments::cst_file.eq(&current.cst_file))
        .filter(xml_fragments::frag_idx.eq(current.frag_idx + 1))
        .first::<XmlFragmentRecord>(&mut conn)
        .optional()
        .map_err(|e| format!("Query failed for next fragment: {}", e))?
        .map(|r| AdjacentFragment {
            id: r.id,
            frag_idx: r.frag_idx,
            frag_type: r.frag_type,
            content_xml: r.content_xml,
        });
    
    let detail = FragmentDetail {
        id: current.id,
        cst_file: current.cst_file,
        frag_idx: current.frag_idx,
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
) -> Result<Json<String>, String> {
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
    
    Ok(Json("Fragment metadata updated successfully".to_string()))
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
                let prev: XmlFragmentRecord = xml_fragments::table
                    .filter(xml_fragments::cst_file.eq(&current.cst_file))
                    .filter(xml_fragments::frag_idx.eq(current.frag_idx - 1))
                    .first(conn)?;
                (prev, current)
            } else {
                // Adjusting boundary with next fragment
                let next: XmlFragmentRecord = xml_fragments::table
                    .filter(xml_fragments::cst_file.eq(&current.cst_file))
                    .filter(xml_fragments::frag_idx.eq(current.frag_idx + 1))
                    .first(conn)?;
                (current, next)
            };
        
        // Calculate new boundaries based on action
        // Note: This is a simplified implementation
        // In a real implementation, you would need to:
        // 1. Load the original XML file
        // 2. Re-extract content based on new boundaries
        // 3. Update content_xml for both fragments
        
        let (new_target_end_line, new_target_end_char, new_other_start_line, new_other_start_char) = 
            match request.action {
                BoundaryAction::LineUp => {
                    // Move one line from other to target
                    (target_fragment.end_line + 1, 0, other_fragment.start_line + 1, 0)
                }
                BoundaryAction::LineDown => {
                    // Move one line from target to other
                    (target_fragment.end_line - 1, target_fragment.end_char, other_fragment.start_line - 1, 0)
                }
                BoundaryAction::CharLeft => {
                    // Move one character from other to target
                    if other_fragment.start_char > 0 {
                        (target_fragment.end_line, target_fragment.end_char + 1, 
                         other_fragment.start_line, other_fragment.start_char - 1)
                    } else {
                        (target_fragment.end_line, target_fragment.end_char, 
                         other_fragment.start_line, other_fragment.start_char)
                    }
                }
                BoundaryAction::CharRight => {
                    // Move one character from target to other
                    if target_fragment.end_char > 0 {
                        (target_fragment.end_line, target_fragment.end_char - 1,
                         other_fragment.start_line, other_fragment.start_char + 1)
                    } else {
                        (target_fragment.end_line, target_fragment.end_char,
                         other_fragment.start_line, other_fragment.start_char)
                    }
                }
            };
        
        // Update target fragment
        let target_update = UpdateFragmentBoundary {
            start_line: target_fragment.start_line,
            start_char: target_fragment.start_char,
            end_line: new_target_end_line,
            end_char: new_target_end_char,
            content_xml: target_fragment.content_xml.clone(), // TODO: Re-extract from XML
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
            content_xml: other_fragment.content_xml.clone(), // TODO: Re-extract from XML
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
        let prev_fragment: Option<XmlFragmentRecord> = xml_fragments::table
            .filter(xml_fragments::cst_file.eq(&fragment_to_delete.cst_file))
            .filter(xml_fragments::frag_idx.eq(fragment_to_delete.frag_idx - 1))
            .first(conn)
            .optional()?;
        
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
            let next_fragment: Option<XmlFragmentRecord> = xml_fragments::table
                .filter(xml_fragments::cst_file.eq(&fragment_to_delete.cst_file))
                .filter(xml_fragments::frag_idx.eq(fragment_to_delete.frag_idx + 1))
                .first(conn)
                .optional()?;
            
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
        
        // Update frag_idx for all subsequent fragments in the same file
        let subsequent: Vec<XmlFragmentRecord> = xml_fragments::table
            .filter(xml_fragments::cst_file.eq(&fragment_to_delete.cst_file))
            .filter(xml_fragments::frag_idx.gt(fragment_to_delete.frag_idx))
            .load(conn)?;
        
        for frag in subsequent {
            let update = UpdateFragmentIndex {
                frag_idx: frag.frag_idx - 1,
            };
            diesel::update(xml_fragments::table.find(frag.id))
                .set(&update)
                .execute(conn)?;
        }
        
        Ok(())
    }).map_err(|e| format!("Delete transaction failed: {}", e))?;
    
    Ok(Json("Fragment deleted and merged successfully".to_string()))
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
        delete_fragment
    ]
}
