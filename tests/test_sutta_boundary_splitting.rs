//! Integration test: verify XML files are split at sutta boundaries correctly.
//!
//! Each sutta fragment should contain at most ONE sutta-start marker.
//! If a fragment contains two or more markers, the parser failed to split at that boundary.
//!
//! Sutta-start markers by nikaya/type:
//! - DN mula:    `<div type="sutta">`
//! - DN att/tik: `<head rend="chapter">`
//! - MN/SN/AN mula:    `<p rend="subhead">` with numbered title (e.g. "1. Brahmajālasuttaṃ")
//! - MN/AN att/tik:     `<p rend="subhead">` ending in "suttavaṇṇanā" (general.rs)
//! - SN att/tik:        `<p rend="subhead">` ending in "suttavaṇṇanā", "suttādivaṇṇanā",
//!                      "suttadvayavaṇṇanā", or "suttavaṇṇṇanā" (triple ṇ typo)
//!                      (samyutta_nikaya_atthakatha.rs / samyutta_nikaya_tika.rs)

use std::path::PathBuf;
use regex::Regex;

use tipitaka_xml_parser::encoding::read_xml_file;
use tipitaka_xml_parser::nikaya_detector::detect_nikaya_structure;
use tipitaka_xml_parser::parse_into_fragments;
use tipitaka_xml_parser::parsers::helpers::is_sutta_subhead;
use tipitaka_xml_parser::types::{FragmentType, ParserOverrides};

/// Determine which nikaya a file belongs to based on the CST filename convention.
/// s01xx = digha, s02xx = majjhima, s03xx = samyutta, s04xx = anguttara
fn nikaya_from_filename(filename: &str) -> &str {
    if filename.starts_with("s01") {
        "digha"
    } else if filename.starts_with("s02") {
        "majjhima"
    } else if filename.starts_with("s03") {
        "samyutta"
    } else if filename.starts_with("s04") {
        "anguttara"
    } else {
        "unknown"
    }
}

/// Determine the text type from the filename.
fn text_type_from_filename(filename: &str) -> &str {
    if filename.contains(".mul.") {
        "mula"
    } else if filename.contains(".att.") {
        "atthakatha"
    } else if filename.contains(".tik.") {
        "tika"
    } else {
        "unknown"
    }
}

/// Count sutta-start markers in a fragment's XML content.
///
/// Returns (count, Vec<matched_strings>) for diagnostic output.
fn count_sutta_markers(content: &str, nikaya: &str, text_type: &str, cst_file: &str) -> (usize, Vec<String>) {
    let mut matches = Vec::new();

    match (nikaya, text_type) {
        ("digha", "mula") => {
            // DN mula: <div ... type="sutta">
            let re = Regex::new(r#"<div\b[^>]*\btype="sutta"[^>]*>"#).unwrap();
            for m in re.find_iter(content) {
                matches.push(m.as_str().to_string());
            }
        }
        ("digha", _) => {
            // DN att/tik: <head rend="chapter">...</head>
            let re = Regex::new(r#"<head\b[^>]*\brend="chapter"[^>]*>"#).unwrap();
            for m in re.find_iter(content) {
                matches.push(m.as_str().to_string());
            }
        }
        _ => {
            // MN/SN/AN: <p rend="subhead"> checked via is_sutta_subhead()
            // (numbered subhead for mula, sutta commentary suffix for att/tik)
            let re = Regex::new(r#"<p rend="subhead">([^<]*)</p>"#).unwrap();
            for cap in re.captures_iter(content) {
                let title = cap.get(1).map_or("", |m| m.as_str()).trim();
                if is_sutta_subhead(title, nikaya, cst_file) {
                    matches.push(format!("<p rend=\"subhead\">{}</p>", title));
                }
            }
        }
    }

    let count = matches.len();
    (count, matches)
}

#[test]
fn test_each_sutta_fragment_has_at_most_one_sutta_marker() {
    let data_dir = PathBuf::from("tests/data");

    // Collect all XML files
    let mut xml_files: Vec<_> = std::fs::read_dir(&data_dir)
        .expect("Failed to read tests/data directory")
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("xml") {
                Some(path)
            } else {
                None
            }
        })
        .collect();

    xml_files.sort();

    assert!(!xml_files.is_empty(), "No XML files found in tests/data/");

    let mut all_failures: Vec<String> = Vec::new();

    for xml_path in &xml_files {
        let filename = xml_path.file_name().unwrap().to_str().unwrap();
        let nikaya = nikaya_from_filename(filename);
        let text_type = text_type_from_filename(filename);

        if nikaya == "unknown" || text_type == "unknown" {
            continue;
        }

        let xml_content = read_xml_file(xml_path)
            .unwrap_or_else(|e| panic!("Failed to read {}: {}", filename, e));

        let structure = detect_nikaya_structure(&xml_content)
            .unwrap_or_else(|e| panic!("Failed to detect nikaya structure for {}: {}", filename, e));

        let fragments = parse_into_fragments(
            &xml_content,
            &structure,
            filename,
            &ParserOverrides::default(),
            false,
        )
        .unwrap_or_else(|e| panic!("Failed to parse fragments for {}: {}", filename, e));

        let sutta_fragments: Vec<_> = fragments
            .iter()
            .filter(|f| matches!(f.frag_type, FragmentType::Sutta))
            .collect();

        for frag in &sutta_fragments {
            let (count, matched) = count_sutta_markers(&frag.content_xml, nikaya, text_type, filename);
            if count > 1 {
                let msg = format!(
                    "FAIL: {} frag_idx={} has {} sutta markers (lines {}-{}):\n{}",
                    filename,
                    frag.frag_idx,
                    count,
                    frag.start_line,
                    frag.end_line,
                    matched
                        .iter()
                        .enumerate()
                        .map(|(i, m)| format!("  {}. {}", i + 1, m))
                        .collect::<Vec<_>>()
                        .join("\n"),
                );
                all_failures.push(msg);
            }
        }
    }

    if !all_failures.is_empty() {
        let summary = format!(
            "\n{} fragment(s) contain multiple sutta markers:\n\n{}",
            all_failures.len(),
            all_failures.join("\n\n"),
        );
        panic!("{}", summary);
    }
}
