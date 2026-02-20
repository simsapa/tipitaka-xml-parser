//! Integration tests for fragment insertion and regeneration preservation
//!
//! Tests that:
//! - Inserted fragments get correct frag_idx_code assignment
//! - Boundary adjustment correctly moves content between fragments
//! - Inserted fragments survive regeneration with correct boundaries
//! - XML reconstruction produces the original file after splitting

use std::path::PathBuf;
use tempfile::NamedTempFile;
use diesel::prelude::*;
use diesel::SqliteConnection;
use tipitaka_xml_parser::fragment_exporter::{
    export_fragments_to_db, extract_correction_overrides,
};
use tipitaka_xml_parser::nikaya_detector::detect_nikaya_structure;
use tipitaka_xml_parser::parse_into_fragments;
use tipitaka_xml_parser::types::ParserOverrides;
use tipitaka_xml_parser::fragment_operations::{insert_fragment, Direction};
use tipitaka_xml_parser::fragment_reconstructor::reconstruct_xml_with_conn;
use tipitaka_xml_parser::fragments_schema::xml_fragments;
use tipitaka_xml_parser::fragments_models::{XmlFragmentRecord, UpdateFragmentBoundary};

/// Test: Parse s0304t.tik.xml, insert fragment after 17.0, move content, validate reconstruction
#[test]
fn test_insert_fragment_and_validate_reconstruction() {
    // Load the test XML file
    let test_file = PathBuf::from("tests/data/s0304t.tik.xml");
    if !test_file.exists() {
        eprintln!("Skipping test: test file not found at {:?}", test_file);
        return;
    }

    let temp_db = NamedTempFile::new().unwrap();
    let db_path = temp_db.path();

    // Read XML content
    let xml_content = std::fs::read_to_string(&test_file).unwrap();
    let original_length = xml_content.len();
    let structure = detect_nikaya_structure(&xml_content).unwrap();
    let cst_file = "s0304t.tik.xml";

    // Step 1: Parse into fragments
    let fragments = parse_into_fragments(
        &xml_content,
        &structure,
        cst_file,
        &ParserOverrides::default(),
        false,
    )
    .unwrap();

    println!("Parsed {} fragments", fragments.len());

    // Step 2: Export to database
    export_fragments_to_db(&fragments, &structure, db_path).unwrap();

    // Connect to DB
    let mut conn = SqliteConnection::establish(db_path.to_str().unwrap()).unwrap();

    // Step 3: Verify fragment 17.0 exists and inspect its content
    let frag_17_0: XmlFragmentRecord = xml_fragments::table
        .filter(xml_fragments::cst_file.eq(cst_file))
        .filter(xml_fragments::frag_idx_code.eq("17.0"))
        .first(&mut conn)
        .expect("Fragment 17.0 should exist");

    println!("Fragment 17.0:");
    println!("  start: line {}, char {}", frag_17_0.start_line, frag_17_0.start_char);
    println!("  end: line {}, char {}", frag_17_0.end_line, frag_17_0.end_char);
    println!("  content length: {} bytes", frag_17_0.content_xml.len());

    // Find the position of "<p rend=\"title\">6. Avijjāvaggavaṇṇanā</p>" in fragment 17.0
    let split_marker = "<p rend=\"title\">6. Avijjāvaggavaṇṇanā</p>";
    let split_pos_in_fragment = frag_17_0.content_xml.find(split_marker);

    assert!(
        split_pos_in_fragment.is_some(),
        "Fragment 17.0 should contain the Avijjāvaggavaṇṇanā title. Content starts with: {}",
        &frag_17_0.content_xml[..frag_17_0.content_xml.len().min(200)]
    );

    let split_pos = split_pos_in_fragment.unwrap();
    println!("  Split marker found at position {} in fragment content", split_pos);

    // Calculate the absolute position in the XML file
    // We need to find the line/char where the split should happen
    let content_before_split = &frag_17_0.content_xml[..split_pos];
    let lines_before_split: Vec<&str> = content_before_split.split('\n').collect();
    let split_line_offset = lines_before_split.len() - 1; // 0-indexed offset from fragment start
    let split_char: i32 = if split_line_offset == 0 {
        frag_17_0.start_char + split_pos as i32
    } else {
        // Last line's length (after the last newline)
        lines_before_split.last().unwrap().len() as i32
    };
    let split_line: i32 = frag_17_0.start_line + split_line_offset as i32;

    println!("  Split point: line {}, char {}", split_line, split_char);

    // Step 4: Insert new fragment after 17.0
    let new_fragment = insert_fragment(&mut conn, cst_file, "17.0", Direction::Next)
        .expect("Should insert fragment successfully");

    println!("\nInserted new fragment:");
    println!("  frag_idx_code: {}", new_fragment.frag_idx_code);
    println!("  start: line {}, char {}", new_fragment.start_line, new_fragment.start_char);
    println!("  end: line {}, char {}", new_fragment.end_line, new_fragment.end_char);

    assert_eq!(new_fragment.frag_idx_code, "17.1", "New fragment should be 17.1");

    // Step 5: Adjust boundaries to move content from 17.0 to 17.1
    // 17.0's end should be at the split point
    // 17.1's start should be at the split point, end should be at 17.0's original end

    // Calculate new content for both fragments
    let new_content_17_0 = frag_17_0.content_xml[..split_pos].to_string();
    let new_content_17_1 = frag_17_0.content_xml[split_pos..].to_string();

    // Update 17.0: truncate at split point
    let update_17_0 = UpdateFragmentBoundary {
        start_line: frag_17_0.start_line,
        start_char: frag_17_0.start_char,
        end_line: split_line,
        end_char: split_char,
        content_xml: new_content_17_0.clone(),
    };
    diesel::update(xml_fragments::table)
        .filter(xml_fragments::cst_file.eq(cst_file))
        .filter(xml_fragments::frag_idx_code.eq("17.0"))
        .set(&update_17_0)
        .execute(&mut conn)
        .expect("Should update 17.0 boundary");

    // Update 17.1: start at split point, end at 17.0's original end
    let update_17_1 = UpdateFragmentBoundary {
        start_line: split_line,
        start_char: split_char,
        end_line: frag_17_0.end_line,
        end_char: frag_17_0.end_char,
        content_xml: new_content_17_1.clone(),
    };
    diesel::update(xml_fragments::table)
        .filter(xml_fragments::cst_file.eq(cst_file))
        .filter(xml_fragments::frag_idx_code.eq("17.1"))
        .set(&update_17_1)
        .execute(&mut conn)
        .expect("Should update 17.1 boundaries");

    // Mark 17.1 as checked (to simulate user action)
    diesel::update(xml_fragments::table)
        .filter(xml_fragments::cst_file.eq(cst_file))
        .filter(xml_fragments::frag_idx_code.eq("17.1"))
        .set(xml_fragments::frag_review.eq(Some("checked")))
        .execute(&mut conn)
        .expect("Should mark 17.1 as checked");

    println!("\nAfter boundary adjustment:");

    // Verify the fragments
    let updated_17_0: XmlFragmentRecord = xml_fragments::table
        .filter(xml_fragments::cst_file.eq(cst_file))
        .filter(xml_fragments::frag_idx_code.eq("17.0"))
        .first(&mut conn)
        .unwrap();
    println!("Fragment 17.0:");
    println!("  end: line {}, char {}", updated_17_0.end_line, updated_17_0.end_char);
    println!("  content length: {} bytes", updated_17_0.content_xml.len());

    let updated_17_1: XmlFragmentRecord = xml_fragments::table
        .filter(xml_fragments::cst_file.eq(cst_file))
        .filter(xml_fragments::frag_idx_code.eq("17.1"))
        .first(&mut conn)
        .unwrap();
    println!("Fragment 17.1:");
    println!("  start: line {}, char {}", updated_17_1.start_line, updated_17_1.start_char);
    println!("  end: line {}, char {}", updated_17_1.end_line, updated_17_1.end_char);
    println!("  content length: {} bytes", updated_17_1.content_xml.len());
    println!("  frag_review: {:?}", updated_17_1.frag_review);

    // Step 6: Validate reconstruction
    let reconstructed = reconstruct_xml_with_conn(&mut conn, cst_file)
        .expect("Should reconstruct XML");

    println!("\nReconstruction validation:");
    println!("  Original length: {} bytes", original_length);
    println!("  Reconstructed length: {} bytes", reconstructed.len());

    assert_eq!(
        reconstructed.len(),
        original_length,
        "Reconstructed XML should have same length as original"
    );

    assert_eq!(
        reconstructed,
        xml_content,
        "Reconstructed XML should match original exactly"
    );

    println!("SUCCESS: Reconstruction matches original!");
}

/// Test: Inserted fragment survives reparse with correct boundaries
#[test]
fn test_inserted_fragment_survives_reparse() {
    // Load the test XML file
    let test_file = PathBuf::from("tests/data/s0304t.tik.xml");
    if !test_file.exists() {
        eprintln!("Skipping test: test file not found");
        return;
    }

    let temp_db = NamedTempFile::new().unwrap();
    let db_path = temp_db.path();

    // Read XML content
    let xml_content = std::fs::read_to_string(&test_file).unwrap();
    let structure = detect_nikaya_structure(&xml_content).unwrap();
    let cst_file = "s0304t.tik.xml";

    // Parse and export
    let fragments = parse_into_fragments(
        &xml_content,
        &structure,
        cst_file,
        &ParserOverrides::default(),
        false,
    )
    .unwrap();
    export_fragments_to_db(&fragments, &structure, db_path).unwrap();

    let mut conn = SqliteConnection::establish(db_path.to_str().unwrap()).unwrap();

    // Get fragment 17.0
    let frag_17_0: XmlFragmentRecord = xml_fragments::table
        .filter(xml_fragments::cst_file.eq(cst_file))
        .filter(xml_fragments::frag_idx_code.eq("17.0"))
        .first(&mut conn)
        .unwrap();

    // Find split position
    let split_marker = "<p rend=\"title\">6. Avijjāvaggavaṇṇanā</p>";
    let split_pos = frag_17_0.content_xml.find(split_marker).unwrap();
    let content_before_split = &frag_17_0.content_xml[..split_pos];
    let lines_before_split: Vec<&str> = content_before_split.split('\n').collect();
    let split_line_offset = lines_before_split.len() - 1;
    let split_char: i32 = if split_line_offset == 0 {
        frag_17_0.start_char + split_pos as i32
    } else {
        lines_before_split.last().unwrap().len() as i32
    };
    let split_line: i32 = frag_17_0.start_line + split_line_offset as i32;

    // Calculate new content
    let new_content_17_0 = frag_17_0.content_xml[..split_pos].to_string();
    let new_content_17_1 = frag_17_0.content_xml[split_pos..].to_string();

    // Insert fragment
    let new_fragment = insert_fragment(&mut conn, cst_file, "17.0", Direction::Next).unwrap();
    assert_eq!(new_fragment.frag_idx_code, "17.1");

    // Adjust boundaries for 17.0
    let update_17_0 = UpdateFragmentBoundary {
        start_line: frag_17_0.start_line,
        start_char: frag_17_0.start_char,
        end_line: split_line,
        end_char: split_char,
        content_xml: new_content_17_0,
    };
    diesel::update(xml_fragments::table)
        .filter(xml_fragments::cst_file.eq(cst_file))
        .filter(xml_fragments::frag_idx_code.eq("17.0"))
        .set(&update_17_0)
        .execute(&mut conn)
        .unwrap();

    // Adjust boundaries for 17.1
    let update_17_1 = UpdateFragmentBoundary {
        start_line: split_line,
        start_char: split_char,
        end_line: frag_17_0.end_line,
        end_char: frag_17_0.end_char,
        content_xml: new_content_17_1.clone(),
    };
    diesel::update(xml_fragments::table)
        .filter(xml_fragments::cst_file.eq(cst_file))
        .filter(xml_fragments::frag_idx_code.eq("17.1"))
        .set(&update_17_1)
        .execute(&mut conn)
        .unwrap();

    // Mark 17.1 as checked
    diesel::update(xml_fragments::table)
        .filter(xml_fragments::cst_file.eq(cst_file))
        .filter(xml_fragments::frag_idx_code.eq("17.1"))
        .set(xml_fragments::frag_review.eq(Some("checked")))
        .execute(&mut conn)
        .unwrap();

    // Verify state before reparse
    let inserted_before: XmlFragmentRecord = xml_fragments::table
        .filter(xml_fragments::cst_file.eq(cst_file))
        .filter(xml_fragments::frag_idx_code.eq("17.1"))
        .first(&mut conn)
        .unwrap();

    let content_17_1_before = inserted_before.content_xml.clone();
    println!("Content 17.1 before reparse: {} bytes", content_17_1_before.len());
    println!("  starts with: {}", &content_17_1_before[..content_17_1_before.len().min(80)]);

    // Now extract correction overrides and reparse
    let (correction_overrides, _review_status, inserted_fragments_vec) =
        extract_correction_overrides(db_path, cst_file).unwrap();

    println!("\nExtracted {} correction overrides", correction_overrides.len());
    println!("Extracted {} inserted fragments", inserted_fragments_vec.len());

    // Verify inserted fragment was extracted
    assert_eq!(
        inserted_fragments_vec.len(),
        1,
        "Should have extracted 1 inserted fragment"
    );

    let ins_frag = &inserted_fragments_vec[0];
    println!("\nInserted fragment details:");
    println!("  frag_idx_code: {}", ins_frag.frag_idx_code);
    println!("  start_line: {}, start_char: {}", ins_frag.start_line, ins_frag.start_char);
    println!("  end_line: {}, end_char: {}", ins_frag.end_line, ins_frag.end_char);
    println!("  content length: {} bytes", ins_frag.content_xml.len());

    assert_eq!(
        inserted_fragments_vec[0].frag_idx_code,
        "17.1",
        "Inserted fragment should be 17.1"
    );

    // Build InsertedFragmentsMap
    let mut inserted_map = std::collections::HashMap::new();
    inserted_map.insert(cst_file.to_string(), inserted_fragments_vec);

    // Reparse with overrides
    let overrides = ParserOverrides {
        correction_overrides: if correction_overrides.is_empty() {
            None
        } else {
            Some(correction_overrides)
        },
        inserted_fragments: Some(inserted_map),
        pali_titles: None,
    };

    // DEBUG: Verify byte positions
    let start_pos = 23283usize;
    let end_pos = 25796usize;
    println!("\nDEBUG: Content at byte positions:");
    println!("  xml[{}..{}] (len={}):", start_pos, end_pos, end_pos - start_pos);
    println!("  starts with: {}", &xml_content[start_pos..start_pos+100.min(xml_content.len()-start_pos)]);
    println!("  ends with: ...{}", &xml_content[(end_pos-100).max(start_pos)..end_pos]);

    // What's actually at line 323?
    let lines: Vec<&str> = xml_content.lines().collect();
    println!("\nDEBUG: Actual line 323 (0-indexed 322): {}", lines.get(322).unwrap_or(&"<out of bounds>"));
    println!("DEBUG: Actual line 323 (1-indexed via lines[322])");

    // Calculate byte position of line 323 manually
    let mut byte_pos = 0;
    for (i, line) in xml_content.lines().enumerate() {
        if i == 322 { // 0-indexed, so 322 is line 323
            println!("DEBUG: Line 323 starts at byte position: {}", byte_pos);
            println!("DEBUG: Line 323 content: {}", &xml_content[byte_pos..byte_pos+80.min(xml_content.len()-byte_pos)]);
            break;
        }
        byte_pos += line.len() + 1; // +1 for newline
    }

    let reparsed_fragments = parse_into_fragments(
        &xml_content,
        &structure,
        cst_file,
        &overrides,
        false,
    )
    .unwrap();

    println!("\nAfter reparse: {} fragments", reparsed_fragments.len());

    // Find fragment 17.1 in reparsed results
    let frag_17_1 = reparsed_fragments
        .iter()
        .find(|f| f.frag_idx_code == "17.1");

    assert!(
        frag_17_1.is_some(),
        "Fragment 17.1 should exist after reparse. Fragment codes: {:?}",
        reparsed_fragments.iter().map(|f| &f.frag_idx_code).collect::<Vec<_>>()
    );

    let frag_17_1 = frag_17_1.unwrap();
    println!("Fragment 17.1 after reparse:");
    println!("  content length: {} bytes", frag_17_1.content_xml.len());
    println!("  starts with: {}", &frag_17_1.content_xml[..frag_17_1.content_xml.len().min(80)]);

    // Verify content is preserved
    assert_eq!(
        frag_17_1.content_xml.len(),
        content_17_1_before.len(),
        "Content length should be preserved"
    );

    // Verify fragment 17.0 was truncated
    let frag_17_0_reparsed = reparsed_fragments
        .iter()
        .find(|f| f.frag_idx_code == "17.0")
        .expect("Fragment 17.0 should exist");

    println!("Fragment 17.0 after reparse:");
    println!("  start_line: {}, start_char: {}", frag_17_0_reparsed.start_line, frag_17_0_reparsed.start_char);
    println!("  end_line: {}, end_char: {}", frag_17_0_reparsed.end_line, frag_17_0_reparsed.end_char);
    println!("  content length: {} bytes", frag_17_0_reparsed.content_xml.len());
    println!("  content ends with: ...{}", &frag_17_0_reparsed.content_xml[frag_17_0_reparsed.content_xml.len().saturating_sub(100)..]);

    // 17.0 should NOT contain the Avijjāvaggavaṇṇanā title anymore
    assert!(
        !frag_17_0_reparsed.content_xml.contains(split_marker),
        "Fragment 17.0 should not contain the split content after reparse"
    );

    // Validate reconstruction
    let total_length: usize = reparsed_fragments.iter().map(|f| f.content_xml.len()).sum();
    println!("\nTotal content length: {} bytes", total_length);
    println!("Original XML length: {} bytes", xml_content.len());

    // Check fragment 18.0 to see if it overlaps
    let frag_18_0 = reparsed_fragments.iter().find(|f| f.frag_idx_code == "18.0");
    if let Some(f18) = frag_18_0 {
        println!("\nFragment 18.0:");
        println!("  start_line: {}, start_char: {}", f18.start_line, f18.start_char);
        println!("  content length: {} bytes", f18.content_xml.len());
        println!("  starts with: {}", &f18.content_xml[..f18.content_xml.len().min(80)]);
    }

    // List all fragments around 17
    println!("\nFragments 16-19:");
    for f in reparsed_fragments.iter().filter(|f| {
        let code = &f.frag_idx_code;
        code.starts_with("16.") || code.starts_with("17.") || code.starts_with("18.") || code.starts_with("19.")
    }) {
        println!("  {} (lines {}-{}, {} bytes)", f.frag_idx_code, f.start_line, f.end_line, f.content_xml.len());
    }

    assert_eq!(
        total_length,
        xml_content.len(),
        "Total fragment content should equal original XML length"
    );

    println!("SUCCESS: Inserted fragment survived reparse with correct content!");
}
