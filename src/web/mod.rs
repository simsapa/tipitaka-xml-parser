/// Web UI module for fragment review and correction
/// 
/// This module provides a web-based interface for reviewing and correcting
/// XML fragment boundaries parsed from Tipitaka XML files.

pub mod arangodb;
pub mod routes;
pub mod models;
pub mod state;
pub mod settings;
pub mod validation;

// Re-export commonly used functions
pub use settings::{load_settings, load_settings_from_path, load_or_create_default_settings, generate_default_paths};

use rocket::{Rocket, Build, Config};
use rocket::figment::Figment;
use rocket::fs::FileServer;
use std::path::{Path, PathBuf};
use anyhow::Result;

use crate::web::state::DbState;

/// Initialize and configure the Rocket web server
fn build_server(db_path: &Path, port: u16) -> Rocket<Build> {
    let figment = Figment::from(Config::default())
        .merge(("port", port));
    
    // Get the path to static files relative to the binary location
    let static_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/static");
    
    rocket::custom(figment)
        .manage(DbState {
            db_path: db_path.to_path_buf(),
        })
        .mount("/", routes::get_routes())
        .mount("/static", FileServer::from(static_dir))
}

/// Start the web server (blocking call)
pub fn start_server(db_path: &Path, port: u16) -> Result<()> {
    // Load current settings
    let mut settings = settings::load_settings().unwrap_or_default();
    
    // Update db_path and port in settings if they differ
    let db_path_str = db_path.to_string_lossy().to_string();
    let mut updated = false;
    
    if settings.db_path != db_path_str {
        settings.db_path = db_path_str;
        updated = true;
    }
    
    if settings.port != port {
        settings.port = port;
        updated = true;
    }
    
    // Save updated settings
    if updated {
        if let Err(e) = settings::save_settings(&settings) {
            eprintln!("Warning: Failed to save updated settings: {}", e);
        }
    }
    
    let rocket = build_server(db_path, port);
    
    // Launch the server - this is async, so we need tokio runtime
    let runtime = tokio::runtime::Runtime::new()?;
    runtime.block_on(async {
        rocket.launch().await
            .map_err(|e| anyhow::anyhow!("Rocket launch failed: {}", e))
    })?;
    
    Ok(())
}
