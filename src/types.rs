//! Core data structures for the Tipitaka XML parser
//!
//! This module defines the types used throughout the parser for representing
//! XML fragments, group hierarchies, and nikaya structures.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Type of XML fragment
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum FragmentType {
    /// Header fragment (contains metadata but not sutta content)
    Header,
    /// Sutta fragment (contains actual sutta text)
    Sutta,
}

/// Type of group in the hierarchy
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum GroupType {
    /// Nikaya level (e.g., Dīghanikāya)
    Nikaya,
    /// Book level (e.g., Sīlakkhandhavaggo)
    Book,
    /// Pannasaka level (e.g. Paṭhamapaṇṇāsakaṃ)
    Pannasaka,
    /// Vagga level (e.g., Mūlapariyāyavaggo)
    Vagga,
    /// Samyutta level (e.g., Devatāsaṃyutta)
    Samyutta,
    /// Sutta level (e.g., Brahmajālasutta)
    Sutta,
}

/// Represents a level in the group hierarchy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupLevel {
    /// Type of this group level
    pub group_type: GroupType,
    /// Number/index of this group
    pub group_number: Option<i32>,
    /// Title of this group
    pub title: String,
    /// ID attribute (if present)
    pub id: Option<String>,
}

/// Represents a fragment of XML with associated metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct XmlFragment {
    /// Nikaya name (e.g., "digha", "majjhima", "samyutta", "anguttara")
    pub nikaya: String,
    /// Source XML filename for tracking which file this fragment came from.
    pub cst_file: String,
    /// Index of this fragment in the list of fragments parsed from the XML file (0-indexed)
    pub frag_idx: usize,
    /// Type of this fragment
    pub frag_type: FragmentType,
    /// Comments on the review status of this fragment
    pub frag_review: Option<String>,
    /// Raw XML content of this fragment
    pub content_xml: String,
    /// Starting line number in source file (1-indexed)
    pub start_line: usize,
    /// Starting character position within start_line (0-indexed)
    pub start_char: usize,
    /// Ending line number in source file (1-indexed)
    pub end_line: usize,
    /// Ending character position within end_line (0-indexed, exclusive)
    pub end_char: usize,
    /// CST code (e.g., "dn1.1", "mn1.5.1")
    pub cst_code: Option<String>,
    /// CST vagga title (e.g., "5. Cūḷayamakavaggo")
    pub cst_vagga: Option<String>,
    /// CST sutta title (e.g., "1. Brahmajālasuttaṃ")
    pub cst_sutta: Option<String>,
    /// CST paragraph number (from first <p rend="bodytext" n="...">)
    pub cst_paranum: Option<String>,
    /// SuttaCentral code (e.g., "dn1", "mn41")
    pub sc_code: Option<String>,
    /// SuttaCentral sutta title (e.g., "Brahmajālasutta")
    pub sc_sutta: Option<String>,
    /// Hierarchy levels at the time this fragment was created
    pub group_levels: Vec<GroupLevel>,
}

/// Manual adjustment for a specific fragment
#[derive(Debug, Clone)]
pub struct FragmentAdjustment {
    pub cst_file: String,
    /// Fragment index (0-indexed)
    pub frag_idx: usize,
    /// Override end line (1-indexed)
    pub end_line: Option<usize>,
    /// Override end character position (0-indexed)
    pub end_char: Option<usize>,
}

/// Key for looking up fragment adjustments
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FragmentKey {
    pub cst_file: String,
    pub frag_idx: usize,
}

/// Container for fragment adjustments loaded from TSV
pub type FragmentAdjustments = HashMap<FragmentKey, FragmentAdjustment>;

/// Override data from a checked fragment in the database.
///
/// This type supersedes `FragmentAdjustment` by including SC field overrides
/// in addition to boundary overrides. When a user marks a fragment as "checked"
/// and corrects its `sc_code` or `sc_sutta`, those values can be used as overrides
/// during parsing to help the parser through problematic areas.
///
/// During parsing:
/// - Boundary overrides (`end_line`, `end_char`) are applied during fragment finalization
/// - SC field overrides (`sc_code`, `sc_sutta`) are applied in post-processing
///
/// `CheckedFragmentOverrides` take precedence over `FragmentAdjustments`.
#[derive(Debug, Clone, Default)]
pub struct CheckedFragmentOverride {
    /// Override end line (1-indexed). Applied during fragment finalization.
    pub end_line: Option<usize>,
    /// Override end character position (0-indexed). Applied during fragment finalization.
    pub end_char: Option<usize>,
    /// Override SC reference code (e.g., "sn5.1"). Applied in post-processing.
    pub sc_code: Option<String>,
    /// Override SC sutta name (e.g., "Āḷavikāsutta"). Applied in post-processing.
    pub sc_sutta: Option<String>,
}

/// Container for checked fragment overrides extracted from the database.
/// Key is `(cst_file, frag_idx)` matching the `FragmentKey` type.
pub type CheckedFragmentOverrides = HashMap<FragmentKey, CheckedFragmentOverride>;

/// Combined override configuration for parsing.
///
/// This struct bundles all override types to simplify function signatures.
/// During parsing, overrides are applied in this priority order:
/// 1. `checked_overrides` - User-verified corrections from the database (highest priority)
/// 2. `adjustments` - Legacy TSV-based boundary adjustments (fallback)
///
/// When both exist for the same `(cst_file, frag_idx)`:
/// - Check `checked_overrides` first; if found, use it
/// - Only fall back to `adjustments` if no checked override exists
#[derive(Debug, Clone, Default)]
pub struct ParserOverrides {
    /// Legacy fragment adjustments loaded from embedded TSV.
    /// Contains boundary overrides (`end_line`, `end_char`) only.
    pub adjustments: Option<FragmentAdjustments>,
    /// Checked fragment overrides extracted from the database.
    /// Contains both boundary and SC field overrides.
    pub checked_overrides: Option<CheckedFragmentOverrides>,
}

/// Parsed components from an SC code for context propagation.
///
/// When applying SC overrides, the `sc_code` is parsed to extract group numbers
/// that can be used to derive `sc_code` values for subsequent fragments.
///
/// # Examples
/// - `sn5.1` → `ScCodeComponents { prefix: "sn", samyutta: Some(5), sutta: Some(1), .. }`
/// - `an3.1` → `ScCodeComponents { prefix: "an", nipata: Some(3), sutta: Some(1), .. }`
/// - `dn1` → `ScCodeComponents { prefix: "dn", sutta: Some(1), .. }`
/// - `mn41` → `ScCodeComponents { prefix: "mn", sutta: Some(41), .. }`
#[derive(Debug, Clone, Default)]
pub struct ScCodeComponents {
    /// Nikaya prefix (e.g., "sn", "an", "dn", "mn")
    pub prefix: String,
    /// Samyutta number for SN (e.g., sn5.1 → 5)
    pub samyutta: Option<i32>,
    /// Nipata (book) number for AN (e.g., an3.1 → 3)
    pub nipata: Option<i32>,
    /// Sutta number (last number in the code)
    pub sutta: Option<i32>,
}

/// Typed parser errors for critical error handling.
///
/// Use these variants at error origin sites and downcast from `anyhow::Error`
/// in callers to distinguish critical errors (must exit) from non-critical ones
/// (log and continue).
#[derive(Debug, thiserror::Error)]
pub enum ParserError {
    /// Critical: fragment boundaries from DB overrides point to wrong positions.
    /// Must exit immediately — override data is stale/corrupt.
    #[error("Invalid boundary override: {details}")]
    InvalidBoundaryOverride { details: String },

    /// Critical: reconstructed XML from DB doesn't match original.
    /// Must exit immediately — parser is losing data.
    #[error("Reconstruction verification failed for {filename}: {details}")]
    ReconstructionVerificationFailed { filename: String, details: String },

    /// Non-critical: a recoverable issue for a single file.
    /// Log the error and continue with remaining files.
    #[error("{message}")]
    GeneralError { message: String },
}

impl ParserError {
    /// Returns true if the error is critical and requires immediate exit.
    pub fn is_critical(&self) -> bool {
        matches!(self,
            ParserError::InvalidBoundaryOverride { .. } |
            ParserError::ReconstructionVerificationFailed { .. }
        )
    }
}

use anyhow::{Context, Result};

// NOTE: Should remain private to limit relying on the data. Provide public
// functions to access data derived from tsv.
static ADJUST_FRAGMENTS_TSV: &str = include_str!("../assets/adjust-fragments.tsv");

/// Load fragment adjustments from the embedded TSV data
///
/// The TSV data should have a header line with at least these fields:
/// - cst_file: Name of the XML file
/// - frag_idx: Fragment index (0-indexed)
/// - end_line: (Optional) Override end line number (1-indexed)
/// - end_char: (Optional) Override end character position (0-indexed)
///
/// # Returns
/// HashMap mapping (cst_file, frag_idx) to FragmentAdjustment
pub fn load_fragment_adjustments() -> Result<FragmentAdjustments> {
    let mut lines = ADJUST_FRAGMENTS_TSV.lines();
    
    // Read header line
    let header = lines.next()
        .ok_or_else(|| anyhow::anyhow!("TSV data is empty"))?;
    
    // Parse header to find column indices
    let columns: Vec<&str> = header.split('\t').collect();
    let cst_file_idx = columns.iter().position(|&c| c == "cst_file")
        .ok_or_else(|| anyhow::anyhow!("Missing 'cst_file' column in TSV header"))?;
    let frag_idx_col = columns.iter().position(|&c| c == "frag_idx")
        .ok_or_else(|| anyhow::anyhow!("Missing 'frag_idx' column in TSV header"))?;
    let end_line_idx = columns.iter().position(|&c| c == "end_line");
    let end_char_idx = columns.iter().position(|&c| c == "end_char");
    
    let mut adjustments = FragmentAdjustments::new();
    
    // Parse data lines
    for (line_num, line) in lines.enumerate() {
        // Skip empty lines
        if line.trim().is_empty() {
            continue;
        }
        
        let fields: Vec<&str> = line.split('\t').collect();
        
        // Extract cst_file
        let cst_file = fields.get(cst_file_idx)
            .ok_or_else(|| anyhow::anyhow!("Missing cst_file field on line {}", line_num + 2))?
            .trim()
            .to_string();
        
        // Extract frag_idx
        let frag_idx_str = fields.get(frag_idx_col)
            .ok_or_else(|| anyhow::anyhow!("Missing frag_idx field on line {}", line_num + 2))?
            .trim();
        let frag_idx: usize = frag_idx_str.parse()
            .with_context(|| format!("Invalid frag_idx '{}' on line {}", frag_idx_str, line_num + 2))?;
        
        // Extract end_line if present
        let end_line = if let Some(idx) = end_line_idx {
            fields.get(idx)
                .and_then(|s| {
                    let trimmed = s.trim();
                    if trimmed.is_empty() {
                        None
                    } else {
                        trimmed.parse::<usize>().ok()
                    }
                })
        } else {
            None
        };
        
        // Extract end_char if present
        let end_char = if let Some(idx) = end_char_idx {
            fields.get(idx)
                .and_then(|s| {
                    let trimmed = s.trim();
                    if trimmed.is_empty() {
                        None
                    } else {
                        trimmed.parse::<usize>().ok()
                    }
                })
        } else {
            None
        };
        
        // Create adjustment
        let adjustment = FragmentAdjustment {
            cst_file: cst_file.clone(),
            frag_idx,
            end_line,
            end_char,
        };
        
        let key = FragmentKey {
            cst_file,
            frag_idx,
        };
        
        adjustments.insert(key, adjustment);
    }
    
    Ok(adjustments)
}
