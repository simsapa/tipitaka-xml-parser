//! Parser modules for different nikaya types
//!
//! This module contains specialized parsers for different nikaya structures.

pub mod helpers;
pub mod digha_nikaya_mula;
pub mod general;

pub use digha_nikaya_mula::DighaNikayaMulaParser;
pub use general::GeneralParser;
