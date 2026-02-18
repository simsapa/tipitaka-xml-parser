//! Integration tests for fragment move operations
//!
//! This test suite validates the functionality of moving fragment content between
//! adjacent fragments, including skip-over logic for already-moved fragments,
//! boundary validation, and metadata handling.

use diesel::prelude::*;
use diesel::sqlite::SqliteConnection;
use tempfile::TempDir;
use tipitaka_xml_parser::fragment_operations::{Direction, move_fragment_content};
use tipitaka_xml_parser::fragments_models::{NewXmlFragment, NewNikayaStructure};
use tipitaka_xml_parser::fragments_schema::{xml_fragments, nikaya_structures};

/// Helper function to create a test database with sample fragments
///
/// Returns a tuple of (TempDir, SqliteConnection) where the TempDir must be kept alive
/// for the duration of the test to prevent the temporary directory from being deleted.
fn setup_test_db() -> (TempDir, SqliteConnection) {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let db_path = temp_dir.path().join("test_fragments.db");
    
    // Create connection
    let mut conn = SqliteConnection::establish(db_path.to_str().unwrap())
        .expect("Failed to create test database");
    
    // Run migrations
    diesel::sql_query(
        "CREATE TABLE nikaya_structures (
            id INTEGER PRIMARY KEY NOT NULL,
            nikaya TEXT NOT NULL UNIQUE,
            levels TEXT NOT NULL,
            created_at TIMESTAMP
        )"
    )
    .execute(&mut conn)
    .expect("Failed to create nikaya_structures table");
    
    diesel::sql_query(
        "CREATE TABLE xml_fragments (
            id INTEGER PRIMARY KEY NOT NULL,
            cst_file TEXT NOT NULL,
            frag_idx_code TEXT NOT NULL,
            frag_type TEXT NOT NULL,
            frag_review TEXT,
            nikaya TEXT NOT NULL,
            cst_code TEXT,
            sc_code TEXT,
            content_xml TEXT NOT NULL,
            content_html TEXT,
            cst_vagga TEXT,
            cst_sutta TEXT,
            cst_paranum TEXT,
            sc_sutta TEXT,
            start_line INTEGER NOT NULL,
            start_char INTEGER NOT NULL,
            end_line INTEGER NOT NULL,
            end_char INTEGER NOT NULL,
            group_levels TEXT NOT NULL,
            created_at TIMESTAMP,
            FOREIGN KEY (nikaya) REFERENCES nikaya_structures(nikaya),
            UNIQUE(cst_file, frag_idx_code)
        )"
    )
    .execute(&mut conn)
    .expect("Failed to create xml_fragments table");
    
    // Insert nikaya structure
    let nikaya = NewNikayaStructure {
        nikaya: "test_nikaya",
        levels: "[]",
    };
    
    diesel::insert_into(nikaya_structures::table)
        .values(&nikaya)
        .execute(&mut conn)
        .expect("Failed to insert nikaya structure");
    
    (temp_dir, conn)
}

/// Helper function to insert a test fragment
fn insert_fragment(
    conn: &mut SqliteConnection,
    cst_file: &str,
    frag_idx_code: &str,
    content: &str,
    start_line: i32,
    end_line: i32,
    frag_review: Option<&str>,
) -> i32 {
    let fragment = NewXmlFragment {
        cst_file,
        frag_idx_code,
        frag_type: "sutta",
        frag_review,
        nikaya: "test_nikaya",
        cst_code: Some("test_code"),
        sc_code: Some("sc_code"),
        content_xml: content,
        content_html: None,
        cst_vagga: Some("vagga1"),
        cst_sutta: Some("sutta1"),
        cst_paranum: Some("1"),
        sc_sutta: Some("sc1"),
        start_line,
        start_char: 0,
        end_line,
        end_char: 100,
        group_levels: "[]",
    };

    diesel::insert_into(xml_fragments::table)
        .values(&fragment)
        .execute(conn)
        .expect("Failed to insert fragment");

    // Return the ID of the inserted fragment
    xml_fragments::table
        .filter(xml_fragments::cst_file.eq(cst_file))
        .filter(xml_fragments::frag_idx_code.eq(frag_idx_code))
        .select(xml_fragments::id)
        .first(conn)
        .expect("Failed to retrieve inserted fragment ID")
}

#[test]
fn test_move_to_prev_basic() {
    let (_temp_dir, mut conn) = setup_test_db();
    
    // Insert two fragments
    insert_fragment(&mut conn, "test.xml", "1.0", "Fragment 1 content", 1, 10, None);
    insert_fragment(&mut conn, "test.xml", "2.0", "Fragment 2 content", 11, 20, None);
    
    // Move fragment 2 to previous
    let result = move_fragment_content(&mut conn, "test.xml", "2.0", Direction::Prev);
    assert!(result.is_ok(), "Move operation should succeed");
    
    let (current, target) = result.unwrap();
    
    // Verify current fragment (frag_idx=2) is now empty and marked as moved
    assert_eq!(current.frag_idx_code, "2.0");
    assert_eq!(current.content_xml, "");
    assert_eq!(current.frag_review, Some("moved".to_string()));
    assert_eq!(current.cst_code, None);
    assert_eq!(current.sc_code, None);
    assert_eq!(current.cst_vagga, None);
    assert_eq!(current.cst_sutta, None);
    
    // Verify target fragment (frag_idx=1) has merged content and updated boundaries
    assert_eq!(target.frag_idx_code, "1.0");
    assert!(target.content_xml.contains("Fragment 1 content"));
    assert!(target.content_xml.contains("Fragment 2 content"));
    assert_eq!(target.start_line, 1);
    assert_eq!(target.end_line, 20); // Should extend to include fragment 2's end
}

#[test]
fn test_move_to_next_basic() {
    let (_temp_dir, mut conn) = setup_test_db();
    
    // Insert two fragments
    insert_fragment(&mut conn, "test.xml", "1.0", "Fragment 1 content", 1, 10, None);
    insert_fragment(&mut conn, "test.xml", "2.0", "Fragment 2 content", 11, 20, None);
    
    // Move fragment 1 to next
    let result = move_fragment_content(&mut conn, "test.xml", "1.0", Direction::Next);
    assert!(result.is_ok(), "Move operation should succeed");
    
    let (current, target) = result.unwrap();
    
    // Verify current fragment (frag_idx=1) is now empty and marked as moved
    assert_eq!(current.frag_idx_code, "1.0");
    assert_eq!(current.content_xml, "");
    assert_eq!(current.frag_review, Some("moved".to_string()));
    
    // Verify target fragment (frag_idx=2) has merged content and updated boundaries
    assert_eq!(target.frag_idx_code, "2.0");
    assert!(target.content_xml.contains("Fragment 1 content"));
    assert!(target.content_xml.contains("Fragment 2 content"));
    assert_eq!(target.start_line, 1); // Should extend to include fragment 1's start
    assert_eq!(target.end_line, 20);
}

#[test]
fn test_skip_moved_fragments_to_prev() {
    let (_temp_dir, mut conn) = setup_test_db();
    
    // Insert three fragments, with middle one already marked as moved
    insert_fragment(&mut conn, "test.xml", "1.0", "Fragment 1 content", 1, 10, None);
    insert_fragment(&mut conn, "test.xml", "2.0", "", 11, 20, Some("moved"));
    insert_fragment(&mut conn, "test.xml", "3.0", "Fragment 3 content", 21, 30, None);
    
    // Move fragment 3 to previous - should skip fragment 2 and merge with fragment 1
    let result = move_fragment_content(&mut conn, "test.xml", "3.0", Direction::Prev);
    assert!(result.is_ok(), "Move operation should succeed");
    
    let (current, target) = result.unwrap();
    
    // Verify current fragment (frag_idx=3) is now empty and marked as moved
    assert_eq!(current.frag_idx_code, "3.0");
    assert_eq!(current.content_xml, "");
    assert_eq!(current.frag_review, Some("moved".to_string()));
    
    // Verify target fragment (frag_idx=1) has merged content (skipped fragment 2)
    assert_eq!(target.frag_idx_code, "1.0");
    assert!(target.content_xml.contains("Fragment 1 content"));
    assert!(target.content_xml.contains("Fragment 3 content"));
    assert!(!target.content_xml.contains("Fragment 2"));
}

#[test]
fn test_skip_moved_fragments_to_next() {
    let (_temp_dir, mut conn) = setup_test_db();
    
    // Insert three fragments, with middle one already marked as moved
    insert_fragment(&mut conn, "test.xml", "1.0", "Fragment 1 content", 1, 10, None);
    insert_fragment(&mut conn, "test.xml", "2.0", "", 11, 20, Some("moved"));
    insert_fragment(&mut conn, "test.xml", "3.0", "Fragment 3 content", 21, 30, None);
    
    // Move fragment 1 to next - should skip fragment 2 and merge with fragment 3
    let result = move_fragment_content(&mut conn, "test.xml", "1.0", Direction::Next);
    assert!(result.is_ok(), "Move operation should succeed");
    
    let (current, target) = result.unwrap();
    
    // Verify current fragment (frag_idx=1) is now empty and marked as moved
    assert_eq!(current.frag_idx_code, "1.0");
    assert_eq!(current.content_xml, "");
    assert_eq!(current.frag_review, Some("moved".to_string()));
    
    // Verify target fragment (frag_idx=3) has merged content (skipped fragment 2)
    assert_eq!(target.frag_idx_code, "3.0");
    assert!(target.content_xml.contains("Fragment 1 content"));
    assert!(target.content_xml.contains("Fragment 3 content"));
    assert!(!target.content_xml.contains("Fragment 2"));
}

#[test]
fn test_move_to_prev_boundary_error() {
    let (_temp_dir, mut conn) = setup_test_db();
    
    // Insert only one fragment
    insert_fragment(&mut conn, "test.xml", "1.0", "Fragment 1 content", 1, 10, None);
    
    // Try to move fragment 1 to previous - should fail (no previous fragment)
    let result = move_fragment_content(&mut conn, "test.xml", "1.0", Direction::Prev);
    assert!(result.is_err(), "Move operation should fail at boundary");
    
    let error_msg = result.unwrap_err().to_string();
    assert!(error_msg.contains("no valid target fragment found") || error_msg.contains("boundary"));
}

#[test]
fn test_move_to_next_boundary_error() {
    let (_temp_dir, mut conn) = setup_test_db();
    
    // Insert only one fragment
    insert_fragment(&mut conn, "test.xml", "1.0", "Fragment 1 content", 1, 10, None);
    
    // Try to move fragment 1 to next - should fail (no next fragment)
    let result = move_fragment_content(&mut conn, "test.xml", "1.0", Direction::Next);
    assert!(result.is_err(), "Move operation should fail at boundary");
    
    let error_msg = result.unwrap_err().to_string();
    assert!(error_msg.contains("no valid target fragment found") || error_msg.contains("boundary"));
}

#[test]
fn test_metadata_cleared_on_move() {
    let (_temp_dir, mut conn) = setup_test_db();
    
    // Insert fragments with metadata
    insert_fragment(&mut conn, "test.xml", "1.0", "Fragment 1 content", 1, 10, None);
    insert_fragment(&mut conn, "test.xml", "2.0", "Fragment 2 content", 11, 20, None);
    
    // Move fragment 2 to previous
    let result = move_fragment_content(&mut conn, "test.xml", "2.0", Direction::Prev);
    assert!(result.is_ok());
    
    let (current, _target) = result.unwrap();
    
    // Verify all metadata fields are cleared
    assert_eq!(current.cst_code, None);
    assert_eq!(current.sc_code, None);
    assert_eq!(current.cst_vagga, None);
    assert_eq!(current.cst_sutta, None);
    assert_eq!(current.cst_paranum, None);
    assert_eq!(current.sc_sutta, None);
    assert_eq!(current.frag_review, Some("moved".to_string()));
}

#[test]
fn test_boundaries_updated_correctly() {
    let (_temp_dir, mut conn) = setup_test_db();
    
    // Insert fragments with specific boundaries
    insert_fragment(&mut conn, "test.xml", "1.0", "Fragment 1 content", 10, 20, None);
    insert_fragment(&mut conn, "test.xml", "2.0", "Fragment 2 content", 25, 35, None);
    
    // Move fragment 2 to previous
    let result = move_fragment_content(&mut conn, "test.xml", "2.0", Direction::Prev);
    assert!(result.is_ok());
    
    let (_current, target) = result.unwrap();
    
    // Verify boundaries are updated correctly
    // Target should start at fragment 1's start and end at fragment 2's end
    assert_eq!(target.start_line, 10);
    assert_eq!(target.start_char, 0);
    assert_eq!(target.end_line, 35);
    assert_eq!(target.end_char, 100);
}
