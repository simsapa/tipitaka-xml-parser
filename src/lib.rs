//! Tipitaka XML Parser
//!
//! This module provides functionality for parsing Tipitaka XML files into structured
//! fragments that can be assembled into sutta database records.
//!
//! The parser uses a fragment-based architecture to handle different nikaya structures
//! and preserve line tracking information.

pub mod logger;
pub mod types;
pub mod nikaya_detector;
pub mod nikaya_structure;
pub mod fragment_exporter;
pub mod fragment_reconstructor;
pub mod fragment_tsv_exporter;
pub mod sutta_builder;
pub mod integration;
pub mod encoding;
pub mod fragments_schema;
pub mod fragments_models;

// Modular XML parser modules
pub mod xml_file_type;
pub mod xml_type_detector;
pub mod xml_parser_trait;
pub mod parsers;
pub mod xml_parser;

#[cfg(test)]
mod test_tsv_validation;

// Re-export main types for convenience
pub use types::{FragmentType, GroupType, GroupLevel, XmlFragment, FragmentAdjustment, FragmentKey, FragmentAdjustments, load_fragment_adjustments};
pub use nikaya_structure::NikayaStructure;
pub use nikaya_detector::detect_nikaya_structure;
pub use xml_parser::parse_into_fragments;
pub use fragment_exporter::{export_fragments_to_db, validate_fragments_db, ValidationStats};
pub use fragment_reconstructor::reconstruct_xml_from_db;
pub use fragment_tsv_exporter::export_fragments_to_tsv;
pub use integration::{TipitakaImporter, FileImportStats, ProcessingStats};
