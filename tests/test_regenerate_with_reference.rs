//! Integration tests for "Regenerate Using Current DB as Reference" flow
//!
//! These tests replicate the action of the web UI "Regenerate Using Current DB
//! as Reference" button.
//!
//! Test data:
//! - `tests/data/test-db/fragments-unmodified.sqlite3` - pristine database copy
//! - `tests/data/regenerate-test-config.toml` - config with adapted paths
//!
//! The flow:
//! 1. Read config from `tests/data/regenerate-test-config.toml`
//! 2. Copy `fragments-unmodified.sqlite3` → `fragments.sqlite3` (fresh start)
//! 3. Extract correction overrides from `fragments.sqlite3` (acting as reference)
//! 4. Load fragment adjustments from embedded TSV
//! 5. Build `ParserOverrides` combining both
//! 6. Create `TipitakaImporter` with overrides
//! 7. For each XML file: `importer.export_fragments(xml_path, new_db_path)`
//!    (includes reconstruction verification)

use std::path::{Path, PathBuf};
use std::fs;
use tempfile::TempDir;

use tipitaka_xml_parser::{
    TipitakaImporter,
    load_fragment_adjustments,
};
use tipitaka_xml_parser::types::ParserOverrides;
use tipitaka_xml_parser::fragment_exporter::extract_all_correction_overrides;
use tipitaka_xml_parser::web::models::AppSettings;

/// Path to the test-specific config (relative to project root where cargo runs)
const TEST_CONFIG_PATH: &str = "tests/data/regenerate-test-config.toml";

/// Pristine database that is never modified; copied to db_path before each test
const UNMODIFIED_DB: &str = "tests/data/test-db/fragments-unmodified.sqlite3";

/// Load test config from `tests/data/regenerate-test-config.toml`
fn load_test_config() -> AppSettings {
    let config_path = PathBuf::from(TEST_CONFIG_PATH);
    assert!(config_path.exists(),
        "Test config not found at {:?}. Run from project root.", config_path);

    let content = fs::read_to_string(&config_path)
        .expect("Failed to read test config");

    toml::from_str(&content)
        .expect("Failed to parse test config TOML")
}

/// Helper to set up the regeneration test environment.
///
/// 1. Reads config from `tests/data/regenerate-test-config.toml`
/// 2. Copies `fragments-unmodified.sqlite3` → config `db_path` (fresh start)
/// 3. Extracts correction overrides from the fresh copy
/// 4. Creates a temp dir with a new DB path for output
///
/// Returns (temp_dir, new_db_path, importer, settings).
fn setup_regeneration() -> (TempDir, PathBuf, TipitakaImporter, AppSettings) {
    let settings = load_test_config();

    // Verify the pristine database exists
    let unmodified_db = PathBuf::from(UNMODIFIED_DB);
    assert!(unmodified_db.exists(),
        "Unmodified database not found at {:?}.\n\
         Create it by running: cp data/fragments.sqlite3 {}",
        unmodified_db, UNMODIFIED_DB);

    // Verify xml_dir exists
    let xml_dir = PathBuf::from(&settings.xml_dir);
    assert!(xml_dir.exists(), "XML directory not found at {:?}", xml_dir);

    // Copy unmodified DB → db_path from config (ensures a fresh start each run)
    let db_path = PathBuf::from(&settings.db_path);
    if let Some(parent) = db_path.parent() {
        fs::create_dir_all(parent).expect("Failed to create db_path parent directory");
    }
    fs::copy(&unmodified_db, &db_path)
        .expect("Failed to copy unmodified database to db_path");

    // Extract correction overrides from the fresh copy (acting as reference)
    let correction_overrides = extract_all_correction_overrides(&db_path)
        .expect("Failed to extract correction overrides");

    eprintln!("Loaded {} correction overrides from reference database", correction_overrides.len());

    // Load fragment adjustments from embedded TSV
    let adjustments = load_fragment_adjustments()
        .expect("Failed to load fragment adjustments");

    eprintln!("Loaded {} fragment adjustments", adjustments.len());

    // Build ParserOverrides
    let overrides = ParserOverrides {
        adjustments: Some(adjustments),
        correction_overrides: Some(correction_overrides),
    };

    // Create importer with overrides
    let importer = TipitakaImporter::new()
        .expect("Failed to create importer")
        .with_overrides(overrides);

    // Create temp dir for the new output database
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let new_db_path = temp_dir.path().join("fragments-new.sqlite3");

    (temp_dir, new_db_path, importer, settings)
}

/// Resolve XML file path from config settings
fn xml_path(settings: &AppSettings, filename: &str) -> PathBuf {
    Path::new(&settings.xml_dir).join(filename)
}

/// Test regeneration of a single file that is known to fail.
///
/// The error reported was:
/// ```text
/// Reconstruction verification failed for s0101a.att.xml:
///   length mismatch (original: 827757 bytes, reconstructed: 180 bytes)
/// ```
#[test]
fn test_regenerate_single_file_s0101a_att() {
    let (_temp_dir, new_db_path, importer, settings) = setup_regeneration();

    let filename = "s0101a.att.xml";
    let path = xml_path(&settings, filename);
    assert!(path.exists(), "XML file not found: {:?}", path);

    let result = importer.export_fragments(&path, &new_db_path);

    match &result {
        Ok(count) => eprintln!("{}: exported {} fragments successfully", filename, count),
        Err(e) => eprintln!("{}: FAILED - {}", filename, e),
    }

    assert!(result.is_ok(),
        "Failed to export fragments for {}: {}", filename, result.unwrap_err());
}

/// Test regeneration of the DN files to catch errors quickly.
#[test]
fn test_regenerate_dn_files() {
    let (_temp_dir, new_db_path, importer, settings) = setup_regeneration();

    let dn_files = &[
        "s0101m.mul.xml",
        "s0102m.mul.xml",
        "s0103m.mul.xml",
        "s0101a.att.xml",
        "s0102a.att.xml",
        "s0103a.att.xml",
    ];

    for filename in dn_files {
        let path = xml_path(&settings, filename);
        assert!(path.exists(), "XML file not found: {:?}", path);

        let result = importer.export_fragments(&path, &new_db_path);

        match &result {
            Ok(count) => eprintln!("{}: exported {} fragments", filename, count),
            Err(e) => eprintln!("{}: FAILED - {}", filename, e),
        }

        assert!(result.is_ok(),
            "Failed to export fragments for {}: {}", filename, result.unwrap_err());
    }
}

/// Full regeneration test: process all XML files from the test config.
///
/// This replicates the complete "Regenerate Using Current DB as Reference" action.
#[test]
fn test_regenerate_all_files_with_reference() {
    let (_temp_dir, new_db_path, importer, settings) = setup_regeneration();

    let mut errors: Vec<String> = Vec::new();
    let total = settings.xml_filenames.len();

    for (idx, filename) in settings.xml_filenames.iter().enumerate() {
        let path = xml_path(&settings, filename);
        if !path.exists() {
            errors.push(format!("{}: XML file not found at {:?}", filename, path));
            continue;
        }

        eprintln!("[{}/{}] Processing: {}", idx + 1, total, filename);

        match importer.export_fragments(&path, &new_db_path) {
            Ok(count) => eprintln!("  -> {} fragments exported", count),
            Err(e) => {
                let msg = format!("{}: {}", filename, e);
                eprintln!("  -> FAILED: {}", msg);
                errors.push(msg);
            }
        }
    }

    assert!(errors.is_empty(),
        "Regeneration failed for {} file(s):\n{}",
        errors.len(),
        errors.join("\n"));
}
