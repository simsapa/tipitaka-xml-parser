//! Parser modules for different nikaya types
//!
//! This module contains specialized parsers for different nikaya structures.

pub mod general;
pub mod helpers;

// Nikaya-specific parsers
pub mod samyutta_nikaya_mula;
pub mod samyutta_nikaya_atthakatha;
pub mod samyutta_nikaya_tika;

pub mod anguttara_nikaya_mula;
pub mod anguttara_nikaya_atthakatha;
pub mod anguttara_nikaya_tika;

// Re-export parsers
pub use general::GeneralParser;
pub use samyutta_nikaya_mula::SamyuttaNikayaMula;
pub use samyutta_nikaya_atthakatha::SamyuttaNikayaAtthakatha;
pub use samyutta_nikaya_tika::SamyuttaNikayaTika;
pub use anguttara_nikaya_mula::AnguttaraNikayaMula;
pub use anguttara_nikaya_atthakatha::AnguttaraNikayaAtthakatha;
pub use anguttara_nikaya_tika::AnguttaraNikayaTika;
