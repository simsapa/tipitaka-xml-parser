//! Regeneration functionality for fragments database
//!
//! This module provides functions to regenerate the fragments database from XML files,
//! optionally using a reference database to preserve correction overrides and review status.

use std::fs;
use std::path::{Path, PathBuf};
use anyhow::{Result, Context};

use crate::logger;
use crate::types::{ParserError, ParserOverrides};
use crate::fragment_exporter::extract_all_correction_overrides;
use crate::{
    TipitakaImporter,
    export_fragments_to_tsv,
    check_tsv_improvements,
    validate_fragments_db,
};

/// Configuration for the regeneration process
#[derive(Debug, Clone)]
pub struct RegenerateConfig {
    /// Path to the current (working) database
    pub db_path: PathBuf,
    /// Path where the new fragments database will be created
    pub new_db_path: PathBuf,
    /// Path where the reference database will be stored (copy of current before regeneration)
    pub reference_db_path: PathBuf,
    /// Path for the new TSV export
    pub new_tsv_path: PathBuf,
    /// Path for the reference TSV export
    pub reference_tsv_path: PathBuf,
    /// Directory containing XML files
    pub xml_dir: PathBuf,
    /// List of XML filenames to process
    pub xml_filenames: Vec<String>,
    /// Whether to use the current database as reference for preserving corrections
    pub use_reference_db: bool,
    /// Pali titles from ArangoDB for sc_sutta lookup
    pub pali_titles: Option<std::collections::HashMap<String, String>>,
}

/// Result of the regeneration process
#[derive(Debug)]
pub struct RegenerateResult {
    /// Whether the regeneration was successful
    pub success: bool,
    /// Detailed output/log of the regeneration process
    pub output: String,
    /// Whether the current database was replaced with the new one
    pub db_replaced: bool,
}

impl RegenerateResult {
    fn error(output: String) -> Self {
        Self {
            success: false,
            output,
            db_replaced: false,
        }
    }
}

/// Regenerate the fragments database from XML files
///
/// This function performs the complete regeneration workflow:
/// 1. Optionally copy current database to reference database
/// 2. Parse all XML files and export fragments to new database
/// 3. Export fragments to TSV for comparison
/// 4. Check for regressions against reference TSV
/// 5. Replace current database with new one
///
/// # Arguments
/// * `config` - Configuration for the regeneration process
///
/// # Returns
/// * `RegenerateResult` containing success status, output log, and whether DB was replaced
pub fn regenerate_fragments_db(config: &RegenerateConfig) -> RegenerateResult {
    let mut output = String::new();

    // Step 0: Copy current database to reference database (only if using reference)
    if config.use_reference_db {
        output.push_str("=== Preparing Reference Database ===\n");

        if config.db_path.exists() {
            output.push_str(&format!("Copying {:?} to {:?}\n", config.db_path, config.reference_db_path));

            match fs::copy(&config.db_path, &config.reference_db_path) {
                Ok(bytes) => {
                    output.push_str(&format!("Copied {} bytes successfully\n\n", bytes));
                }
                Err(e) => {
                    return RegenerateResult::error(
                        format!("{}ERROR: Failed to copy database to reference: {}\n", output, e)
                    );
                }
            }
        } else {
            return RegenerateResult::error(
                format!("{}ERROR: Current database does not exist: {:?}\n", output, config.db_path)
            );
        }
    } else {
        output.push_str("=== Generating Fresh Database ===\n");
        output.push_str("Not using reference database - generating completely new fragments\n\n");
    }

    // Build list of XML file paths
    let xml_files: Vec<PathBuf> = config.xml_filenames
        .iter()
        .map(|filename| config.xml_dir.join(filename))
        .collect();

    output.push_str(&format!("Processing {} XML files\n\n", xml_files.len()));

    // Step 1: Parse Tipitaka XML
    output.push_str("=== Parsing Tipitaka XML ===\n");

    let reference_db = if config.use_reference_db {
        output.push_str(&format!("Using reference database: {:?}\n", config.reference_db_path));
        Some(config.reference_db_path.as_path())
    } else {
        None
    };

    match parse_tipitaka_xml_files(&xml_files, &config.new_db_path, reference_db, config.pali_titles.clone()) {
        Ok(count) => {
            output.push_str(&format!("Successfully parsed {} files\n\n", count));
        }
        Err(e) => {
            return RegenerateResult::error(
                format!("{}ERROR: Failed to parse XML files: {}\n", output, e)
            );
        }
    }

    // Step 2: Export fragments to TSV
    output.push_str("=== Exporting Fragments to TSV ===\n");
    output.push_str(&format!("Exporting from {:?} to {:?}\n", config.new_db_path, config.new_tsv_path));

    match export_fragments_to_tsv(&config.new_db_path, &config.new_tsv_path) {
        Ok(count) => {
            output.push_str(&format!("Exported {} fragments to TSV\n\n", count));
        }
        Err(e) => {
            output.push_str(&format!("ERROR: Failed to export fragments to TSV: {}\n\n", e));
        }
    }

    // Step 3: Check TSV regressions (if reference TSV exists)
    if config.reference_tsv_path.exists() {
        output.push_str("=== Checking TSV Regressions ===\n");
        output.push_str(&format!("Comparing {:?} to {:?}\n", config.reference_tsv_path, config.new_tsv_path));

        match check_tsv_improvements(&config.reference_tsv_path, &config.new_tsv_path) {
            Ok(result) => {
                output.push_str(&format!("Checked {} total rows, {} rows differ\n",
                    result.total_rows, result.differing_rows));

                if result.validation_errors.is_empty() {
                    output.push_str("No regressions detected\n\n");
                } else {
                    output.push_str(&format!("\nFound {} regressions:\n", result.validation_errors.len()));
                    for error in &result.validation_errors {
                        output.push_str(&format!("  - File: {}, Fragment: {}, CST Code: {}\n",
                            error.cst_file, error.frag_idx_code, error.cst_code));
                    }
                    output.push_str("\n");
                }
            }
            Err(e) => {
                output.push_str(&format!("ERROR: Failed to check TSV regressions: {}\n\n", e));
            }
        }
    } else {
        output.push_str("=== Skipping TSV Regression Check ===\n");
        output.push_str("Reference TSV file does not exist\n\n");
    }

    // Step 4: Replace current database with new one
    let mut db_replaced = false;
    output.push_str("=== Replacing Current Database ===\n");

    if config.new_db_path.exists() {
        output.push_str(&format!("Copying {:?} to {:?}\n", config.new_db_path, config.db_path));

        match fs::copy(&config.new_db_path, &config.db_path) {
            Ok(bytes) => {
                output.push_str(&format!("Replaced {} bytes successfully\n", bytes));
                output.push_str("Current database has been replaced with the new one.\n\n");
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

    RegenerateResult {
        success: true,
        output,
        db_replaced,
    }
}

/// Parse multiple XML files and export fragments to database
///
/// This is a library-accessible wrapper for the parse-tipitaka-xml functionality.
///
/// # Arguments
/// * `xml_files` - List of paths to XML files to parse
/// * `fragments_db` - Path to the output fragments database
/// * `reference_fragments_db` - Optional path to reference database for preserving corrections
///
/// # Returns
/// * `Ok(usize)` - Number of files successfully processed
/// * `Err` - If any critical error occurs
pub fn parse_tipitaka_xml_files(
    xml_files: &[PathBuf],
    fragments_db: &Path,
    reference_fragments_db: Option<&Path>,
    pali_titles: Option<std::collections::HashMap<String, String>>,
) -> Result<usize> {
    // Extract correction overrides from reference database if provided
    let correction_overrides = if let Some(ref_db_path) = reference_fragments_db {
        match extract_all_correction_overrides(ref_db_path) {
            Ok(overrides) => {
                if !overrides.is_empty() {
                    logger::info(&format!("Loaded {} correction overrides from reference database", overrides.len()));
                }
                Some(overrides)
            }
            Err(e) => {
                logger::warn(&format!("Failed to load correction overrides from reference: {}", e));
                None
            }
        }
    } else {
        None
    };

    // Build ParserOverrides using the provided pali_titles
    let overrides = ParserOverrides {
        correction_overrides,
        pali_titles,
    };

    if xml_files.is_empty() {
        anyhow::bail!("No XML files to process");
    }

    let mut importer = TipitakaImporter::new()
        .context("Failed to create importer")?;

    // Add parser overrides
    importer = importer.with_overrides(overrides);

    // Set reference database path for row count validation
    if let Some(ref_db_path) = reference_fragments_db {
        importer = importer.with_reference_db(ref_db_path.to_path_buf());
    }

    // Process each XML file
    let mut success_count = 0;
    let mut errors = 0;

    for (idx, xml_file) in xml_files.iter().enumerate() {
        let msg = format!("[{}/{}] Processing: {:?}",
            idx + 1, xml_files.len(),
            xml_file.file_name().unwrap_or_default());
        logger::info(&msg);

        match importer.export_fragments(xml_file, fragments_db) {
            Ok(count) => {
                logger::info(&format!("Exported {} fragments to {:?}", count, fragments_db));
                success_count += 1;
            }
            Err(e) => {
                let error_msg = format!("Error exporting fragments from {:?}: {}",
                    xml_file.file_name().unwrap_or_default(), e);
                logger::error(&error_msg);

                // Check for critical parser errors
                if let Some(parser_err) = e.downcast_ref::<ParserError>() {
                    if parser_err.is_critical() {
                        anyhow::bail!("Critical error: {}", e);
                    }
                }

                errors += 1;
            }
        }
    }

    if errors > 0 {
        logger::error(&format!("Processing completed with {} errors", errors));
    }

    // Validate the fragments database
    logger::info("Validating fragments database...");
    match validate_fragments_db(fragments_db) {
        Ok(stats) => {
            logger::info(&format!("Validation complete: {} total Sutta fragments", stats.total_sutta_fragments));

            if stats.empty_cst_code > 0 {
                logger::error(&format!("{} Sutta fragments have empty cst_code", stats.empty_cst_code));
            }
            if stats.empty_sc_code > 0 {
                logger::error(&format!("{} Sutta fragments have empty sc_code", stats.empty_sc_code));
            }
        }
        Err(e) => {
            logger::error(&format!("Failed to validate fragments database: {}", e));
        }
    }

    Ok(success_count)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_regenerate_config() {
        let config = RegenerateConfig {
            db_path: PathBuf::from("./data/fragments.sqlite3"),
            new_db_path: PathBuf::from("./data/fragments-new.sqlite3"),
            reference_db_path: PathBuf::from("./data/fragments-reference.sqlite3"),
            new_tsv_path: PathBuf::from("./data/fragments-new.tsv"),
            reference_tsv_path: PathBuf::from("./data/fragments-reference.tsv"),
            xml_dir: PathBuf::from("./xml"),
            xml_filenames: vec!["s0101m.mul.xml".to_string()],
            use_reference_db: true,
            pali_titles: None,
        };

        assert!(config.use_reference_db);
        assert_eq!(config.xml_filenames.len(), 1);
    }
}
