//! Main XML parser module with type detection and dispatch
//!
//! This module provides the main entry point for parsing Tipitaka XML files.
//! It detects the XML file type and dispatches to the appropriate parser implementation.

use anyhow::Result;
use crate::types::{XmlFragment, FragmentAdjustments};
use crate::nikaya_structure::NikayaStructure;
use crate::xml_file_type::XmlFileType;
use crate::xml_type_detector::detect_xml_file_type;
use crate::xml_parser_trait::XmlParser;
use crate::parsers::{
    DighaNikayaMulaParser,
    GeneralParser,
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
/// * `adjustments` - Optional fragment adjustments to apply
/// * `populate_sc_fields` - Whether to populate SC fields from embedded TSV
///
/// # Returns
/// Vector of fragments or error if parsing fails
pub fn parse_into_fragments(
    xml_content: &str,
    nikaya_structure: &NikayaStructure,
    cst_file: &str,
    adjustments: Option<&FragmentAdjustments>,
    populate_sc_fields: bool,
) -> Result<Vec<XmlFragment>> {
    // Detect the XML file type
    let xml_type = detect_xml_file_type(xml_content, cst_file)?;
    
    // Dispatch to the appropriate parser based on type
    // For now, all types use the general parser
    let parser: Box<dyn XmlParser> = match xml_type {
        XmlFileType::DighaNikayaMula => {
            Box::new(DighaNikayaMulaParser::new())
        }

        XmlFileType::DighaNikayaAtthakatha
        | XmlFileType::DighaNikayaTika
        | XmlFileType::MajjhimaNikayaMula 
        | XmlFileType::MajjhimaNikayaAtthakatha 
        | XmlFileType::MajjhimaNikayaTika
        | XmlFileType::SamyuttaNikayaMula 
        | XmlFileType::SamyuttaNikayaAtthakatha 
        | XmlFileType::SamyuttaNikayaTika
        | XmlFileType::AnguttaraNikayaMula 
        | XmlFileType::AnguttaraNikayaAtthakatha 
        | XmlFileType::AnguttaraNikayaTika
        | XmlFileType::KhuddakaNikaya
        | XmlFileType::General => {
            Box::new(GeneralParser::new())
        }
    };
    
    // Parse using the selected parser
    parser.parse_into_fragments(
        xml_content,
        nikaya_structure,
        cst_file,
        adjustments,
        populate_sc_fields,
    )
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
        
        let fragments = parse_into_fragments(&xml, &structure, "s0101m.mul.xml", None, false)
            .expect("Should parse fragments");
        
        assert!(!fragments.is_empty(), "Should have fragments");
    }

    #[test]
    fn test_dispatch_to_general_parser() {
        let xml = create_dn_sample_xml();
        let structure = detect_nikaya_structure(&xml).unwrap();
        
        // All types should currently dispatch to general parser
        let fragments = parse_into_fragments(&xml, &structure, "s0101m.mul.xml", None, false).unwrap();
        
        // Verify we got valid fragments
        assert!(!fragments.is_empty());
        assert!(fragments.iter().any(|f| !f.content_xml.trim().is_empty()));
    }
}
