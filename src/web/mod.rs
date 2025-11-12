/// Web UI module for fragment review and correction
/// 
/// This module provides a web-based interface for reviewing and correcting
/// XML fragment boundaries parsed from Tipitaka XML files.

pub mod routes;
pub mod models;
pub mod state;
pub mod settings;

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
    // Update settings with command-line arguments
    if let Err(e) = settings::update_settings_from_args(db_path, port) {
        eprintln!("Warning: Failed to update settings: {}", e);
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
