/// Web-specific DTOs and response models
/// 
/// This module contains data transfer objects used by the web API
/// for communicating with the frontend.

use std::collections::HashMap;
use serde::{Serialize, Deserialize};

/// File list item with fragment count and nikaya information
#[derive(Serialize, Deserialize, Debug)]
pub struct FileListItem {
    pub filename: String,
    pub fragment_count: i32,
    pub nikaya: String,
}

/// Grouped files by nikaya for display
#[derive(Serialize, Deserialize, Debug)]
pub struct NikayaGroup {
    pub nikaya: String,
    pub display_name: String,
    pub files: Vec<FileListItem>,
}

/// Fragment list item with basic info
#[derive(Serialize, Deserialize, Debug)]
pub struct FragmentListItem {
    pub id: i32,
    pub frag_idx: i32,
    pub frag_type: String,
    pub frag_review: Option<String>,
    pub cst_code: Option<String>,
    pub sc_code: Option<String>,
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
    pub frag_type: String,
    pub content_xml: String,
    pub cst_code: Option<String>,
    pub sc_code: Option<String>,
    pub cst_vagga: Option<String>,
    pub cst_sutta: Option<String>,
    pub sc_sutta: Option<String>,
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

/// Request body for creating a new fragment
#[derive(Serialize, Deserialize, Debug)]
pub struct CreateFragmentRequest {
    pub direction: String, // "prev" or "next"
}

/// Response for fragment creation
#[derive(Serialize, Deserialize, Debug)]
pub struct CreateFragmentResponse {
    pub success: bool,
    pub new_fragment_id: i32,
    pub message: Option<String>,
}

/// Request body for moving fragment content
#[derive(Serialize, Deserialize, Debug)]
pub struct MoveFragmentRequest {
    pub frag_idx: i32,
    pub xml_file: String,
    pub direction: String, // "prev" or "next"
}

/// Response for fragment move operation
#[derive(Serialize, Deserialize, Debug)]
pub struct MoveFragmentResponse {
    pub current_fragment: FragmentListItem,
    pub target_fragment: FragmentListItem,
}

/// Color theme options
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "lowercase")]
pub enum ColorTheme {
    Light,
    Dark,
    System,
}

impl Default for ColorTheme {
    fn default() -> Self {
        ColorTheme::System
    }
}

/// Application settings stored in TOML config file
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AppSettings {
    pub db_path: String,
    pub port: u16,
    pub xml_dir: String,
    pub xml_filenames: Vec<String>,
    pub new_fragments_db_path: Option<String>,
    pub reference_fragments_db_path: Option<String>,
    pub new_fragments_tsv_path: Option<String>,
    pub reference_fragments_tsv_path: Option<String>,
    pub xml_parser_binary_path: Option<String>,
    pub color_theme: Option<ColorTheme>,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            db_path: String::new(),
            port: 8000,
            xml_dir: String::new(),
            xml_filenames: Vec::new(),
            new_fragments_db_path: None,
            reference_fragments_db_path: None,
            new_fragments_tsv_path: None,
            reference_fragments_tsv_path: None,
            xml_parser_binary_path: None,
            color_theme: None,
        }
    }
}

// ============================================================================
// ArangoDB Integration Models
// ============================================================================

/// Response for ArangoDB connection status check
#[derive(Serialize, Deserialize, Debug)]
pub struct ArangoStatusResponse {
    /// Whether connection to ArangoDB was successful
    pub connected: bool,
    /// Error message if connection failed (for tooltip display)
    pub error: Option<String>,
}

/// Response type for Pali titles endpoint
/// Maps uid (sc_code) to name (title), e.g., "dn1" -> "Brahmajālasutta"
pub type PaliTitlesResponse = HashMap<String, String>;

// ============================================================================
// Validation Models
// ============================================================================

use crate::web::validation::ValidationCheckResult;

/// Response for running all validation checks
#[derive(Serialize, Deserialize, Debug)]
pub struct ValidationRunResponse {
    /// Map of check_id to validation result
    pub checks: HashMap<String, ValidationCheckResult>,
    /// Whether ArangoDB was connected (affects auto-fix availability)
    pub arango_connected: bool,
}

/// Request to apply auto-fixes for missing sc_sutta
#[derive(Serialize, Deserialize, Debug)]
pub struct AutoFixRequest {
    /// List of fixes to apply
    pub fixes: Vec<AutoFixItem>,
}

/// Single auto-fix item
#[derive(Serialize, Deserialize, Debug)]
pub struct AutoFixItem {
    /// Fragment database ID
    pub fragment_id: i32,
    /// Value to set for sc_sutta
    pub suggested_value: String,
}

/// Response after applying auto-fixes
#[derive(Serialize, Deserialize, Debug)]
pub struct AutoFixResponse {
    /// Whether the operation succeeded
    pub success: bool,
    /// Number of fragments updated
    pub updated_count: i32,
    /// Error message if operation failed
    pub error: Option<String>,
}
