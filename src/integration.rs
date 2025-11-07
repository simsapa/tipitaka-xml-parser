//! High-level integration API
//!
//! This module provides the high-level API for processing XML files
//! and directories with the fragment-based parser.

use std::path::Path;
use anyhow::Result;

use super::encoding::read_xml_file;
use super::{
    detect_nikaya_structure,
    parse_into_fragments,
};
use super::types::FragmentAdjustments;

/// Statistics for a single file import
#[derive(Debug, Clone, Default)]
pub struct FileImportStats {
    pub filename: String,
    pub nikaya: String,
    pub fragments_parsed: usize,
    pub suttas_total: usize,
    pub suttas_inserted: usize,
    pub suttas_failed: usize,
}

/// Statistics from processing operations
#[derive(Debug, Clone, Default)]
pub struct ProcessingStats {
    /// Number of files processed
    pub files_processed: usize,
    /// Number of suttas inserted into database
    pub suttas_inserted: usize,
    /// Number of errors encountered
    pub errors: usize,
}

/// Complete import process for Tipitaka XML files using fragment-based parser
pub struct TipitakaImporter {
    adjustments: Option<FragmentAdjustments>,
}

impl TipitakaImporter {
    /// Create a new importer
    ///
    /// # Returns
    /// New TipitakaImporter instance
    pub fn new() -> Result<Self> {
        Ok(Self {
            adjustments: None,
        })
    }

    /// Set fragment adjustments for the importer
    pub fn with_adjustments(mut self, adjustments: FragmentAdjustments) -> Self {
        self.adjustments = Some(adjustments);
        self
    }

    /// Export fragments from an XML file to a fragments database
    ///
    /// # Arguments
    /// * `xml_path` - Path to the XML file to process
    /// * `fragments_db_path` - Path to the fragments database
    ///
    /// # Returns
    /// Number of fragments exported or error if export fails
    pub fn export_fragments(&self, xml_path: &Path, fragments_db_path: &Path) -> Result<usize> {
        use super::export_fragments_to_db;

        let filename = xml_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        // Read and parse XML
        let xml_content = read_xml_file(xml_path)?;
        let nikaya_structure = detect_nikaya_structure(&xml_content)?;
        
        // Parse into fragments
        let fragments = parse_into_fragments(
            &xml_content,
            &nikaya_structure,
            &filename,
            self.adjustments.as_ref(),
            true
        )?;

        // Export to fragments database
        export_fragments_to_db(&fragments, &nikaya_structure, fragments_db_path)
    }
}
