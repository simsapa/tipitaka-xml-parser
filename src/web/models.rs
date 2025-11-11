/// Web-specific DTOs and response models
/// 
/// This module contains data transfer objects used by the web API
/// for communicating with the frontend.

use serde::{Serialize, Deserialize};

/// File list item with fragment count
#[derive(Serialize, Deserialize, Debug)]
pub struct FileListItem {
    pub filename: String,
    pub fragment_count: i32,
}

/// Fragment list item with basic info
#[derive(Serialize, Deserialize, Debug)]
pub struct FragmentListItem {
    pub id: i32,
    pub frag_idx: i32,
    pub frag_type: String,
    pub frag_review: Option<String>,
}

/// Complete fragment details including adjacent fragments
#[derive(Serialize, Deserialize, Debug)]
pub struct FragmentDetail {
    pub id: i32,
    pub cst_file: String,
    pub frag_idx: i32,
    pub frag_type: String,
    pub frag_review: Option<String>,
    pub nikaya: String,
    pub cst_code: Option<String>,
    pub sc_code: Option<String>,
    pub content_xml: String,
    pub cst_vagga: Option<String>,
    pub cst_sutta: Option<String>,
    pub cst_paranum: Option<String>,
    pub sc_sutta: Option<String>,
    pub start_line: i32,
    pub start_char: i32,
    pub end_line: i32,
    pub end_char: i32,
    pub group_levels: String,
    
    // Adjacent fragments
    pub prev_fragment: Option<AdjacentFragment>,
    pub next_fragment: Option<AdjacentFragment>,
}

/// Adjacent fragment (previous or next)
#[derive(Serialize, Deserialize, Debug)]
pub struct AdjacentFragment {
    pub id: i32,
    pub frag_idx: i32,
    pub content_xml: String,
}

/// Request body for updating fragment metadata
#[derive(Serialize, Deserialize, Debug)]
pub struct UpdateMetadataRequest {
    pub frag_review: Option<String>,
    pub cst_code: Option<String>,
    pub sc_code: Option<String>,
    pub cst_vagga: Option<String>,
    pub cst_sutta: Option<String>,
    pub cst_paranum: Option<String>,
    pub sc_sutta: Option<String>,
}

/// Boundary adjustment action types
#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "snake_case")]
pub enum BoundaryAction {
    LineUp,
    LineDown,
    CharLeft,
    CharRight,
}

/// Request body for boundary adjustment
#[derive(Serialize, Deserialize, Debug)]
pub struct BoundaryAdjustmentRequest {
    pub action: BoundaryAction,
    pub direction: String, // "prev" or "next"
}

/// Response for boundary adjustment
#[derive(Serialize, Deserialize, Debug)]
pub struct BoundaryAdjustmentResponse {
    pub success: bool,
    pub message: Option<String>,
    pub deleted_fragment_id: Option<i32>,
}
