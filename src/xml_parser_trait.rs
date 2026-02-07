//! XML parser trait definition
//!
//! This module defines the common interface that all XML parser implementations
//! must follow, ensuring consistent API across different nikaya-specific parsers.

use anyhow::Result;
use crate::types::{XmlFragment, ParserOverrides};
use crate::nikaya_structure::NikayaStructure;

/// Common interface for all XML parser implementations
///
/// All parsers (general and nikaya-specific) must implement this trait
/// to ensure a consistent parsing interface.
pub trait XmlParser {
    /// Parse XML content into fragments with line tracking
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
    fn parse_into_fragments(
        &self,
        xml_content: &str,
        nikaya_structure: &NikayaStructure,
        cst_file: &str,
        overrides: &ParserOverrides,
        populate_sc_fields: bool,
    ) -> Result<Vec<XmlFragment>>;
}
