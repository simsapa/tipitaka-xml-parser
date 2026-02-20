/// Rocket state management for database connection
///
/// This module manages the application state, particularly the database
/// connection pool shared across request handlers.

use std::path::PathBuf;
use diesel::sqlite::SqliteConnection;
use anyhow::Result;

use crate::fragment_exporter::establish_connection_and_migrate;

/// Application state containing database connection information
pub struct DbState {
    pub db_path: PathBuf,
}

impl DbState {
    /// Create a new database connection and run any pending migrations
    pub fn connect(&self) -> Result<SqliteConnection> {
        establish_connection_and_migrate(&self.db_path)
    }
}
