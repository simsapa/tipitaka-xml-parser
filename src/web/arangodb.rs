//! ArangoDB connection management for SuttaCentral integration
//!
//! This module provides functionality to connect to a local SuttaCentral
//! ArangoDB instance and fetch Pali sutta titles for the fragment review UI.
//!
//! ## Connection Details
//!
//! The module connects to ArangoDB using hardcoded credentials:
//! - Host: `http://localhost:8529`
//! - Username: `root`
//! - Password: `test`
//! - Database: `suttacentral`
//!
//! ## Features
//!
//! - Connection status checking for health monitoring
//! - Fetching Pali root titles from the `names` collection

use std::collections::HashMap;
use anyhow::{Result, Context};
use arangors::{Connection, Database};
use arangors::client::reqwest::ReqwestClient;
use serde_json::Value;

/// ArangoDB connection constants
const ARANGO_HOST: &str = "http://localhost:8529";
const ARANGO_USER: &str = "root";
const ARANGO_PASSWORD: &str = "test";
const ARANGO_DATABASE: &str = "suttacentral";

/// Connect to the local SuttaCentral ArangoDB instance
///
/// Establishes a connection to ArangoDB at localhost:8529 using credentials:
/// - username: "root"
/// - password: "test"
/// - database: "suttacentral"
///
/// Returns a Database handle that can be used for queries.
pub async fn connect_to_arangodb() -> Result<Database<ReqwestClient>> {
    let conn = Connection::establish_basic_auth(ARANGO_HOST, ARANGO_USER, ARANGO_PASSWORD)
        .await
        .context("Failed to establish connection to ArangoDB")?;

    let db = conn.db(ARANGO_DATABASE)
        .await
        .context("Failed to access 'suttacentral' database")?;

    Ok(db)
}

/// Check if ArangoDB is reachable and the suttacentral database is accessible
///
/// Returns a tuple of (connected: bool, error_message: Option<String>)
/// - If connected successfully, returns (true, None)
/// - If connection fails, returns (false, Some(error_description))
///
/// This function is used for the status indicator health check.
pub async fn check_connection_status() -> (bool, Option<String>) {
    match connect_to_arangodb().await {
        Ok(_) => (true, None),
        Err(e) => (false, Some(format!("Cannot connect to ArangoDB at {}: {}", ARANGO_HOST, e))),
    }
}

/// Fetch all Pali root titles from the SuttaCentral `names` collection
///
/// Queries ArangoDB with: `FOR x IN names FILTER x.is_root == true RETURN x`
///
/// Returns a HashMap mapping uid (sc_code) to name (title).
/// For example: "dn1" -> "Brahmajālasutta"
pub async fn get_pali_titles() -> Result<HashMap<String, String>> {
    let db = connect_to_arangodb().await?;

    let aql = "FOR x IN names FILTER x.is_root == true RETURN x";

    let results: Vec<Value> = db.aql_str(aql)
        .await
        .context("Failed to execute AQL query for Pali titles")?;

    let mut result_map = HashMap::new();

    for doc in results {
        if let (Some(uid), Some(name)) = (
            doc.get("uid").and_then(|v| v.as_str()),
            doc.get("name").and_then(|v| v.as_str())
        ) {
            result_map.insert(uid.to_string(), name.to_string());
        }
    }

    Ok(result_map)
}
