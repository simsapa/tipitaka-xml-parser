//! Main XML parser module with type detection and dispatch
//!
//! This module provides the main entry point for parsing Tipitaka XML files.
//! It detects the XML file type and dispatches to the appropriate parser implementation.

use anyhow::Result;
use crate::types::{XmlFragment, ParserOverrides, GroupLevel, compare_frag_idx_code};
use crate::nikaya_structure::NikayaStructure;
use crate::xml_file_type::XmlFileType;
use crate::xml_type_detector::detect_xml_file_type;
use crate::xml_parser_trait::XmlParser;
use crate::parsers::{
    GeneralParser,
    SamyuttaNikayaMula,
    SamyuttaNikayaCommentary,
};

/// Parse XML content into fragments with automatic type detection
///
/// This function detects the XML file type and dispatches to the appropriate
/// parser implementation. Currently, all types use the GeneralParser.
///
/// # Arguments
/// * `xml_content` - The complete XML file content
/// * `nikaya_structure` - The structure configuration for this nikaya
/// * `cst_file` - Name of the XML file being parsed
/// * `overrides` - Parser overrides including adjustments and checked fragment overrides
/// * `populate_sc_fields` - Whether to populate SC fields from embedded TSV
///
/// # Returns
/// Vector of fragments or error if parsing fails
pub fn parse_into_fragments(
    xml_content: &str,
    nikaya_structure: &NikayaStructure,
    cst_file: &str,
    overrides: &ParserOverrides,
    populate_sc_fields: bool,
) -> Result<Vec<XmlFragment>> {
    // Detect the XML file type
    let xml_type = detect_xml_file_type(xml_content, cst_file)?;

    // Dispatch to the appropriate parser based on type
    let parser: Box<dyn XmlParser> = match xml_type {
        XmlFileType::DighaNikayaMula |
        XmlFileType::DighaNikayaAtthakatha |
        XmlFileType::DighaNikayaTika |
        XmlFileType::MajjhimaNikayaMula |
        XmlFileType::MajjhimaNikayaAtthakatha |
        XmlFileType::MajjhimaNikayaTika => {
            Box::new(GeneralParser::new())
        }

        XmlFileType::SamyuttaNikayaMula => {
            Box::new(SamyuttaNikayaMula::new())
        }
        XmlFileType::SamyuttaNikayaAtthakatha |
        XmlFileType::SamyuttaNikayaTika => {
            Box::new(SamyuttaNikayaCommentary::new())
        }

        XmlFileType::AnguttaraNikayaMula |
        XmlFileType::AnguttaraNikayaAtthakatha |
        XmlFileType::AnguttaraNikayaTika => {
            Box::new(GeneralParser::new())
        }

        XmlFileType::KhuddakaNikaya
        | XmlFileType::General => {
            Box::new(GeneralParser::new())
        }
    };

    // Parse using the selected parser
    let mut fragments = parser.parse_into_fragments(
        xml_content,
        nikaya_structure,
        cst_file,
        overrides,
        populate_sc_fields,
    )?;

    // Apply SC overrides from CorrectionFragmentOverrides (post-processing)
    // This applies sc_code and sc_sutta overrides directly to fragments
    // and propagates context to subsequent null fragments
    if let Some(ref correction_overrides) = overrides.correction_overrides
        && !correction_overrides.is_empty() {
            crate::parsers::helpers::apply_sc_overrides(
                &mut fragments,
                correction_overrides,
                cst_file,
                overrides.pali_titles.as_ref()
            );
        }

    // Inject inserted fragments (post-processing)
    // These are fragments that were manually inserted by users (sub-index > 0)
    // and need to be re-injected at their correct positions after parsing.
    if let Some(ref inserted_map) = overrides.inserted_fragments {
        if let Some(inserted_list) = inserted_map.get(cst_file) {
            inject_inserted_fragments(&mut fragments, inserted_list);
        }
    }

    Ok(fragments)
}

/// Inject inserted fragments into the fragment list at their correct positions.
///
/// Inserted fragments (those with sub-index > 0, e.g., "21.1", "21.2") are stored
/// separately from the generated fragments during extraction. After parsing produces
/// the base fragment set with codes "0.0", "1.0", etc., this function splices the
/// inserted fragments back into the list at their correct sorted positions.
///
/// # Arguments
/// * `fragments` - Mutable vector of parsed fragments to inject into
/// * `inserted_list` - Slice of inserted fragment data to inject (should be sorted by frag_idx_code)
fn inject_inserted_fragments(
    fragments: &mut Vec<XmlFragment>,
    inserted_list: &[crate::types::InsertedFragmentData],
) {
    if inserted_list.is_empty() {
        return;
    }

    // Convert InsertedFragmentData to XmlFragment
    let mut to_insert: Vec<XmlFragment> = inserted_list.iter().map(|data| {
        // Deserialize group_levels from JSON
        let group_levels: Vec<GroupLevel> = serde_json::from_str(&data.group_levels)
            .unwrap_or_default();

        XmlFragment {
            nikaya: data.nikaya.clone(),
            cst_file: String::new(), // Will be set below, this is just for construction
            frag_idx_code: data.frag_idx_code.clone(),
            frag_type: data.frag_type.clone(),
            frag_review: data.frag_review.clone(),
            content_xml: data.content_xml.clone(),
            start_line: data.start_line,
            start_char: data.start_char,
            end_line: data.end_line,
            end_char: data.end_char,
            cst_code: data.cst_code.clone(),
            cst_vagga: data.cst_vagga.clone(),
            cst_sutta: data.cst_sutta.clone(),
            cst_paranum: data.cst_paranum.clone(),
            sc_code: data.sc_code.clone(),
            sc_sutta: data.sc_sutta.clone(),
            group_levels,
        }
    }).collect();

    // Get the cst_file from the first fragment to set it on inserted fragments
    if let Some(first_frag) = fragments.first() {
        for inserted in &mut to_insert {
            inserted.cst_file = first_frag.cst_file.clone();
        }
    }

    // Merge the two sorted lists
    // Both fragments and to_insert should be sorted by frag_idx_code
    // Take ownership of fragments and create a new merged list
    let original_fragments = std::mem::take(fragments);
    let mut merged = Vec::with_capacity(original_fragments.len() + to_insert.len());
    let mut frag_iter = original_fragments.into_iter().peekable();
    let mut insert_iter = to_insert.into_iter().peekable();

    while frag_iter.peek().is_some() || insert_iter.peek().is_some() {
        match (frag_iter.peek(), insert_iter.peek()) {
            (Some(f), Some(i)) => {
                if compare_frag_idx_code(&f.frag_idx_code, &i.frag_idx_code) == std::cmp::Ordering::Less
                    || compare_frag_idx_code(&f.frag_idx_code, &i.frag_idx_code) == std::cmp::Ordering::Equal {
                    merged.push(frag_iter.next().unwrap());
                } else {
                    merged.push(insert_iter.next().unwrap());
                }
            }
            (Some(_), None) => {
                merged.push(frag_iter.next().unwrap());
            }
            (None, Some(_)) => {
                merged.push(insert_iter.next().unwrap());
            }
            (None, None) => break,
        }
    }

    // Replace the original vector with the merged result
    *fragments = merged;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::nikaya_detector::detect_nikaya_structure;

    fn create_dn_sample_xml() -> String {
        r#"<?xml version="1.0" encoding="UTF-8"?>
<TEI.2>
<teiHeader></teiHeader>
<text>
<body>
<p rend="nikaya">Dīghanikāyo</p>
<div id="dn1" type="book">
<head rend="book">Sīlakkhandhavaggapāḷi</head>
<div id="dn1_1" type="sutta">
<head rend="chapter">1. Brahmajālasutta</head>
<p rend="subhead">Paribbājakakathā</p>
<p rend="bodytext" n="1">Evaṃ me sutaṃ</p>
</div>
</div>
</body>
</text>
</TEI.2>"#.to_string()
    }

    #[test]
    fn test_parse_with_type_detection() {
        let xml = create_dn_sample_xml();
        let structure = detect_nikaya_structure(&xml).expect("Should detect structure");

        let fragments = parse_into_fragments(&xml, &structure, "s0101m.mul.xml", &ParserOverrides::default(), false)
            .expect("Should parse fragments");

        assert!(!fragments.is_empty(), "Should have fragments");
    }

    #[test]
    fn test_dispatch_to_general_parser() {
        let xml = create_dn_sample_xml();
        let structure = detect_nikaya_structure(&xml).unwrap();

        // All types should currently dispatch to general parser
        let fragments = parse_into_fragments(&xml, &structure, "s0101m.mul.xml", &ParserOverrides::default(), false).unwrap();

        // Verify we got valid fragments
        assert!(!fragments.is_empty());
        assert!(fragments.iter().any(|f| !f.content_xml.trim().is_empty()));
    }
}
