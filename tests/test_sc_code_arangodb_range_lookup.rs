//! Tests for sc_code validation against ArangoDB with range lookup support
//!
//! When validating sc_codes against ArangoDB, expanded codes from a range
//! (e.g. 'sn12.72' from 'sn12.71-81') must also match range keys in ArangoDB
//! (e.g. 'sn12.72-81').

use std::collections::HashMap;
use tipitaka_xml_parser::web::validation::{expand_sc_code_range, sc_code_exists_in_titles};

/// Build a mock ArangoDB titles HashMap with range keys matching real data.
fn mock_arangodb_titles() -> HashMap<String, String> {
    let mut titles = HashMap::new();

    // sn12.x range keys (Nidānasaṃyutta atthakathā)
    // In ArangoDB, sn12.72-81 exists as a single ranged entry
    titles.insert("sn12.72-81".to_string(), "Sutta 72-81".to_string());
    // Some individual entries that exist as exact matches
    titles.insert("sn12.71".to_string(), "Sutta 71".to_string());

    // sn35.x range keys (Saḷāyatanasaṃyutta atthakathā)
    // In ArangoDB, sn35.171-173 exists as a single ranged entry
    titles.insert("sn35.171-173".to_string(), "Sutta 171-173".to_string());
    titles.insert("sn35.168".to_string(), "Sutta 168".to_string());

    // sn43.x range keys (Asaṅkhatasaṃyutta)
    // In ArangoDB, sn43.14-43 exists as a single ranged entry
    titles.insert("sn43.14-43".to_string(), "Sutta 14-43".to_string());
    titles.insert("sn43.12".to_string(), "Sutta 12".to_string());
    titles.insert("sn43.13".to_string(), "Sutta 13".to_string());

    titles
}

/// Test: sc_code 'sn12.72' expanded from 'sn12.71-81' should be found
/// because ArangoDB has the range key 'sn12.72-81'.
///
/// cst_file: s0302a.att.xml
#[test]
fn test_sn12_72_found_in_arangodb_range_sn12_72_81() {
    let titles = mock_arangodb_titles();

    // 'sn12.72' is not an exact key, but falls within range 'sn12.72-81'
    assert!(
        sc_code_exists_in_titles("sn12.72", &titles),
        "sn12.72 should be found within ArangoDB range key sn12.72-81"
    );
}

/// Test: all expanded codes from 'sn12.71-81' should be found in ArangoDB.
///
/// sn12.71 exists as an exact key.
/// sn12.72 through sn12.81 exist within the range key 'sn12.72-81'.
///
/// cst_file: s0302a.att.xml
#[test]
fn test_sn12_71_81_all_expanded_found() {
    let titles = mock_arangodb_titles();
    let expanded = expand_sc_code_range("sn12.71-81");

    assert_eq!(expanded.len(), 11, "sn12.71-81 should expand to 11 codes");
    assert_eq!(expanded[0], "sn12.71");
    assert_eq!(expanded[10], "sn12.81");

    let missing: Vec<&String> = expanded
        .iter()
        .filter(|code| !sc_code_exists_in_titles(code, &titles))
        .collect();

    assert!(
        missing.is_empty(),
        "All expanded codes from sn12.71-81 should be found in ArangoDB, but missing: {:?}",
        missing
    );
}

/// Test: sc_code 'sn35.171' expanded from 'sn35.168-227' should be found
/// because ArangoDB has the range key 'sn35.171-173'.
///
/// cst_file: s0304a.att.xml
#[test]
fn test_sn35_171_found_in_arangodb_range_sn35_171_173() {
    let titles = mock_arangodb_titles();

    assert!(
        sc_code_exists_in_titles("sn35.171", &titles),
        "sn35.171 should be found within ArangoDB range key sn35.171-173"
    );
    assert!(
        sc_code_exists_in_titles("sn35.172", &titles),
        "sn35.172 should be found within ArangoDB range key sn35.171-173"
    );
    assert!(
        sc_code_exists_in_titles("sn35.173", &titles),
        "sn35.173 should be found within ArangoDB range key sn35.171-173"
    );
}

/// Test: sc_code 'sn43.14' expanded from 'sn43.12-44' should be found
/// because ArangoDB has the range key 'sn43.14-43'.
///
/// sn43.12 and sn43.13 exist as exact keys.
/// sn43.14 through sn43.43 exist within the range key 'sn43.14-43'.
#[test]
fn test_sn43_14_found_in_arangodb_range_sn43_14_43() {
    let titles = mock_arangodb_titles();

    // Exact matches
    assert!(
        sc_code_exists_in_titles("sn43.12", &titles),
        "sn43.12 should be found as exact key"
    );
    assert!(
        sc_code_exists_in_titles("sn43.13", &titles),
        "sn43.13 should be found as exact key"
    );

    // Range matches within sn43.14-43
    assert!(
        sc_code_exists_in_titles("sn43.14", &titles),
        "sn43.14 should be found within ArangoDB range key sn43.14-43"
    );
    assert!(
        sc_code_exists_in_titles("sn43.30", &titles),
        "sn43.30 should be found within ArangoDB range key sn43.14-43"
    );
    assert!(
        sc_code_exists_in_titles("sn43.43", &titles),
        "sn43.43 should be found within ArangoDB range key sn43.14-43"
    );
}

/// Test: a code that truly doesn't exist should NOT be found.
#[test]
fn test_nonexistent_code_not_found() {
    let titles = mock_arangodb_titles();

    assert!(
        !sc_code_exists_in_titles("sn12.50", &titles),
        "sn12.50 should NOT be found (not in any key or range)"
    );
    assert!(
        !sc_code_exists_in_titles("sn99.1", &titles),
        "sn99.1 should NOT be found (no sn99 prefix at all)"
    );
}

/// Test: code at the boundary of a range is found (start and end inclusive).
#[test]
fn test_range_boundary_inclusive() {
    let titles = mock_arangodb_titles();

    // Start of range sn12.72-81
    assert!(
        sc_code_exists_in_titles("sn12.72", &titles),
        "sn12.72 (start of range sn12.72-81) should be found"
    );
    // End of range sn12.72-81
    assert!(
        sc_code_exists_in_titles("sn12.81", &titles),
        "sn12.81 (end of range sn12.72-81) should be found"
    );
    // Just outside the range
    assert!(
        !sc_code_exists_in_titles("sn12.82", &titles),
        "sn12.82 (outside range sn12.72-81) should NOT be found"
    );
}

/// Test: expand_sc_code_range correctly expands the example ranges.
#[test]
fn test_expand_sc_code_range_examples() {
    let expanded = expand_sc_code_range("sn12.71-81");
    assert_eq!(expanded.len(), 11);
    assert_eq!(expanded.first().unwrap(), "sn12.71");
    assert_eq!(expanded.last().unwrap(), "sn12.81");

    let expanded = expand_sc_code_range("sn35.168-227");
    assert_eq!(expanded.len(), 60);
    assert_eq!(expanded.first().unwrap(), "sn35.168");
    assert_eq!(expanded.last().unwrap(), "sn35.227");

    let expanded = expand_sc_code_range("sn43.12-44");
    assert_eq!(expanded.len(), 33);
    assert_eq!(expanded.first().unwrap(), "sn43.12");
    assert_eq!(expanded.last().unwrap(), "sn43.44");
}

/// Test: non-range codes pass through expand_sc_code_range unchanged.
#[test]
fn test_expand_non_range_code() {
    let expanded = expand_sc_code_range("sn12.71");
    assert_eq!(expanded, vec!["sn12.71"]);

    let expanded = expand_sc_code_range("dn1");
    assert_eq!(expanded, vec!["dn1"]);
}
