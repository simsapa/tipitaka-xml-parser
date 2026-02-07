//! High-level integration API
//!
//! This module provides the high-level API for processing XML files
//! and directories with the fragment-based parser.

use std::path::{Path, PathBuf};
use anyhow::Result;

use super::encoding::read_xml_file;
use super::{
    detect_nikaya_structure,
    parse_into_fragments,
};
use super::types::{FragmentAdjustments, ParserOverrides, CorrectionFragmentOverrides, ParserError};

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
    overrides: ParserOverrides,
    reference_db_path: Option<PathBuf>,
}

impl TipitakaImporter {
    /// Create a new importer
    ///
    /// # Returns
    /// New TipitakaImporter instance
    pub fn new() -> Result<Self> {
        Ok(Self {
            overrides: ParserOverrides::default(),
            reference_db_path: None,
        })
    }

    /// Set fragment adjustments for the importer (legacy TSV-based adjustments)
    pub fn with_adjustments(mut self, adjustments: FragmentAdjustments) -> Self {
        self.overrides.adjustments = Some(adjustments);
        self
    }

    /// Set correction fragment overrides for the importer (database-based overrides)
    pub fn with_correction_overrides(mut self, correction_overrides: CorrectionFragmentOverrides) -> Self {
        self.overrides.correction_overrides = Some(correction_overrides);
        self
    }

    /// Set full parser overrides for the importer
    pub fn with_overrides(mut self, overrides: ParserOverrides) -> Self {
        self.overrides = overrides;
        self
    }

    /// Set reference database path for row count validation
    pub fn with_reference_db(mut self, path: PathBuf) -> Self {
        self.reference_db_path = Some(path);
        self
    }

    /// Export fragments from an XML file to a fragments database
    ///
    /// After exporting, this method verifies that the fragments can be correctly
    /// reconstructed by reading them back from the database and comparing with
    /// the original XML content.
    ///
    /// # Reconstruction Verification
    ///
    /// The verification process:
    /// 1. Reads all fragments for this file from the database
    /// 2. Reconstructs the XML by concatenating fragments in order
    /// 3. Compares byte-by-byte with the original XML
    /// 4. Returns an error if any mismatch is detected
    ///
    /// If reconstruction fails, the error will be a typed
    /// `ParserError::ReconstructionVerificationFailed` which main.rs uses
    /// (via downcast) to detect critical failures that should exit immediately.
    ///
    /// # Arguments
    /// * `xml_path` - Path to the XML file to process
    /// * `fragments_db_path` - Path to the fragments database
    ///
    /// # Returns
    /// Number of fragments exported or error if export/verification fails
    ///
    /// # Errors
    /// Returns an error if:
    /// - File reading fails
    /// - Nikaya detection fails
    /// - Fragment parsing fails
    /// - Database export fails
    /// - Reconstruction verification fails (critical error)
    pub fn export_fragments(&self, xml_path: &Path, fragments_db_path: &Path) -> Result<usize> {
        use super::export_fragments_to_db;
        use super::reconstruct_xml_from_db;
        use super::fragment_exporter::count_fragments_in_db;
        use crate::logger;

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
            &self.overrides,
            true
        )?;

        // Validate that first and last fragments are Headers
        super::fragment_exporter::validate_first_last_headers(&fragments, &filename)?;

        // Verify fragment count matches reference DB if available
        if let Some(ref_db_path) = &self.reference_db_path {
            let ref_count = count_fragments_in_db(ref_db_path, &filename)?;
            let new_count = fragments.len();

            if new_count != ref_count {
                return Err(ParserError::RowCountMismatch {
                    filename: filename.clone(),
                    new_count,
                    ref_count,
                }.into());
            }

            logger::info(&format!("Fragment count verified for {} ({} fragments)", filename, new_count));
        }

        // Export to fragments database
        let count = export_fragments_to_db(&fragments, &nikaya_structure, fragments_db_path)?;
        
        // Verify reconstruction: read fragments from DB and compare with original
        logger::info(&format!("Verifying reconstruction for {}", filename));
        let reconstructed_xml = reconstruct_xml_from_db(fragments_db_path, &filename)?;
        
        // Compare original and reconstructed XML
        if xml_content != reconstructed_xml {
            // Try to find where they differ for better error reporting
            let original_len = xml_content.len();
            let reconstructed_len = reconstructed_xml.len();
            
            if original_len != reconstructed_len {
                return Err(ParserError::ReconstructionVerificationFailed {
                    filename: filename.clone(),
                    details: format!("length mismatch (original: {} bytes, reconstructed: {} bytes)",
                        original_len, reconstructed_len),
                }.into());
            }
            
            // Find first difference
            for (idx, (orig_char, recon_char)) in xml_content.chars().zip(reconstructed_xml.chars()).enumerate() {
                if orig_char != recon_char {
                    let context_start = idx.saturating_sub(50);
                    let context_end = (idx + 50).min(xml_content.len());
                    let context = &xml_content[context_start..context_end];
                    return Err(ParserError::ReconstructionVerificationFailed {
                        filename: filename.clone(),
                        details: format!("at position {}: expected '{}', got '{}'\nContext: {}",
                            idx, orig_char, recon_char, context),
                    }.into());
                }
            }
            
            return Err(ParserError::ReconstructionVerificationFailed {
                filename: filename.clone(),
                details: "content mismatch (details unknown)".to_string(),
            }.into());
        }
        
        logger::info(&format!("Reconstruction verified for {}", filename));
        
        Ok(count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;
    use std::path::PathBuf;

    #[test]
    fn test_export_with_reconstruction_verification() {
        // Test that export_fragments correctly verifies reconstruction
        let importer = TipitakaImporter::new().unwrap();
        
        // Use a test file from tests/data
        let test_file = PathBuf::from("tests/data/s0101m.mul.xml");
        let temp_db = NamedTempFile::new().unwrap();
        let db_path = temp_db.path();
        
        // This should succeed and verify reconstruction
        let count = importer.export_fragments(&test_file, db_path).unwrap();
        
        // Verify we exported some fragments
        assert!(count > 0, "Should export at least one fragment");
        
        // Verify the database contains the fragments
        use super::super::reconstruct_xml_from_db;
        use super::super::encoding::read_xml_file;
        
        let original_xml = read_xml_file(&test_file).unwrap();
        let reconstructed_xml = reconstruct_xml_from_db(db_path, "s0101m.mul.xml").unwrap();
        
        // They should be identical
        assert_eq!(original_xml, reconstructed_xml, 
            "Reconstructed XML should match original");
    }
    
    #[test]
    fn test_export_multiple_files_with_verification() {
        // Test that verification works when exporting multiple files to the same DB
        let importer = TipitakaImporter::new().unwrap();
        let temp_db = NamedTempFile::new().unwrap();
        let db_path = temp_db.path();
        
        let test_files = vec![
            "tests/data/s0101m.mul.xml",
            "tests/data/s0201m.mul.xml",
        ];
        
        for test_file in &test_files {
            let path = PathBuf::from(test_file);
            let count = importer.export_fragments(&path, db_path).unwrap();
            assert!(count > 0, "Should export fragments for {}", test_file);
        }
        
        // Verify each file can be reconstructed correctly
        use super::super::reconstruct_xml_from_db;
        use super::super::encoding::read_xml_file;
        
        for test_file in &test_files {
            let path = PathBuf::from(test_file);
            let filename = path.file_name().unwrap().to_str().unwrap();
            
            let original_xml = read_xml_file(&path).unwrap();
            let reconstructed_xml = reconstruct_xml_from_db(db_path, filename).unwrap();
            
            assert_eq!(original_xml, reconstructed_xml,
                "Reconstructed XML should match original for {}", filename);
        }
    }
    
    #[test]
    fn test_reconstruction_verification_error_is_typed() {
        // Test that reconstruction verification errors use the typed ParserError
        // and can be identified via downcast from anyhow::Error
        let err: anyhow::Error = ParserError::ReconstructionVerificationFailed {
            filename: "test.xml".to_string(),
            details: "length mismatch (original: 100 bytes, reconstructed: 90 bytes)".to_string(),
        }.into();

        // Should be dowcastable to ParserError
        let parser_err = err.downcast_ref::<ParserError>()
            .expect("Error should downcast to ParserError");

        // Should be critical
        assert!(parser_err.is_critical(),
            "ReconstructionVerificationFailed should be critical");

        // Display should contain structured info
        let display = format!("{}", err);
        assert!(display.contains("test.xml"), "Display should contain filename");
        assert!(display.contains("length mismatch"), "Display should contain details");
    }
}
