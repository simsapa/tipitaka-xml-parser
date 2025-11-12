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
    UpdateMetadataRequest, BoundaryAdjustmentRequest, BoundaryAdjustmentResponse, BoundaryAction,
    CreateFragmentRequest, CreateFragmentResponse, AppSettings
};
use crate::web::settings;
use crate::fragments_schema::xml_fragments;
use crate::fragments_models::{
    XmlFragmentRecord, UpdateFragmentMetadata, UpdateFragmentBoundary, UpdateFragmentIndex, NewXmlFragment
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
            // 1. Increment frag_idx for current and all subsequent fragments
            let to_update: Vec<XmlFragmentRecord> = xml_fragments::table
                .filter(xml_fragments::cst_file.eq(&current.cst_file))
                .filter(xml_fragments::frag_idx.ge(current.frag_idx))
                .load(conn)?;
            
            for frag in to_update {
                let update = UpdateFragmentIndex {
                    frag_idx: frag.frag_idx + 1,
                };
                diesel::update(xml_fragments::table.find(frag.id))
                    .set(&update)
                    .execute(conn)?;
            }
            
            // 2. Create new fragment at current's original frag_idx
            // Split the current fragment's content in half (approximately)
            let midpoint_line = (current.start_line + current.end_line) / 2;
            
            let new_fragment = NewXmlFragment {
                cst_file: &current.cst_file,
                frag_idx: current.frag_idx,
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
                .filter(xml_fragments::frag_idx.eq(current.frag_idx))
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
            // 1. Increment frag_idx for all subsequent fragments
            let to_update: Vec<XmlFragmentRecord> = xml_fragments::table
                .filter(xml_fragments::cst_file.eq(&current.cst_file))
                .filter(xml_fragments::frag_idx.gt(current.frag_idx))
                .load(conn)?;
            
            for frag in to_update {
                let update = UpdateFragmentIndex {
                    frag_idx: frag.frag_idx + 1,
                };
                diesel::update(xml_fragments::table.find(frag.id))
                    .set(&update)
                    .execute(conn)?;
            }
            
            // 2. Create new fragment after current
            let midpoint_line = (current.start_line + current.end_line) / 2;
            
            let new_fragment = NewXmlFragment {
                cst_file: &current.cst_file,
                frag_idx: current.frag_idx + 1,
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
                .filter(xml_fragments::frag_idx.eq(current.frag_idx + 1))
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

/// POST /api/regenerate - Run regeneration process
#[post("/api/regenerate", data = "<request>")]
fn regenerate(request: Json<RegenerateRequest>) -> Json<RegenerateResponse> {
    use std::process::Command;
    use std::fs;
    use std::path::Path;
    
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
    
    let xml_parser_path = match settings.xml_parser_binary_path.as_ref() {
        Some(p) => p,
        None => {
            return Json(RegenerateResponse {
                success: false,
                output: "ERROR: XML parser binary path not configured".to_string(),
                db_replaced: false,
            });
        }
    };
    
    let new_db_path = match settings.new_fragments_db_path.as_ref() {
        Some(p) => p,
        None => {
            return Json(RegenerateResponse {
                success: false,
                output: "ERROR: New fragments DB path not configured".to_string(),
                db_replaced: false,
            });
        }
    };
    
    let ref_db_path = match settings.reference_fragments_db_path.as_ref() {
        Some(p) => p,
        None => {
            return Json(RegenerateResponse {
                success: false,
                output: "ERROR: Reference fragments DB path not configured".to_string(),
                db_replaced: false,
            });
        }
    };
    
    let new_tsv_path = match settings.new_fragments_tsv_path.as_ref() {
        Some(p) => p,
        None => {
            return Json(RegenerateResponse {
                success: false,
                output: "ERROR: New fragments TSV path not configured".to_string(),
                db_replaced: false,
            });
        }
    };
    
    let ref_tsv_path = match settings.reference_fragments_tsv_path.as_ref() {
        Some(p) => p,
        None => {
            return Json(RegenerateResponse {
                success: false,
                output: "ERROR: Reference fragments TSV path not configured".to_string(),
                db_replaced: false,
            });
        }
    };
    
    let mut output = String::new();
    let use_reference_db = request.use_reference_db;
    
    // Step 0: Copy current database to reference database (only if using reference)
    if use_reference_db {
        output.push_str("=== Preparing Reference Database ===\n");
        let current_db_path = Path::new(&settings.db_path);
        let ref_db_file = Path::new(ref_db_path);
        
        if current_db_path.exists() {
            output.push_str(&format!("Copying {:?} to {:?}\n", current_db_path, ref_db_file));
            
            match fs::copy(current_db_path, ref_db_file) {
                Ok(bytes) => {
                    output.push_str(&format!("Copied {} bytes successfully\n\n", bytes));
                }
                Err(e) => {
                    return Json(RegenerateResponse {
                        success: false,
                        output: format!("{}ERROR: Failed to copy database to reference: {}\n", output, e),
                        db_replaced: false,
                    });
                }
            }
        } else {
            return Json(RegenerateResponse {
                success: false,
                output: format!("{}ERROR: Current database does not exist: {:?}\n", output, current_db_path),
                db_replaced: false,
            });
        }
    } else {
        output.push_str("=== Generating Fresh Database ===\n");
        output.push_str("Not using reference database - generating completely new fragments\n\n");
    }
    
    // Create temporary XML list file
    let temp_xml_list = std::env::temp_dir().join("tipitaka_xml_list.txt");
    let xml_list_content = settings.xml_filenames
        .iter()
        .map(|filename| {
            Path::new(&settings.xml_dir)
                .join(filename)
                .to_string_lossy()
                .to_string()
        })
        .collect::<Vec<_>>()
        .join("\n");
    
    if let Err(e) = fs::write(&temp_xml_list, &xml_list_content) {
        return Json(RegenerateResponse {
            success: false,
            output: format!("{}ERROR: Failed to write temp XML list: {}", output, e),
            db_replaced: false,
        });
    }
    
    output.push_str(&format!("Created temporary XML list with {} files\n\n", settings.xml_filenames.len()));
    
    // Command 1: Parse Tipitaka XML
    output.push_str("=== Parsing Tipitaka XML ===\n");
    
    let mut cmd = Command::new(xml_parser_path);
    cmd.arg("parse-tipitaka-xml")
        .arg("--xml-list")
        .arg(&temp_xml_list)
        .arg("--new-fragments-db")
        .arg(new_db_path)
        .env("ENABLE_PRINT_LOG", "false");
    
    if use_reference_db {
        output.push_str(&format!("Command: {} parse-tipitaka-xml --xml-list {:?} --new-fragments-db {:?} --reference-fragments-db {:?}\n\n",
            xml_parser_path, temp_xml_list, new_db_path, ref_db_path));
        cmd.arg("--reference-fragments-db").arg(ref_db_path);
    } else {
        output.push_str(&format!("Command: {} parse-tipitaka-xml --xml-list {:?} --new-fragments-db {:?}\n\n",
            xml_parser_path, temp_xml_list, new_db_path));
    }
    
    let cmd1 = match cmd.output() {
        Ok(output) => output,
        Err(e) => {
            let _ = fs::remove_file(&temp_xml_list);
            return Json(RegenerateResponse {
                success: false,
                output: format!("{}ERROR: Failed to run parse command: {}\nMake sure the parser binary exists at: {}", 
                    output, e, xml_parser_path),
                db_replaced: false,
            });
        }
    };
    
    output.push_str(&format!("Exit code: {}\n", cmd1.status.code().unwrap_or(-1)));
    output.push_str(&String::from_utf8_lossy(&cmd1.stdout));
    output.push_str(&String::from_utf8_lossy(&cmd1.stderr));
    output.push_str("\n");
    
    let parse_success = cmd1.status.success();
    
    // Command 2: Export fragments to TSV
    if parse_success {
        output.push_str("=== Exporting Fragments to TSV ===\n");
        output.push_str(&format!("Command: {} export-fragments-to-tsv {:?} {:?}\n\n",
            xml_parser_path, new_db_path, new_tsv_path));
        
        match Command::new(xml_parser_path)
            .arg("export-fragments-to-tsv")
            .arg(new_db_path)
            .arg(new_tsv_path)
            .env("ENABLE_PRINT_LOG", "false")
            .output()
        {
            Ok(cmd2) => {
                output.push_str(&format!("Exit code: {}\n", cmd2.status.code().unwrap_or(-1)));
                output.push_str(&String::from_utf8_lossy(&cmd2.stdout));
                output.push_str(&String::from_utf8_lossy(&cmd2.stderr));
                output.push_str("\n");
            }
            Err(e) => {
                output.push_str(&format!("ERROR: Failed to run export command: {}\n\n", e));
            }
        }
    }
    
    // Command 3: Check TSV regressions
    if parse_success && Path::new(ref_tsv_path).exists() {
        output.push_str("=== Checking TSV Regressions ===\n");
        output.push_str(&format!("Command: {} check-tsv-regressions {:?} {:?}\n\n",
            xml_parser_path, ref_tsv_path, new_tsv_path));
        
        match Command::new(xml_parser_path)
            .arg("check-tsv-regressions")
            .arg(ref_tsv_path)
            .arg(new_tsv_path)
            .env("ENABLE_PRINT_LOG", "false")
            .output()
        {
            Ok(cmd3) => {
                output.push_str(&format!("Exit code: {}\n", cmd3.status.code().unwrap_or(-1)));
                output.push_str(&String::from_utf8_lossy(&cmd3.stdout));
                output.push_str(&String::from_utf8_lossy(&cmd3.stderr));
                output.push_str("\n");
            }
            Err(e) => {
                output.push_str(&format!("ERROR: Failed to run regression check command: {}\n\n", e));
            }
        }
    } else if !Path::new(ref_tsv_path).exists() {
        output.push_str("=== Skipping TSV Regression Check ===\n");
        output.push_str("Reference TSV file does not exist\n\n");
    }
    
    // Clean up temp file
    let _ = fs::remove_file(&temp_xml_list);
    
    // Step 4: Replace current database with new one (if requested and parse was successful)
    let mut db_replaced = false;
    if !use_reference_db && parse_success {
        output.push_str("=== Replacing Current Database ===\n");
        let current_db_path = Path::new(&settings.db_path);
        let new_db_file = Path::new(new_db_path);
        
        if new_db_file.exists() {
            output.push_str(&format!("Copying {:?} to {:?}\n", new_db_file, current_db_path));
            
            match fs::copy(new_db_file, current_db_path) {
                Ok(bytes) => {
                    output.push_str(&format!("Replaced {} bytes successfully\n", bytes));
                    output.push_str("Current database has been replaced with the new one.\n");
                    output.push_str("The UI will reload to use the new database.\n\n");
                    db_replaced = true;
                }
                Err(e) => {
                    output.push_str(&format!("WARNING: Failed to replace database: {}\n", e));
                    output.push_str("The new database is available at the new-fragments-db path.\n\n");
                }
            }
        } else {
            output.push_str("WARNING: New database file not found, cannot replace current database.\n\n");
        }
    }
    
    Json(RegenerateResponse {
        success: parse_success,
        output,
        db_replaced,
    })
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
        create_fragment,
        get_settings,
        save_settings_endpoint,
        regenerate
    ]
}
