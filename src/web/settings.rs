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

/// Load settings from TOML config file at default path
pub fn load_settings() -> Result<AppSettings> {
    let config_path = get_config_path();
    load_settings_from_path(&config_path)
}

/// Load settings from a specific TOML config file path
pub fn load_settings_from_path(config_path: &Path) -> Result<AppSettings> {
    if !config_path.exists() {
        // Return default settings if config doesn't exist
        return Ok(AppSettings::default());
    }
    
    let content = fs::read_to_string(config_path)
        .with_context(|| format!("Failed to read config file: {:?}", config_path))?;
    
    let settings: AppSettings = toml::from_str(&content)
        .with_context(|| format!("Failed to parse TOML config: {:?}", config_path))?;
    
    Ok(settings)
}

/// Load settings or create default if not exists
pub fn load_or_create_default_settings() -> Result<AppSettings> {
    let config_path = get_config_path();
    
    if !config_path.exists() {
        // Create default config file
        let default_settings = AppSettings::default();
        save_settings(&default_settings)?;
        println!("Created default config file: {:?}", config_path);
        println!("Please edit this file to configure the web UI.");
        Ok(default_settings)
    } else {
        load_settings_from_path(&config_path)
    }
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

/// Generate default paths based on the current db_path
pub fn generate_default_paths(settings: &mut AppSettings) {
    use std::path::Path;
    
    if settings.db_path.is_empty() {
        return;
    }
    
    let db_path = Path::new(&settings.db_path);
    let db_dir = db_path.parent().unwrap_or(Path::new("."));
    let db_stem = db_path.file_stem().and_then(|s| s.to_str()).unwrap_or("fragments");
    
    // Generate default new_fragments_db_path (same dir, -new suffix)
    if settings.new_fragments_db_path.is_none() {
        let new_db = db_dir.join(format!("{}-new.sqlite3", db_stem));
        settings.new_fragments_db_path = Some(new_db.to_string_lossy().to_string());
    }
    
    // Generate default reference_fragments_db_path (same dir, -reference suffix)
    if settings.reference_fragments_db_path.is_none() {
        let ref_db = db_dir.join(format!("{}-reference.sqlite3", db_stem));
        settings.reference_fragments_db_path = Some(ref_db.to_string_lossy().to_string());
    }
    
    // Generate default new_fragments_tsv_path (based on new db)
    if settings.new_fragments_tsv_path.is_none() {
        if let Some(new_db_path) = &settings.new_fragments_db_path {
            let new_db = Path::new(new_db_path);
            let new_tsv = new_db.with_extension("tsv");
            settings.new_fragments_tsv_path = Some(new_tsv.to_string_lossy().to_string());
        }
    }
    
    // Generate default reference_fragments_tsv_path (reference suffix in filename)
    if settings.reference_fragments_tsv_path.is_none() {
        if let Some(new_tsv_path) = &settings.new_fragments_tsv_path {
            let new_tsv = Path::new(new_tsv_path);
            let tsv_stem = new_tsv.file_stem().and_then(|s| s.to_str()).unwrap_or("fragments");
            let tsv_dir = new_tsv.parent().unwrap_or(Path::new("."));
            let ref_tsv = tsv_dir.join(format!("{}-reference.tsv", tsv_stem));
            settings.reference_fragments_tsv_path = Some(ref_tsv.to_string_lossy().to_string());
        }
    }
    
    // Generate default xml_parser_binary_path (target/debug/tipitaka_xml_parser in project root)
    if settings.xml_parser_binary_path.is_none() {
        // Assume the binary is in target/debug relative to current directory
        settings.xml_parser_binary_path = Some("target/debug/tipitaka_xml_parser".to_string());
    }
}
