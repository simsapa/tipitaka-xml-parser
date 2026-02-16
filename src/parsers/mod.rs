//! Parser modules for different nikaya types
//!
//! This module contains specialized parsers for different nikaya structures.

pub mod general;
pub mod helpers;

// Nikaya-specific parsers
pub mod samyutta_nikaya_mula;
pub mod samyutta_nikaya_commentary;

// Re-export parsers
pub use general::GeneralParser;
pub use samyutta_nikaya_mula::SamyuttaNikayaMula;
pub use samyutta_nikaya_commentary::SamyuttaNikayaCommentary;
