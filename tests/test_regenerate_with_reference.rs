//! Integration tests for "Regenerate Using Current DB as Reference" flow
//!
//! These tests replicate the action of the web UI "Regenerate Using Current DB
//! as Reference" button.
//!
//! Test data:
//! - `tests/data/fragments-for-testing.sqlite3`
//! - `tests/data/regenerate-test-config.toml` - config with adapted paths
//!
//! The flow:
//! 1. Read config from `tests/data/regenerate-test-config.toml`
//! 2. Extract correction overrides from `fragments.sqlite3` (acting as reference)
//! 3. Load fragment adjustments from embedded TSV
//! 4. Build `ParserOverrides` combining both
//! 5. Create `TipitakaImporter` with overrides
//! 6. For each XML file: `importer.export_fragments(xml_path, new_db_path)`
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
use tipitaka_xml_parser::web::arangodb;

/// Path to the test-specific config (relative to project root where cargo runs)
const TEST_CONFIG_PATH: &str = "tests/data/regenerate-test-config.toml";

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
/// 2. Extracts correction overrides
/// 3. Fetches Pali titles from ArangoDB (if available)
/// 4. Creates a temp dir with a new DB path for output
///
/// Returns (temp_dir, new_db_path, importer, settings).
fn setup_regeneration() -> (TempDir, PathBuf, TipitakaImporter, AppSettings) {
    let settings = load_test_config();

    // Verify xml_dir exists
    let xml_dir = PathBuf::from(&settings.xml_dir);
    assert!(xml_dir.exists(), "XML directory not found at {:?}", xml_dir);

    // Extract correction overrides
    let db_path = PathBuf::from(&settings.db_path);
    let correction_overrides = extract_all_correction_overrides(&db_path)
        .expect("Failed to extract correction overrides");

    eprintln!("Loaded {} correction overrides from reference database", correction_overrides.len());

    // Load fragment adjustments from embedded TSV
    let adjustments = load_fragment_adjustments()
        .expect("Failed to load fragment adjustments");

    eprintln!("Loaded {} fragment adjustments", adjustments.len());

    // Try to fetch Pali titles from ArangoDB (may fail if ArangoDB not running)
    let pali_titles = tokio::runtime::Runtime::new()
        .expect("Failed to create tokio runtime")
        .block_on(async {
            match arangodb::get_pali_titles().await {
                Ok(titles) => {
                    eprintln!("Loaded {} Pali titles from ArangoDB", titles.len());
                    Some(titles)
                }
                Err(e) => {
                    eprintln!("Warning: Could not fetch Pali titles from ArangoDB: {}", e);
                    eprintln!("Note: sc_sutta titles will not be populated for propagated sc_codes");
                    None
                }
            }
        });

    // Build ParserOverrides
    let overrides = ParserOverrides {
        adjustments: Some(adjustments),
        correction_overrides: Some(correction_overrides),
        pali_titles,
    };

    // Create importer with overrides AND reference DB for row count validation
    let importer = TipitakaImporter::new()
        .expect("Failed to create importer")
        .with_overrides(overrides)
        .with_reference_db(db_path.clone());  // Use the same DB as reference for validation

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

/// Test regeneration of a few DN files to catch errors quickly.
#[test]
fn test_regenerate_dn_files() {
    let (_temp_dir, new_db_path, importer, settings) = setup_regeneration();

    let dn_files = &[
        "s0101m.mul.xml",
        "s0101a.att.xml",
        "s0101t.tik.xml",
        "s0102m.mul.xml",
        "s0102a.att.xml",
        "s0102t.tik.xml",
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
///
/// Also verifies that sc_code propagation works correctly:
/// - s0301m.mul.xml frag_idx 162 has "checked" sc_code "sn5.1"
/// - Fragments 163-171 should have sc_code filled in after regeneration
/// - Fragment 172 has sc_code "sn6.1"
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

    // Verify sc_code propagation for s0301m.mul.xml
    verify_s0301m_sc_code_propagation(&new_db_path);

    // Verify fragment type preservation for s0101m.mul.xml frag_idx 14
    verify_s0101m_frag_14_type(&new_db_path);

    // Verify that first and last fragments of each file are Headers
    verify_first_last_fragments_are_headers(&new_db_path);
}

/// Test single-file reparsing for s0301m.mul.xml with sc_code propagation.
///
/// This test verifies that single-file reparsing correctly fills in sc_code values:
/// - frag_idx 162 has "checked" sc_code "sn5.1"
/// - frag_idx 163-171 should get sc_code filled in after reparsing
/// - frag_idx 172 has sc_code "sn6.1"
#[test]
fn test_reparse_s0301m_with_sc_code_propagation() {
    let (_temp_dir, new_db_path, importer, settings) = setup_regeneration();

    let filename = "s0301m.mul.xml";
    let path = xml_path(&settings, filename);
    assert!(path.exists(), "XML file not found: {:?}", path);

    eprintln!("Reparsing single file: {}", filename);

    let result = importer.export_fragments(&path, &new_db_path);

    match &result {
        Ok(count) => eprintln!("{}: exported {} fragments successfully", filename, count),
        Err(e) => eprintln!("{}: FAILED - {}", filename, e),
    }

    assert!(result.is_ok(),
        "Failed to export fragments for {}: {}", filename, result.unwrap_err());

    // Verify sc_code propagation
    verify_s0301m_sc_code_propagation(&new_db_path);
}

/// Helper function to verify sc_code propagation for s0301m.mul.xml fragments 162-172.
///
/// Checks that:
/// - frag_idx 162 has sc_code "sn5.1" (from checked override)
/// - frag_idx 163-171 have non-null sc_code values (propagated)
/// - frag_idx 172 has sc_code "sn6.1"
/// - All fragments with sc_code also have non-null sc_sutta titles
fn verify_s0301m_sc_code_propagation(db_path: &Path) {
    use diesel::prelude::*;
    use diesel::sqlite::SqliteConnection;
    use tipitaka_xml_parser::fragments_schema::xml_fragments;

    let mut conn = SqliteConnection::establish(db_path.to_str().unwrap())
        .expect("Failed to connect to database");

    // Query fragments 162-172 from s0301m.mul.xml
    let fragments: Vec<(i32, Option<String>, Option<String>)> = xml_fragments::table
        .filter(xml_fragments::cst_file.eq("s0301m.mul.xml"))
        .filter(xml_fragments::frag_idx.ge(162))
        .filter(xml_fragments::frag_idx.le(172))
        .select((xml_fragments::frag_idx, xml_fragments::sc_code, xml_fragments::sc_sutta))
        .order_by(xml_fragments::frag_idx.asc())
        .load(&mut conn)
        .expect("Failed to query fragments");

    eprintln!("\nVerifying sc_code and sc_sutta propagation for s0301m.mul.xml:");
    for (frag_idx, sc_code, sc_sutta) in &fragments {
        eprintln!("  frag_idx {}: sc_code = {:?}, sc_sutta = {:?}", frag_idx, sc_code, sc_sutta);
    }

    // Verify frag_idx 162 has sc_code "sn5.1"
    let frag_162 = fragments.iter().find(|(idx, _, _)| *idx == 162)
        .expect("Fragment 162 not found");
    assert_eq!(
        frag_162.1.as_deref(),
        Some("sn5.1"),
        "frag_idx 162 should have sc_code 'sn5.1' from checked override"
    );

    // Verify frag_idx 163-171 have non-null sc_code values (should be propagated)
    for frag_idx in 163..=171 {
        let frag = fragments.iter().find(|(idx, _, _)| *idx == frag_idx)
            .expect(&format!("Fragment {} not found", frag_idx));
        assert!(
            frag.1.is_some(),
            "frag_idx {} should have sc_code filled in after regeneration (was null before), got: {:?}",
            frag_idx,
            frag.1
        );
    }

    // Verify frag_idx 172 has sc_code "sn6.1"
    let frag_172 = fragments.iter().find(|(idx, _, _)| *idx == 172)
        .expect("Fragment 172 not found");
    assert_eq!(
        frag_172.1.as_deref(),
        Some("sn6.1"),
        "frag_idx 172 should have sc_code 'sn6.1'"
    );

    eprintln!("✓ sc_code propagation verified successfully");

    // Verify that all fragments with sc_code also have sc_sutta titles
    eprintln!("\nVerifying sc_sutta titles are populated:");
    for (frag_idx, sc_code, sc_sutta) in &fragments {
        if sc_code.is_some() {
            assert!(
                sc_sutta.is_some() && !sc_sutta.as_ref().unwrap().is_empty(),
                "frag_idx {} has sc_code {:?} but sc_sutta is missing or empty: {:?}",
                frag_idx,
                sc_code,
                sc_sutta
            );
            eprintln!("  ✓ frag_idx {}: sc_sutta = {:?}", frag_idx, sc_sutta);
        }
    }

    eprintln!("✓ sc_sutta titles verified successfully");
}

/// Helper function to verify that s0101m.mul.xml frag_idx 14 remains "Header" type.
///
/// This fragment has a "checked" review status correction override where the type
/// is "Header". After regeneration with corrections applied, it should remain "Header",
/// not change to "Sutta".
fn verify_s0101m_frag_14_type(db_path: &Path) {
    use diesel::prelude::*;
    use diesel::sqlite::SqliteConnection;
    use tipitaka_xml_parser::fragments_schema::xml_fragments;

    let mut conn = SqliteConnection::establish(db_path.to_str().unwrap())
        .expect("Failed to connect to database");

    // Query fragment 14 from s0101m.mul.xml
    let fragment: Option<(i32, String)> = xml_fragments::table
        .filter(xml_fragments::cst_file.eq("s0101m.mul.xml"))
        .filter(xml_fragments::frag_idx.eq(14))
        .select((xml_fragments::frag_idx, xml_fragments::frag_type))
        .first(&mut conn)
        .optional()
        .expect("Failed to query fragment");

    match fragment {
        Some((frag_idx, frag_type)) => {
            eprintln!("\nVerifying s0101m.mul.xml frag_idx 14:");
            eprintln!("  frag_idx {}: frag_type = {:?}", frag_idx, frag_type);

            assert_eq!(
                frag_type,
                "Header",
                "s0101m.mul.xml frag_idx 14 should remain 'Header' type (has 'checked' override), but got '{}'",
                frag_type
            );

            eprintln!("✓ s0101m.mul.xml frag_idx 14 type verified as 'Header'");
        }
        None => {
            panic!("s0101m.mul.xml frag_idx 14 not found in database");
        }
    }
}

/// Helper function to verify that for every distinct cst_file,
/// the first and last frag_idx are "Header" type fragments.
///
/// This is a structural invariant: XML files should always start and end
/// with Header fragments (containing metadata), not Sutta fragments.
fn verify_first_last_fragments_are_headers(db_path: &Path) {
    use diesel::prelude::*;
    use diesel::sqlite::SqliteConnection;
    use tipitaka_xml_parser::fragments_schema::xml_fragments;

    let mut conn = SqliteConnection::establish(db_path.to_str().unwrap())
        .expect("Failed to connect to database");

    // Get all distinct cst_file values
    let files: Vec<String> = xml_fragments::table
        .select(xml_fragments::cst_file)
        .distinct()
        .order_by(xml_fragments::cst_file.asc())
        .load(&mut conn)
        .expect("Failed to query distinct cst_file values");

    eprintln!("\nVerifying first and last fragments are Headers for {} files:", files.len());

    let mut errors: Vec<String> = Vec::new();

    for cst_file in &files {
        // Get the minimum and maximum frag_idx for this file
        let min_max: Option<(i32, i32)> = xml_fragments::table
            .filter(xml_fragments::cst_file.eq(cst_file))
            .select((
                diesel::dsl::sql::<diesel::sql_types::Integer>("MIN(frag_idx)"),
                diesel::dsl::sql::<diesel::sql_types::Integer>("MAX(frag_idx)"),
            ))
            .first(&mut conn)
            .optional()
            .expect(&format!("Failed to query min/max frag_idx for {}", cst_file));

        if let Some((min_frag_idx, max_frag_idx)) = min_max {
            // Query the first fragment type
            let first_frag_type: String = xml_fragments::table
                .filter(xml_fragments::cst_file.eq(cst_file))
                .filter(xml_fragments::frag_idx.eq(min_frag_idx))
                .select(xml_fragments::frag_type)
                .first(&mut conn)
                .expect(&format!("Failed to query first fragment for {}", cst_file));

            // Query the last fragment type
            let last_frag_type: String = xml_fragments::table
                .filter(xml_fragments::cst_file.eq(cst_file))
                .filter(xml_fragments::frag_idx.eq(max_frag_idx))
                .select(xml_fragments::frag_type)
                .first(&mut conn)
                .expect(&format!("Failed to query last fragment for {}", cst_file));

            // Check first fragment
            if first_frag_type != "Header" {
                let msg = format!(
                    "{}: first fragment (frag_idx {}) is '{}', expected 'Header'",
                    cst_file, min_frag_idx, first_frag_type
                );
                eprintln!("  ✗ {}", msg);
                errors.push(msg);
            }

            // Check last fragment
            if last_frag_type != "Header" {
                let msg = format!(
                    "{}: last fragment (frag_idx {}) is '{}', expected 'Header'",
                    cst_file, max_frag_idx, last_frag_type
                );
                eprintln!("  ✗ {}", msg);
                errors.push(msg);
            }

            if first_frag_type == "Header" && last_frag_type == "Header" {
                eprintln!("  ✓ {}: first={}, last={}", cst_file, min_frag_idx, max_frag_idx);
            }
        }
    }

    assert!(
        errors.is_empty(),
        "\nFirst/Last fragment Header validation failed for {} file(s):\n{}",
        errors.len(),
        errors.join("\n")
    );

    eprintln!("✓ All {} files have Header fragments as first and last", files.len());
}

/// Test that s0302m.mul.xml frag_idx 146 has sc_code and sc_sutta populated.
///
/// This tests the fix for when derived cst_code values are not in the lookup data:
/// - If the derived cst_code is not in the lookup data, compare the current fragment's
///   cst_code with the previous fragment's sc_code
/// - If only the sutta number increased (e.g., sn15.20 → sn15.21), increment the sutta
/// - If a new group started (e.g., sn15.20 → sn16.1), increment group and reset sutta to 1
/// - Use pali_titles from ArangoDB for sc_sutta lookup
#[test]
fn test_s0302m_frag_146_sc_code_propagation() {
    use tipitaka_xml_parser::nikaya_detector::detect_nikaya_structure;
    use tipitaka_xml_parser::parse_into_fragments;
    use tipitaka_xml_parser::types::ParserOverrides;

    let pali_titles = tokio::runtime::Runtime::new()
        .expect("Failed to create tokio runtime")
        .block_on(async {
            match arangodb::get_pali_titles().await {
                Ok(titles) => Some(titles),
                Err(_) => None,
            }
        });

    let overrides = ParserOverrides {
        pali_titles,
        ..Default::default()
    };

    let xml_content = std::fs::read_to_string("tests/data/s0302m.mul.xml")
        .expect("Failed to read s0302m.mul.xml");

    let structure = detect_nikaya_structure(&xml_content)
        .expect("Failed to detect nikaya structure");

    let fragments = parse_into_fragments(
        &xml_content,
        &structure,
        "s0302m.mul.xml",
        &overrides,
        true,
    ).expect("Failed to parse fragments");

    let fragment = fragments.get(146).expect("Fragment 146 not found in s0302m.mul.xml");

    assert!(
        fragment.sc_code.is_some() && !fragment.sc_code.as_ref().unwrap().is_empty(),
        "frag_idx 146 should have sc_code populated, got {:?}",
        fragment.sc_code
    );

    assert!(
        fragment.sc_sutta.is_some() && !fragment.sc_sutta.as_ref().unwrap().is_empty(),
        "frag_idx 146 should have sc_sutta populated, got {:?}",
        fragment.sc_sutta
    );

    assert_eq!(
        fragment.sc_code.as_deref(),
        Some("sn16.1"),
        "frag_idx 146 should have sc_code 'sn16.1'"
    );

    assert_eq!(
        fragment.sc_sutta.as_deref(),
        Some("Santuṭṭhasutta"),
        "frag_idx 146 should have sc_sutta 'Santuṭṭhasutta' from ArangoDB"
    );
}

/// Test that s0302m.mul.xml frag_idx 76 has sc_code and sc_sutta populated after a range.
///
/// This tests the case when the previous fragment has a range sc_code (e.g., sn12.93-103).
/// The propagate_sc_codes_from_previous() should handle ranges correctly.
#[test]
fn test_s0302m_frag_76_after_range() {
    use tipitaka_xml_parser::nikaya_detector::detect_nikaya_structure;
    use tipitaka_xml_parser::parse_into_fragments;
    use tipitaka_xml_parser::types::ParserOverrides;

    let pali_titles = tokio::runtime::Runtime::new()
        .expect("Failed to create tokio runtime")
        .block_on(async {
            match arangodb::get_pali_titles().await {
                Ok(titles) => Some(titles),
                Err(_) => None,
            }
        });

    let overrides = ParserOverrides {
        pali_titles,
        ..Default::default()
    };

    let xml_content = std::fs::read_to_string("tests/data/s0302m.mul.xml")
        .expect("Failed to read s0302m.mul.xml");

    let structure = detect_nikaya_structure(&xml_content)
        .expect("Failed to detect nikaya structure");

    let fragments = parse_into_fragments(
        &xml_content,
        &structure,
        "s0302m.mul.xml",
        &overrides,
        true,
    ).expect("Failed to parse fragments");

    // frag_idx 75 has range sc_code sn12.93-103
    // frag_idx 76 should have sc_code derived from previous
    let frag_76 = fragments.get(76).expect("Fragment 76 not found");

    assert!(
        frag_76.sc_code.is_some() && !frag_76.sc_code.as_ref().unwrap().is_empty(),
        "frag_idx 76 should have sc_code populated, got {:?}",
        frag_76.sc_code
    );

    assert!(
        frag_76.sc_sutta.is_some() && !frag_76.sc_sutta.as_ref().unwrap().is_empty(),
        "frag_idx 76 should have sc_sutta populated, got {:?}",
        frag_76.sc_sutta
    );

    assert_eq!(
        frag_76.sc_code.as_deref(),
        Some("sn13.1"),
        "frag_idx 76 should have sc_code 'sn13.1'"
    );

    assert_eq!(
        frag_76.sc_sutta.as_deref(),
        Some("Nakhasikhāsutta"),
        "frag_idx 76 should have sc_sutta 'Nakhasikhāsutta' from ArangoDB"
    );
}
