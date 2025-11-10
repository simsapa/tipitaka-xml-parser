//! XML file type classification
//!
//! This module defines the XmlFileType enum which categorizes different types
//! of Tipitaka XML files based on nikaya and text type (mula/atthakatha/tika).

use serde::{Deserialize, Serialize};

/// Type of XML file, combining nikaya and text type
///
/// This enum allows dispatching to specific parser implementations
/// for different nikaya structures and text types.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum XmlFileType {
    /// Dīgha Nikāya mūla text
    DighaNikayaMula,
    /// Dīgha Nikāya aṭṭhakathā (commentary)
    DighaNikayaAtthakatha,
    /// Dīgha Nikāya ṭīkā (sub-commentary)
    DighaNikayaTika,
    
    /// Majjhima Nikāya mūla text
    MajjhimaNikayaMula,
    /// Majjhima Nikāya aṭṭhakathā (commentary)
    MajjhimaNikayaAtthakatha,
    /// Majjhima Nikāya ṭīkā (sub-commentary)
    MajjhimaNikayaTika,
    
    /// Saṃyutta Nikāya mūla text
    SamyuttaNikayaMula,
    /// Saṃyutta Nikāya aṭṭhakathā (commentary)
    SamyuttaNikayaAtthakatha,
    /// Saṃyutta Nikāya ṭīkā (sub-commentary)
    SamyuttaNikayaTika,
    
    /// Aṅguttara Nikāya mūla text
    AnguttaraNikayaMula,
    /// Aṅguttara Nikāya aṭṭhakathā (commentary)
    AnguttaraNikayaAtthakatha,
    /// Aṅguttara Nikāya ṭīkā (sub-commentary)
    AnguttaraNikayaTika,
    
    /// Khuddaka Nikāya texts
    KhuddakaNikaya,
    
    /// General/unknown XML structure (fallback)
    General,
}

impl XmlFileType {
    /// Get the nikaya name from this file type
    pub fn nikaya_name(&self) -> &str {
        match self {
            XmlFileType::DighaNikayaMula
            | XmlFileType::DighaNikayaAtthakatha
            | XmlFileType::DighaNikayaTika => "digha",
            
            XmlFileType::MajjhimaNikayaMula
            | XmlFileType::MajjhimaNikayaAtthakatha
            | XmlFileType::MajjhimaNikayaTika => "majjhima",
            
            XmlFileType::SamyuttaNikayaMula
            | XmlFileType::SamyuttaNikayaAtthakatha
            | XmlFileType::SamyuttaNikayaTika => "samyutta",
            
            XmlFileType::AnguttaraNikayaMula
            | XmlFileType::AnguttaraNikayaAtthakatha
            | XmlFileType::AnguttaraNikayaTika => "anguttara",
            
            XmlFileType::KhuddakaNikaya => "khuddaka",
            
            XmlFileType::General => "unknown",
        }
    }
    
    /// Check if this is a mūla (root) text
    pub fn is_mula(&self) -> bool {
        matches!(
            self,
            XmlFileType::DighaNikayaMula
                | XmlFileType::MajjhimaNikayaMula
                | XmlFileType::SamyuttaNikayaMula
                | XmlFileType::AnguttaraNikayaMula
        )
    }
    
    /// Check if this is an aṭṭhakathā (commentary)
    pub fn is_atthakatha(&self) -> bool {
        matches!(
            self,
            XmlFileType::DighaNikayaAtthakatha
                | XmlFileType::MajjhimaNikayaAtthakatha
                | XmlFileType::SamyuttaNikayaAtthakatha
                | XmlFileType::AnguttaraNikayaAtthakatha
        )
    }
    
    /// Check if this is a ṭīkā (sub-commentary)
    pub fn is_tika(&self) -> bool {
        matches!(
            self,
            XmlFileType::DighaNikayaTika
                | XmlFileType::MajjhimaNikayaTika
                | XmlFileType::SamyuttaNikayaTika
                | XmlFileType::AnguttaraNikayaTika
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_nikaya_name() {
        assert_eq!(XmlFileType::DighaNikayaMula.nikaya_name(), "digha");
        assert_eq!(XmlFileType::MajjhimaNikayaAtthakatha.nikaya_name(), "majjhima");
        assert_eq!(XmlFileType::SamyuttaNikayaTika.nikaya_name(), "samyutta");
        assert_eq!(XmlFileType::AnguttaraNikayaMula.nikaya_name(), "anguttara");
        assert_eq!(XmlFileType::General.nikaya_name(), "unknown");
    }

    #[test]
    fn test_is_mula() {
        assert!(XmlFileType::DighaNikayaMula.is_mula());
        assert!(!XmlFileType::DighaNikayaAtthakatha.is_mula());
        assert!(!XmlFileType::DighaNikayaTika.is_mula());
        assert!(!XmlFileType::General.is_mula());
    }

    #[test]
    fn test_is_atthakatha() {
        assert!(!XmlFileType::MajjhimaNikayaMula.is_atthakatha());
        assert!(XmlFileType::MajjhimaNikayaAtthakatha.is_atthakatha());
        assert!(!XmlFileType::MajjhimaNikayaTika.is_atthakatha());
    }

    #[test]
    fn test_is_tika() {
        assert!(!XmlFileType::SamyuttaNikayaMula.is_tika());
        assert!(!XmlFileType::SamyuttaNikayaAtthakatha.is_tika());
        assert!(XmlFileType::SamyuttaNikayaTika.is_tika());
    }
}
