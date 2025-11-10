//! XML file type detection
//!
//! This module detects the type of Tipitaka XML file by analyzing both
//! the filename and the XML content to determine nikaya and text type.

use anyhow::Result;
use crate::xml_file_type::XmlFileType;
use crate::nikaya_structure::NikayaStructure;

/// Detect the XML file type from filename and content
///
/// Uses both filename patterns and XML content analysis to determine
/// the specific type of XML file.
///
/// # Arguments
/// * `xml_content` - The complete XML file content
/// * `cst_file` - Name of the XML file
///
/// # Returns
/// The detected XmlFileType
///
/// # File naming patterns
/// - `.mul.xml` - mūla text
/// - `.att.xml` - aṭṭhakathā (commentary)
/// - `.tik.xml` - ṭīkā (sub-commentary)
pub fn detect_xml_file_type(xml_content: &str, cst_file: &str) -> Result<XmlFileType> {
    // First, detect the nikaya from content
    let nikaya_name = detect_nikaya_from_content(xml_content)?;
    
    // Then, detect the text type from filename
    let text_type = detect_text_type_from_filename(cst_file);
    
    // Combine to get the specific XmlFileType
    let file_type = match (nikaya_name.as_str(), text_type) {
        ("digha", TextType::Mula) => XmlFileType::DighaNikayaMula,
        ("digha", TextType::Atthakatha) => XmlFileType::DighaNikayaAtthakatha,
        ("digha", TextType::Tika) => XmlFileType::DighaNikayaTika,
        
        ("majjhima", TextType::Mula) => XmlFileType::MajjhimaNikayaMula,
        ("majjhima", TextType::Atthakatha) => XmlFileType::MajjhimaNikayaAtthakatha,
        ("majjhima", TextType::Tika) => XmlFileType::MajjhimaNikayaTika,
        
        ("samyutta", TextType::Mula) => XmlFileType::SamyuttaNikayaMula,
        ("samyutta", TextType::Atthakatha) => XmlFileType::SamyuttaNikayaAtthakatha,
        ("samyutta", TextType::Tika) => XmlFileType::SamyuttaNikayaTika,
        
        ("anguttara", TextType::Mula) => XmlFileType::AnguttaraNikayaMula,
        ("anguttara", TextType::Atthakatha) => XmlFileType::AnguttaraNikayaAtthakatha,
        ("anguttara", TextType::Tika) => XmlFileType::AnguttaraNikayaTika,
        
        ("khuddaka", _) => XmlFileType::KhuddakaNikaya,
        
        _ => XmlFileType::General,
    };
    
    Ok(file_type)
}

/// Text type classification
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TextType {
    /// Mūla (root) text
    Mula,
    /// Aṭṭhakathā (commentary)
    Atthakatha,
    /// Ṭīkā (sub-commentary)
    Tika,
}

/// Detect nikaya from XML content
///
/// Searches for the `<p rend="nikaya">` tag and normalizes the name
fn detect_nikaya_from_content(xml_content: &str) -> Result<String> {
    // Search for the nikaya marker: <p rend="nikaya">NikayaName</p>
    let nikaya_tag_start = "<p rend=\"nikaya\">";
    let nikaya_tag_end = "</p>";
    
    // Find the start of the nikaya tag
    let start_idx = xml_content
        .find(nikaya_tag_start)
        .ok_or_else(|| anyhow::anyhow!("Nikaya tag not found in XML content"))?;
    
    // Extract content after the opening tag
    let content_start = start_idx + nikaya_tag_start.len();
    let remaining = &xml_content[content_start..];
    
    // Find the closing tag
    let end_idx = remaining
        .find(nikaya_tag_end)
        .ok_or_else(|| anyhow::anyhow!("Nikaya tag closing not found in XML content"))?;
    
    // Extract the nikaya name
    let nikaya_name = remaining[..end_idx].trim();
    
    // Normalize the nikaya name
    let normalized_name = NikayaStructure::normalize_name(nikaya_name)
        .ok_or_else(|| anyhow::anyhow!("Unknown nikaya name: '{}'", nikaya_name))?;
    
    Ok(normalized_name)
}

/// Detect text type from filename
///
/// # File naming patterns
/// - `.mul.xml` - mūla text
/// - `.att.xml` - aṭṭhakathā (commentary)
/// - `.tik.xml` - ṭīkā (sub-commentary)
fn detect_text_type_from_filename(filename: &str) -> TextType {
    let filename_lower = filename.to_lowercase();
    
    if filename_lower.ends_with(".mul.xml") {
        TextType::Mula
    } else if filename_lower.ends_with(".att.xml") {
        TextType::Atthakatha
    } else if filename_lower.ends_with(".tik.xml")  {
        TextType::Tika
    } else {
        // Default to Mula if unclear
        TextType::Mula
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_sample_xml(nikaya: &str) -> String {
        format!(r#"<?xml version="1.0" encoding="UTF-8"?>
<TEI.2>
<teiHeader></teiHeader>
<text>
<body>
<p rend="nikaya">{}</p>
</body>
</text>
</TEI.2>"#, nikaya)
    }

    #[test]
    fn test_detect_digha_mula() {
        let xml = create_sample_xml("Dīghanikāyo");
        let file_type = detect_xml_file_type(&xml, "s0101m.mul.xml").unwrap();
        assert_eq!(file_type, XmlFileType::DighaNikayaMula);
    }

    #[test]
    fn test_detect_digha_atthakatha() {
        let xml = create_sample_xml("Dīghanikāyo");
        let file_type = detect_xml_file_type(&xml, "s0101a.att.xml").unwrap();
        assert_eq!(file_type, XmlFileType::DighaNikayaAtthakatha);
    }

    #[test]
    fn test_detect_digha_tika() {
        let xml = create_sample_xml("Dīghanikāyo");
        let file_type = detect_xml_file_type(&xml, "s0101t.tik.xml").unwrap();
        assert_eq!(file_type, XmlFileType::DighaNikayaTika);
    }

    #[test]
    fn test_detect_majjhima_mula() {
        let xml = create_sample_xml("Majjhimanikāyo");
        let file_type = detect_xml_file_type(&xml, "s0201m.mul.xml").unwrap();
        assert_eq!(file_type, XmlFileType::MajjhimaNikayaMula);
    }

    #[test]
    fn test_detect_samyutta_mula() {
        let xml = create_sample_xml("Saṃyuttanikāyo");
        let file_type = detect_xml_file_type(&xml, "s0301m.mul.xml").unwrap();
        assert_eq!(file_type, XmlFileType::SamyuttaNikayaMula);
    }

    #[test]
    fn test_detect_anguttara_mula() {
        let xml = create_sample_xml("Aṅguttaranikāyo");
        let file_type = detect_xml_file_type(&xml, "s0402m2.mul.xml").unwrap();
        assert_eq!(file_type, XmlFileType::AnguttaraNikayaMula);
    }

    #[test]
    fn test_detect_text_type_from_filename_mula() {
        assert_eq!(detect_text_type_from_filename("s0101m.mul.xml"), TextType::Mula);
        assert_eq!(detect_text_type_from_filename("s0402m2.mul.xml"), TextType::Mula);
    }

    #[test]
    fn test_detect_text_type_from_filename_atthakatha() {
        assert_eq!(detect_text_type_from_filename("s0101a.att.xml"), TextType::Atthakatha);
    }

    #[test]
    fn test_detect_text_type_from_filename_tika() {
        assert_eq!(detect_text_type_from_filename("s0101t.tik.xml"), TextType::Tika);
    }
}
