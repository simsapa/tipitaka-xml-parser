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

/// Key for looking up fragment adjustments
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FragmentKey {
    pub cst_file: String,
    pub frag_idx: usize,
}

/// Override data from a corrected fragment in the database.
///
/// Covers both "checked" fragments (with user-verified corrections) and
/// "moved" fragments (collapsed to zero-width).
///
/// During parsing:
/// - If `collapse` is true, the fragment is made zero-width (end = start)
/// - Boundary overrides (`end_line`, `end_char`) are applied during fragment finalization
///   (ignored when `collapse` is true)
/// - SC field overrides (`sc_code`, `sc_sutta`) are applied in post-processing
/// - User-corrected metadata (`cst_code`, `cst_vagga`, `cst_sutta`, `cst_paranum`) are applied in post-processing
/// - Review status (`frag_review`) is applied in post-processing
#[derive(Debug, Clone, Default)]
pub struct CorrectionFragmentOverride {
    /// If true, collapse this fragment to zero-width (for "moved" fragments).
    /// The parser will set end = start, producing an empty fragment.
    pub collapse: bool,
    /// Override end line (1-indexed). Applied during fragment finalization.
    /// Ignored when `collapse` is true.
    pub end_line: Option<usize>,
    /// Override end character position (0-indexed). Applied during fragment finalization.
    /// Ignored when `collapse` is true.
    pub end_char: Option<usize>,
    /// Override SC reference code (e.g., "sn5.1"). Applied in post-processing.
    pub sc_code: Option<String>,
    /// Override SC sutta name (e.g., "Āḷavikāsutta"). Applied in post-processing.
    pub sc_sutta: Option<String>,
    /// Override CST code (e.g., "dn1.1.0.1"). Applied in post-processing.
    pub cst_code: Option<String>,
    /// Override CST vagga name. Applied in post-processing.
    pub cst_vagga: Option<String>,
    /// Override CST sutta name. Applied in post-processing.
    pub cst_sutta: Option<String>,
    /// Override CST paragraph number. Applied in post-processing.
    pub cst_paranum: Option<String>,
    /// Review status to restore (e.g., "checked", "moved"). Applied in post-processing.
    pub frag_review: Option<String>,
    /// Override fragment type (Header or Sutta). Applied in post-processing.
    pub frag_type: Option<FragmentType>,
}

/// Container for correction fragment overrides extracted from the database.
/// Key is `(cst_file, frag_idx)` matching the `FragmentKey` type.
pub type CorrectionFragmentOverrides = HashMap<FragmentKey, CorrectionFragmentOverride>;

/// Combined override configuration for parsing.
///
/// This struct bundles all override types to simplify function signatures.
#[derive(Debug, Clone, Default)]
pub struct ParserOverrides {
    /// Correction fragment overrides extracted from the database.
    /// Contains both boundary and SC field overrides, plus collapse flag for moved fragments.
    pub correction_overrides: Option<CorrectionFragmentOverrides>,
    /// Pali titles cache from ArangoDB for populating sc_sutta fields.
    /// Maps SC code (e.g., "sn5.2") to Pali title (e.g., "Somāsutta").
    /// Used during SC code propagation to automatically fill in sc_sutta titles.
    pub pali_titles: Option<HashMap<String, String>>,
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

    /// Critical: fragment count differs from reference DB.
    /// Override data may be stale — frag_idx values no longer align.
    #[error("Row count mismatch for {filename}: new parser produced {new_count} fragments, reference db has {ref_count}")]
    RowCountMismatch {
        filename: String,
        new_count: usize,
        ref_count: usize,
    },

    /// Critical: first or last fragment is not a Header type.
    /// This violates the structural invariant that XML files must start and end with Headers.
    #[error("Header validation failed for {filename}: {details}")]
    HeaderValidationFailed { filename: String, details: String },

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
            ParserError::ReconstructionVerificationFailed { .. } |
            ParserError::RowCountMismatch { .. } |
            ParserError::HeaderValidationFailed { .. }
        )
    }
}

