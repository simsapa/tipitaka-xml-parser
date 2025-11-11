/// Rocket state management for database connection
/// 
/// This module manages the application state, particularly the database
/// connection pool shared across request handlers.

use std::path::PathBuf;
use diesel::prelude::*;
use diesel::sqlite::SqliteConnection;
use anyhow::{Result, Context};

/// Application state containing database connection information
pub struct DbState {
    pub db_path: PathBuf,
}

impl DbState {
    /// Create a new database connection
    pub fn connect(&self) -> Result<SqliteConnection> {
        let db_url = self.db_path.to_str()
            .context("Invalid database path")?;
        
        SqliteConnection::establish(db_url)
            .with_context(|| format!("Failed to connect to database: {}", db_url))
    }
}
