/// Settings management for web UI
/// 
/// Handles loading and saving application settings to a TOML configuration file.

use std::fs;
use std::path::{Path, PathBuf};
use anyhow::{Result, Context};
use crate::web::models::AppSettings;

/// Default config file name
const CONFIG_FILE: &str = "web-ui-config.toml";

/// Get the path to the config file (in the current directory)
pub fn get_config_path() -> PathBuf {
    PathBuf::from(CONFIG_FILE)
}

/// Load settings from TOML config file
pub fn load_settings() -> Result<AppSettings> {
    let config_path = get_config_path();
    
    if !config_path.exists() {
        // Return default settings if config doesn't exist
        return Ok(AppSettings::default());
    }
    
    let content = fs::read_to_string(&config_path)
        .with_context(|| format!("Failed to read config file: {:?}", config_path))?;
    
    let settings: AppSettings = toml::from_str(&content)
        .with_context(|| format!("Failed to parse TOML config: {:?}", config_path))?;
    
    Ok(settings)
}

/// Save settings to TOML config file
pub fn save_settings(settings: &AppSettings) -> Result<()> {
    let config_path = get_config_path();
    
    let toml_content = toml::to_string_pretty(settings)
        .context("Failed to serialize settings to TOML")?;
    
    fs::write(&config_path, toml_content)
        .with_context(|| format!("Failed to write config file: {:?}", config_path))?;
    
    Ok(())
}

/// Update settings from command-line arguments (db_path and port)
pub fn update_settings_from_args(db_path: &Path, port: u16) -> Result<()> {
    let mut settings = load_settings().unwrap_or_default();
    
    // Update db_path if it's set and different
    let db_path_str = db_path.to_string_lossy().to_string();
    if !db_path_str.is_empty() && settings.db_path != db_path_str {
        settings.db_path = db_path_str;
    }
    
    // Update port if different from current setting
    if settings.port != port {
        settings.port = port;
    }
    
    // Save updated settings
    save_settings(&settings)?;
    
    Ok(())
}
