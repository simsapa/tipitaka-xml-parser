/// API endpoint handlers for the web UI
/// 
/// This module contains Rocket route handlers for serving the fragment
/// review API endpoints.

use rocket::{Route, State, get, routes};
use rocket::response::content::RawHtml;
use rocket::serde::json::Json;
use std::fs;
use std::path::PathBuf;
use diesel::prelude::*;

use crate::web::state::DbState;
use crate::web::models::{FileListItem, FragmentListItem, FragmentDetail, AdjacentFragment};
use crate::fragments_schema::xml_fragments;
use crate::fragments_models::XmlFragmentRecord;

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

/// Get all routes for the web application
pub fn get_routes() -> Vec<Route> {
    routes![index, get_files, get_file_fragments, get_fragment_detail]
}
