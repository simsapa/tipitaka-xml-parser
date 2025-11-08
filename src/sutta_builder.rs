//! Sutta record assembly from fragments
//!
//! This module provides functionality to assemble database records
//! from parsed XML fragments.

use std::collections::HashMap;
use anyhow::{Result, Context};
use serde::Deserialize;
use quick_xml::{Reader, events::Event};
use html_escape;
use regex::Regex;

use crate::logger;

// NOTE: Should remain private to limit relying on the data. Provide public
// functions to access data derived from tsv.
static CST_VS_SC_TSV: &str = include_str!("../assets/cst-vs-sc.tsv");

/// FIXME: dummy function to be removed
fn consistent_niggahita(text: Option<String>) -> String {
    match text {
        Some(text) => {
            text
        }
        None => String::from("")
    }
}

/// TSV record for CST code lookup. The attribute names are prefixed with 'tsv_'
/// to help identify where the tsv data is used and only rely on it for sc_code
/// lookup and verifying parsed values. All values (other than sc_code) should
/// be parsed from the xml file and not rely on the tsv data.
#[derive(Debug, Clone, Deserialize)]
pub struct TsvRecord {
    #[serde(rename = "cst_file")]
    pub tsv_cst_file: String,
    #[serde(rename = "cst_code")]
    pub tsv_cst_code: String,
    #[serde(rename = "cst_vagga")]
    pub tsv_cst_vagga: String,
    #[serde(rename = "cst_sutta")]
    pub tsv_cst_sutta: String,
    #[serde(rename = "cst_paranum")]
    pub tsv_cst_paranum: String,
    #[serde(rename = "code")]
    pub tsv_sc_code: String,
    #[serde(rename = "sutta")]
    pub tsv_sc_sutta: String,
}

/// Load TSV mapping from embedded string
pub fn load_tsv_mapping() -> Result<Vec<TsvRecord>> {
    let mut reader = csv::ReaderBuilder::new()
        .delimiter(b'\t')
        .from_reader(CST_VS_SC_TSV.as_bytes());
    
    let records: Result<Vec<TsvRecord>, csv::Error> = reader
        .deserialize()
        .collect();
    
    records.context("Failed to deserialize TSV records")
}

/// Find code for a given filename, sutta title, and vagga
pub fn find_code_for_sutta(
    tsv_records: &[TsvRecord],
    cst_file: &str,
    sutta_title: &str,
    vagga_title: Option<&str>,
    is_commentary: bool,
    used_codes: &std::collections::HashSet<String>,
) -> Option<String> {
    // Normalize the xml filename (remove path prefix if present)
    let normalized_filename = cst_file
        .trim_start_matches("romn/")
        .trim_start_matches("mula/");
    
    // For commentaries (.att.xml or .tik.xml), convert the filename to the base .mul.xml format
    // e.g., "s0201a.att.xml" -> "s0201m.mul.xml"
    // e.g., "s0201t.tik.xml" -> "s0201m.mul.xml"
    let base_filename = if is_commentary {
        if normalized_filename.ends_with(".att.xml") {
            // Replace "a.att.xml" with "m.mul.xml"
            normalized_filename.replace("a.att.xml", "m.mul.xml")
        } else if normalized_filename.ends_with(".tik.xml") {
            // Replace "t.tik.xml" with "m.mul.xml"
            normalized_filename.replace("t.tik.xml", "m.mul.xml")
        } else {
            normalized_filename.to_string()
        }
    } else {
        normalized_filename.to_string()
    };
    
    // For commentary titles, strip the "-vaṇṇanā" suffix and try to match with the base sutta
    let search_title = if is_commentary && sutta_title.ends_with("vaṇṇanā") {
        // Strip "vaṇṇanā" - the base should already end with "sutta"
        let base = sutta_title.trim_end_matches("vaṇṇanā");
        // The base already ends with "sutta", so we just need to try both "sutta" and "suttaṃ"
        let mut candidates = if base.ends_with("sutta") {
            vec![
                base.to_string(),  // Keep "sutta" as-is
                format!("{}ṃ", base),  // Add anusvara to make "suttaṃ"
            ]
        } else {
            // Fallback: assume we need to add "sutta"
            vec![
                format!("{}sutta", base),
                format!("{}suttaṃ", base),
            ]
        };
        
        // Edge case: Handle "Vanapatthapariyāya" → "Vanapatthasutta" mismatch
        // 
        // The commentary (s0201a.att.xml, s0201t.tik.xml) uses:
        //   <p rend="subhead">7. Vanapatthapariyāyasuttavaṇṇanā</p>
        // 
        // But the base text (s0201m.mul.xml) uses:
        //   <p rend="subhead">7. Vanapatthasuttaṃ</p>
        //
        // The commentary adds "pariyāya" (method/approach) to the title, which is not
        // present in the base text or TSV mapping. This is MN 17.
        if base.contains("Vanapatthapariyāyasutta") {
            // Extract the number prefix if present (e.g., "7. ")
            if let Some((num, _)) = base.split_once('.') {
                candidates.push(format!("{}. Vanapatthasutta", num));
                candidates.push(format!("{}. Vanapatthasuttaṃ", num));
            } else {
                candidates.push("Vanapatthasutta".to_string());
                candidates.push("Vanapatthasuttaṃ".to_string());
            }
        }
        
        // Also try with "Mahā" prefix for certain suttas (e.g., Satipaṭṭhānasutta -> Mahāsatipaṭṭhānasutta)
        // Extract just the sutta name without numbering
        if let Some((num, sutta_part)) = base.split_once('.') {
            let sutta_part = sutta_part.trim();
            if sutta_part.ends_with("sutta") {
                // Add "Mahā" prefix - lowercase the first letter of the original sutta name
                let sutta_with_maha = if let Some(first_char) = sutta_part.chars().next() {
                    let rest = &sutta_part[first_char.len_utf8()..];
                    format!("Mahā{}{}", first_char.to_lowercase(), rest)
                } else {
                    format!("Mahā{}", sutta_part)
                };
                candidates.push(format!("{}. {}", num, sutta_with_maha));
                candidates.push(format!("{}. {}ṃ", num, sutta_with_maha));
            }
        }
        
        candidates
    } else {
        vec![sutta_title.to_string()]
    };
    
    // Normalize all search titles with consistent niggahita
    let normalized_search_titles: Vec<String> = search_title.iter()
        .map(|t| consistent_niggahita(Some(t.clone())))
        .collect();
    
    // Normalize vagga title if provided
    let normalized_vagga = vagga_title.map(|v| consistent_niggahita(Some(v.to_string())));
    
    let mut fallback_match: Option<String> = None;
    
    for record in tsv_records {
        let record_filename = record.tsv_cst_file
            .trim_start_matches("romn/")
            .trim_start_matches("mula/");
        
        let record_title = consistent_niggahita(Some(record.tsv_cst_sutta.clone()));
        let record_vagga = consistent_niggahita(Some(record.tsv_cst_vagga.clone()));
        
        // Check if filename matches (using base filename for commentaries)
        if record_filename == base_filename {
            
            // Check if any of the search titles match
            for search_title in &normalized_search_titles {
                if &record_title == search_title {
                    // If vagga is provided, prefer exact vagga match
                    if let Some(ref expected_vagga) = normalized_vagga {
                        let vaggas_match = &record_vagga == expected_vagga;
                        if vaggas_match {
                            // Perfect match: title + vagga - only return if code unused
                            if !used_codes.contains(&record.tsv_sc_code) {
                                return Some(record.tsv_sc_code.clone());
                            }
                        } else {
                            // Title matches but vagga doesn't
                            // Save as fallback if code not yet used
                            if fallback_match.is_none() && !used_codes.contains(&record.tsv_sc_code) {
                                fallback_match = Some(record.tsv_sc_code.clone());
                            }
                        }
                    } else {
                        // No vagga filter - return first unused match
                        if !used_codes.contains(&record.tsv_sc_code) {
                            return Some(record.tsv_sc_code.clone());
                        }
                        // If code is already used, save as fallback to continue searching
                        // (This shouldn't happen often but handles edge cases)
                        if fallback_match.is_none() {
                            fallback_match = Some(record.tsv_sc_code.clone());
                        }
                    }
                }
            }
        }
    }
    
    // If no exact match found, return fallback (for commentaries with misaligned vaggas)
    fallback_match
}

/// Returns a map to find (sc_code, sc_sutta) for a given cst_code.
pub fn cst_code_to_sc_code_map() -> Result<HashMap<String, (String, String)>> {
    let tsv_records = load_tsv_mapping()
        .context("Failed to load TSV mapping")?;

    let mut tsv_map: HashMap<String, (String, String)> = HashMap::new();

    for record in tsv_records {
        if !record.tsv_cst_code.is_empty() && !record.tsv_sc_code.is_empty() {
            tsv_map.insert(
                record.tsv_cst_code.clone(),
                (record.tsv_sc_code.clone(), record.tsv_sc_sutta.clone())
            );
        }
    }

    Ok(tsv_map)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_load_tsv_mapping() {
        // Test that the embedded TSV can be loaded and deserialized correctly
        let result = load_tsv_mapping();
        assert!(result.is_ok(), "Failed to load TSV mapping: {:?}", result.err());
        
        let records = result.unwrap();
        assert!(!records.is_empty(), "TSV mapping should contain records");
        
        // Verify first record has expected fields populated
        let first = &records[0];
        assert!(!first.cst_file.is_empty(), "cst_file should not be empty");
        assert!(!first.cst_code.is_empty(), "cst_code should not be empty");
        assert!(!first.sc_code.is_empty(), "sc_code should not be empty");
        
        // Verify known record (DN1) exists with correct mapping
        let dn1 = records.iter().find(|r| r.sc_code == "dn1");
        assert!(dn1.is_some(), "Should find DN 1 record");
        
        if let Some(dn1) = dn1 {
            assert_eq!(dn1.cst_file.trim_start_matches("romn/").trim_start_matches("mula/"), "s0101m.mul.xml");
            assert_eq!(dn1.cst_sutta, "1. Brahmajālasuttaṃ");
            assert_eq!(dn1.cst_vagga, "");
        }
    }
}

/// Transform raw XML fragment content to HTML
pub fn xml_to_html(xml_content: &str) -> Result<String> {
    let mut html = String::new();
    let mut reader = Reader::from_str(xml_content);
    reader.trim_text(false); // Preserve whitespace
    reader.check_end_names(false); // Don't validate end tag names strictly

    let mut buf = Vec::new();
    let mut pending_paranum: Option<String> = None;
    let mut unknown_tags = std::collections::HashSet::new();
    let mut tag_stack: Vec<String> = Vec::new(); // Track opened tags to close them properly

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let name_bytes = e.name();
                let name = std::str::from_utf8(name_bytes.as_ref())
                    .unwrap_or("");

                match name {
                    "p" => {
                        // Get rend attribute for class
                        let mut rend = String::from("bodytext");
                        let mut paranum: Option<String> = None;

                        for attr in e.attributes() {
                            if let Ok(attr) = attr {
                                let key = std::str::from_utf8(attr.key.as_ref()).unwrap_or("");
                                let value = attr.unescape_value().unwrap_or_default();

                                match key {
                                    "rend" => rend = value.to_string(),
                                    "n" => paranum = Some(value.to_string()),
                                    _ => {}
                                }
                            }
                        }

                        // Map rend types to CSS classes
                        let class = match rend.as_str() {
                            "centre" => "centered",
                            "center" => "centered",
                            _ => &rend,
                        };

                        html.push_str(&format!("<p class=\"{}\">", class));
                        pending_paranum = paranum;
                    }
                    "hi" => {
                        // Highlighted text - get rend attribute
                        let mut rend = String::from("bold");

                        for attr in e.attributes() {
                            if let Ok(attr) = attr {
                                let key = std::str::from_utf8(attr.key.as_ref()).unwrap_or("");
                                if key == "rend" {
                                    rend = attr.unescape_value().unwrap_or_default().to_string();
                                }
                            }
                        }

                        // Special handling for paranum
                        if rend == "paranum" {
                            html.push_str("<span class=\"paranum\">");
                        } else {
                            html.push_str(&format!("<span class=\"{}\">", rend));
                        }
                    }
                    "note" => {
                        html.push_str("<span class=\"note\">[");
                    }
                    "pb" => {
                        // Page break
                        let mut ed = String::new();
                        let mut n = String::new();

                        for attr in e.attributes() {
                            if let Ok(attr) = attr {
                                let key = std::str::from_utf8(attr.key.as_ref()).unwrap_or("");
                                let value = attr.unescape_value().unwrap_or_default();

                                match key {
                                    "ed" => ed = value.to_string(),
                                    "n" => n = value.to_string(),
                                    _ => {}
                                }
                            }
                        }

                        html.push_str(&format!("<span class=\"pagebreak\" data-ed=\"{}\" data-n=\"{}\"></span>", ed, n));
                    }
                    "head" => {
                        // Head tags - convert to appropriate HTML heading based on rend attribute
                        let mut rend = String::new();
                        for attr in e.attributes() {
                            if let Ok(attr) = attr {
                                let key = std::str::from_utf8(attr.key.as_ref()).unwrap_or("");
                                if key == "rend" {
                                    rend = attr.unescape_value().unwrap_or_default().to_string();
                                }
                            }
                        }

                        let html_tag = match rend.as_str() {
                            "chapter" => {
                                html.push_str("<h2 class=\"chapter\">");
                                "h2"
                            },
                            "book" => {
                                html.push_str("<h2 class=\"book\">");
                                "h2"
                            },
                            "subhead" => {
                                html.push_str("<h3 class=\"subhead\">");
                                "h3"
                            },
                            "subsubhead" => {
                                html.push_str("<h4 class=\"subsubhead\">");
                                "h4"
                            },
                            _ => {
                                html.push_str(&format!("<h3 class=\"{}\">", rend));
                                "h3"
                            }
                        };
                        tag_stack.push(html_tag.to_string());
                    }
                    "div" => {
                        // Div tags - convert to HTML div with appropriate class
                        let mut div_type = String::new();
                        let mut id = String::new();

                        for attr in e.attributes() {
                            if let Ok(attr) = attr {
                                let key = std::str::from_utf8(attr.key.as_ref()).unwrap_or("");
                                let value = attr.unescape_value().unwrap_or_default();

                                match key {
                                    "type" => div_type = value.to_string(),
                                    "id" => id = value.to_string(),
                                    _ => {}
                                }
                            }
                        }

                        if !div_type.is_empty() {
                            if !id.is_empty() {
                                html.push_str(&format!("<div class=\"{}\" id=\"{}\">", div_type, id));
                            } else {
                                html.push_str(&format!("<div class=\"{}\">", div_type));
                            }
                        } else {
                            html.push_str("<div>");
                        }
                        tag_stack.push("div".to_string());
                    }
                    "trailer" => {
                        // Trailer - typically end markers like "Suttavaṇṇanā niṭṭhitā"
                        let mut rend = String::from("trailer");
                        for attr in e.attributes() {
                            if let Ok(attr) = attr {
                                let key = std::str::from_utf8(attr.key.as_ref()).unwrap_or("");
                                if key == "rend" {
                                    rend = attr.unescape_value().unwrap_or_default().to_string();
                                }
                            }
                        }
                        html.push_str(&format!("<p class=\"{}\">", rend));
                        tag_stack.push("p".to_string());
                    }
                    _ => {
                        // Log unknown tags for future handling
                        if unknown_tags.insert(name.to_string()) {
                            logger::warn(&format!("Unknown XML tag in content: <{}>", name));
                        }
                    }
                }
            }
            Ok(Event::End(ref e)) => {
                let name_bytes = e.name();
                let name = std::str::from_utf8(name_bytes.as_ref())
                    .unwrap_or("");

                match name {
                    "p" => {
                        html.push_str("</p>\n");
                        pending_paranum = None;
                    }
                    "hi" => {
                        html.push_str("</span>");
                    }
                    "note" => {
                        html.push_str("]</span>");
                    }
                    "head" | "div" | "trailer" => {
                        // Close the tag using the tag stack
                        if let Some(tag) = tag_stack.pop() {
                            html.push_str(&format!("</{}>\n", tag));
                        } else {
                            // Fallback if stack is empty (shouldn't happen with well-formed XML)
                            match name {
                                "head" => html.push_str("</h3>\n"),
                                "div" => html.push_str("</div>\n"),
                                "trailer" => html.push_str("</p>\n"),
                                _ => {}
                            }
                        }
                    }
                    _ => {
                        // Unknown closing tags - skip silently (already logged on open)
                    }
                }
            }
            Ok(Event::Text(e)) => {
                let text = e.unescape().unwrap_or_default();

                // Add paranum if pending and this is the first text in paragraph
                if let Some(num) = pending_paranum.take() {
                    html.push_str(&format!("<span class=\"paranum\">{}</span> ", num));
                }

                // HTML escape the text content
                html.push_str(&html_escape::encode_text(&text));
            }
            Ok(Event::Empty(ref e)) => {
                let name_bytes = e.name();
                let name = std::str::from_utf8(name_bytes.as_ref())
                    .unwrap_or("");

                if name == "pb" {
                    // Handle self-closing page break
                    let mut ed = String::new();
                    let mut n = String::new();

                    for attr in e.attributes() {
                        if let Ok(attr) = attr {
                            let key = std::str::from_utf8(attr.key.as_ref()).unwrap_or("");
                            let value = attr.unescape_value().unwrap_or_default();

                            match key {
                                "ed" => ed = value.to_string(),
                                "n" => n = value.to_string(),
                                _ => {}
                            }
                        }
                    }

                    html.push_str(&format!("<span class=\"pagebreak\" data-ed=\"{}\" data-n=\"{}\"></span>", ed, n));
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                // Log error but continue
                logger::error(&format!("XML parsing error at position {}: {}", reader.buffer_position(), e));
            }
            _ => {}
        }

        buf.clear();
    }

    // Normalize successive blank lines to one
    let normalized_html = normalize_blank_lines(&html);

    Ok(normalized_html)
}

/// Normalize successive blank lines to a single blank line
fn normalize_blank_lines(text: &str) -> String {
    // Replace multiple consecutive newlines (3 or more) with just 2 newlines (one blank line)
    let re = Regex::new(r"\n{3,}").unwrap();
    re.replace_all(text, "\n\n").to_string()
}
