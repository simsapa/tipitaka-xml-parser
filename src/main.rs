use std::path::{Path, PathBuf};
use std::process::exit;

use clap::{Parser, Subcommand};
use dotenvy::dotenv;
use anyhow::Result;

use tipitaka_xml_parser::logger;
use tipitaka_xml_parser::types::ParserError;

/// Parse Tipitaka XML files with fragment-based parser
fn parse_tipitaka_xml(
    xml_file: Option<&Path>,
    xml_dir: Option<&Path>,
    xml_list: Option<&Path>,
    fragments_db: Option<&Path>,
    reference_fragments_db: Option<&Path>,
    dry_run: bool,
) -> Result<(), String> {
    use tipitaka_xml_parser::{
        TipitakaImporter,
        load_fragment_adjustments,
    };
    use tipitaka_xml_parser::types::ParserOverrides;
    use tipitaka_xml_parser::fragment_exporter::extract_all_correction_overrides;
    use std::fs;

    // Load fragment adjustments from embedded TSV
    let adjustments = match load_fragment_adjustments() {
        Ok(adj) => {
            logger::info(&format!("Loaded {} fragment adjustments", adj.len()));
            Some(adj)
        }
        Err(e) => {
            return Err(format!("Failed to load fragment adjustments: {}", e));
        }
    };

    // Extract correction overrides from reference database if provided
    let correction_overrides = if let Some(ref_db_path) = reference_fragments_db {
        match extract_all_correction_overrides(ref_db_path) {
            Ok(overrides) => {
                if !overrides.is_empty() {
                    logger::info(&format!("Loaded {} correction overrides from reference database", overrides.len()));
                    println!("Loaded {} correction overrides from reference database", overrides.len());
                }
                Some(overrides)
            }
            Err(e) => {
                // Not a critical error - continue without overrides
                logger::warn(&format!("Failed to load correction overrides from reference: {}", e));
                eprintln!("Warning: Failed to load correction overrides from reference: {}", e);
                None
            }
        }
    } else {
        None
    };

    // Build ParserOverrides combining adjustments and correction overrides
    // Note: pali_titles is None in CLI mode; could be enhanced to fetch from ArangoDB
    let overrides = ParserOverrides {
        adjustments,
        correction_overrides,
        pali_titles: None,
    };

    // Collect XML files to process
    let xml_files: Vec<PathBuf> = if let Some(file_path) = xml_file {
        logger::info(&format!("Processing single file: {:?}", file_path));
        if !file_path.exists() {
            return Err(format!("XML file does not exist: {:?}", file_path));
        }
        if !file_path.is_file() {
            return Err(format!("Path is not a file: {:?}", file_path));
        }
        vec![file_path.to_path_buf()]
    } else if let Some(dir_path) = xml_dir {
        logger::info(&format!("Processing folder: {:?}", dir_path));
        if !dir_path.exists() {
            return Err(format!("Directory does not exist: {:?}", dir_path));
        }
        if !dir_path.is_dir() {
            return Err(format!("Path is not a directory: {:?}", dir_path));
        }
        let files: Vec<PathBuf> = fs::read_dir(dir_path)
            .map_err(|e| format!("Failed to read directory: {}", e))?
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.path())
            .filter(|path| {
                path.extension()
                    .and_then(|ext| ext.to_str())
                    .map(|ext| ext == "xml")
                    .unwrap_or(false)
            })
            .collect();

        logger::info(&format!("Found {} XML files", files.len()));
        files
    } else if let Some(list_path) = xml_list {
        logger::info(&format!("Processing file list: {:?}", list_path));
        if !list_path.exists() {
            return Err(format!("List file does not exist: {:?}", list_path));
        }
        if !list_path.is_file() {
            return Err(format!("Path is not a file: {:?}", list_path));
        }
        let list_content = fs::read_to_string(list_path)
            .map_err(|e| format!("Failed to read list file: {}", e))?;
        
        let files: Vec<PathBuf> = list_content
            .lines()
            .map(|line| line.trim())
            .filter(|line| !line.is_empty())
            .map(PathBuf::from)
            .collect();

        logger::info(&format!("Found {} XML files in list", files.len()));
        
        // Validate that all files exist
        for file_path in &files {
            if !file_path.exists() {
                return Err(format!("XML file from list does not exist: {:?}", file_path));
            }
            if !file_path.is_file() {
                return Err(format!("Path from list is not a file: {:?}", file_path));
            }
        }
        
        files
    } else {
        // This should never happen due to validation in main()
        return Err("No input source specified".to_string());
    };

    if xml_files.is_empty() {
        return Err("No XML files found to process".to_string());
    }

    if dry_run {
        logger::info("DRY RUN MODE - No database operations will be performed");
    }

    let mut importer = TipitakaImporter::new()
        .map_err(|e| format!("Failed to create importer: {}", e))?;

    // Add parser overrides (includes adjustments and checked overrides from reference DB)
    importer = importer.with_overrides(overrides);

    // Set reference database path for row count validation
    if let Some(ref_db_path) = reference_fragments_db {
        importer = importer.with_reference_db(ref_db_path.to_path_buf());
    }

    // Process each XML file
    let mut errors = 0;

    for (idx, xml_file) in xml_files.iter().enumerate() {
        let msg = format!("[{}/{}] Processing: {:?}", idx + 1, xml_files.len(), xml_file.file_name().unwrap_or_default());
        logger::info(&msg);
        println!("{}", msg);

        // Handle fragments export if specified (unique feature of new parser)
        // Export includes automatic reconstruction verification to ensure data integrity
        if let Some(frag_db_path) = fragments_db {
            if !dry_run {
                match importer.export_fragments(xml_file, frag_db_path) {
                    Ok(count) => {
                        logger::info(&format!("Exported {} fragments to {:?}", count, frag_db_path));
                    }
                    Err(e) => {
                        let error_msg = format!("Error exporting fragments from {:?}: {}", 
                            xml_file.file_name().unwrap_or_default(), e);
                        logger::error(&error_msg);
                        eprintln!("{}", error_msg);
                        
                        // Check for critical parser errors that require immediate exit
                        if let Some(parser_err) = e.downcast_ref::<ParserError>() {
                            if parser_err.is_critical() {
                                eprintln!("Critical error, exiting.\n{}", e);
                                return Err(error_msg);
                            }
                        }
                        
                        errors += 1;
                        continue;
                    }
                }
            }
        }

        logger::info("Processing complete");
    }

    if errors > 0 {
        let error_msg = format!("Processing completed with {} errors", errors);
        logger::error(&error_msg);
        return Err(error_msg);
    }

    // Validate the fragments database if it was used
    if let Some(frag_db_path) = fragments_db {
        if !dry_run {
            use tipitaka_xml_parser::validate_fragments_db;
            
            logger::info("Validating fragments database...");
            match validate_fragments_db(frag_db_path) {
                Ok(stats) => {
                    logger::info(&format!("Validation complete: {} total Sutta fragments", stats.total_sutta_fragments));
                    
                    // Print warnings for missing codes
                    if stats.empty_cst_code > 0 {
                        let msg = format!("{} Sutta fragments have empty cst_code", stats.empty_cst_code);
                        logger::error(&msg);
                        eprintln!("{}", msg);
                    }
                    if stats.empty_sc_code > 0 {
                        let msg = format!("{} Sutta fragments have empty sc_code", stats.empty_sc_code);
                        logger::error(&msg);
                        eprintln!("{}", msg);
                    }

                    // Summary message if everything is good
                    if stats.empty_cst_code == 0 && stats.empty_sc_code == 0 {
                        logger::info("All Sutta fragments have both cst_code and sc_code");
                    }
                }
                Err(e) => {
                    logger::error(&format!("Failed to validate fragments database: {}", e));
                }
            }
        }
    }

    Ok(())
}

/// Reconstruct XML file from fragments database
fn reconstruct_xml_from_fragments(
    fragments_db_path: &Path,
    xml_filename: &str,
    output_path: &Path,
) -> Result<(), String> {
    use tipitaka_xml_parser::reconstruct_xml_from_db;
    use std::fs;

    if !fragments_db_path.exists() {
        return Err(format!("Fragments database not found: {:?}", fragments_db_path));
    }

    // Reconstruct XML
    let xml_content = reconstruct_xml_from_db(fragments_db_path, xml_filename)
        .map_err(|e| format!("Failed to reconstruct XML: {}", e))?;

    logger::info(&format!("Reconstructed {} bytes of XML content", xml_content.len()));

    // Write to output file
    fs::write(output_path, &xml_content)
        .map_err(|e| format!("Failed to write output file: {}", e))?;

    logger::info(&format!("Written to: {:?}", output_path));

    Ok(())
}

#[derive(Parser, Debug)]
#[command(author, version, about = "Tipitaka-xml Parser", long_about = None)]
#[command(propagate_version = true)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Parse VRI CST Tipitaka XML files with fragment-based parser
    #[command(arg_required_else_help = true)]
    ParseTipitakaXml {
        /// Path to a single XML file
        #[arg(long, value_name = "XML_FILE")]
        xml_file: Option<PathBuf>,

        /// Path to a folder containing XML files
        #[arg(long, value_name = "XML_DIR")]
        xml_dir: Option<PathBuf>,

        /// Path to a file containing a list of XML files (one per line)
        #[arg(long, value_name = "XML_LIST")]
        xml_list: Option<PathBuf>,

        /// Optional path to SQLite database for exporting fragments (deprecated, use --new-fragments-db)
        #[arg(long, value_name = "FRAGMENTS_DB_PATH")]
        fragments_db: Option<PathBuf>,

        /// Path to the new fragments SQLite database to create
        #[arg(long, value_name = "NEW_FRAGMENTS_DB_PATH")]
        new_fragments_db: Option<PathBuf>,

        /// Path to the reference fragments SQLite database for copying reviewed fragments
        #[arg(long, value_name = "REFERENCE_FRAGMENTS_DB_PATH")]
        reference_fragments_db: Option<PathBuf>,

        /// Parse without inserting into database (dry run)
        #[arg(long, default_value_t = false)]
        dry_run: bool,
    },

    /// Reconstruct XML file from fragments database
    #[command(arg_required_else_help = true)]
    ReconstructXmlFromFragments {
        /// Path to the fragments SQLite database
        #[arg(value_name = "FRAGMENTS_DB_PATH")]
        fragments_db_path: PathBuf,

        /// XML filename to reconstruct (as stored in nikaya table)
        #[arg(value_name = "XML_FILENAME")]
        xml_filename: String,

        /// Path to write the reconstructed XML output
        #[arg(value_name = "OUTPUT_PATH")]
        output_path: PathBuf,
    },

    /// Convert Tipitaka XML file to UTF-8 (normalizes line endings to LF)
    #[command(arg_required_else_help = true)]
    TipitakaXmlToUtf8 {
        /// Path to the input XML file
        #[arg(value_name = "INPUT_XML_PATH")]
        input_xml_path: PathBuf,

        /// Path to write the UTF-8 encoded output
        #[arg(value_name = "OUTPUT_PATH")]
        output_path: PathBuf,
    },

    /// Export xml_fragments table to TSV format
    #[command(arg_required_else_help = true)]
    ExportFragmentsToTsv {
        /// Path to the fragments SQLite database
        #[arg(value_name = "FRAGMENTS_DB_PATH")]
        fragments_db_path: PathBuf,

        /// Path to write the TSV output
        #[arg(value_name = "OUTPUT_TSV_PATH")]
        output_tsv_path: PathBuf,
    },

    /// Check if TSV export differences have no regressions
    #[command(arg_required_else_help = true)]
    CheckTsvRegressions {
        /// Path to the older TSV export
        #[arg(value_name = "OLD_TSV_PATH")]
        old_tsv_path: PathBuf,

        /// Path to the newer TSV export
        #[arg(value_name = "NEW_TSV_PATH")]
        new_tsv_path: PathBuf,
    },

    /// Start web UI for fragment review and correction
    WebUi {
        /// Optional path to config file (defaults to web-ui-config.toml in current directory)
        #[arg(long, value_name = "CONFIG_PATH")]
        config: Option<PathBuf>,

        /// Port to run the web server on (overrides config file)
        #[arg(long)]
        port: Option<u16>,
    },

    /// Regenerate fragments database from XML files
    ///
    /// Reads configuration from web-ui-config.toml (or specified config file) and regenerates
    /// the fragments database. By default, uses the current database as a reference to preserve
    /// correction overrides and review status.
    Regenerate {
        /// Optional path to config file (defaults to web-ui-config.toml in current directory)
        #[arg(long, value_name = "CONFIG_PATH")]
        config: Option<PathBuf>,

        /// Don't use reference database - regenerate completely fresh without preserving corrections
        #[arg(long, default_value_t = false)]
        fresh: bool,
    },

    /// Convert XML files to test data (UTF-8 normalized files)
    ///
    /// Reads configuration from web-ui-config.toml (or specified config file) and converts
    /// each XML file to UTF-8 normalized format in the specified data folder.
    TipitakaXmlToTestData {
        /// Optional path to config file (defaults to web-ui-config.toml in current directory)
        #[arg(long, value_name = "CONFIG_PATH")]
        config: Option<PathBuf>,

        /// Path to the data folder (defaults to tests/data/)
        #[arg(long, value_name = "DATA_FOLDER")]
        data_folder: Option<PathBuf>,
    },
}

fn main() {
    // Attempt to load .env file with values such as ENABLE_PRINT_LOG=true
    let _ = dotenv();

    let cli = Cli::parse();

    // === Execute the requested command ===

    let command_result = match cli.command {
        Commands::ParseTipitakaXml { xml_file, xml_dir, xml_list, fragments_db, new_fragments_db, reference_fragments_db, dry_run } => {
            // Validate that exactly one input source is specified
            let input_count = [xml_file.is_some(), xml_dir.is_some(), xml_list.is_some()]
                .iter()
                .filter(|&&x| x)
                .count();
            
            if input_count == 0 {
                Err("Error: Must specify exactly one of --xml-file, --xml-dir, or --xml-list".to_string())
            } else if input_count > 1 {
                Err("Error: Cannot specify more than one of --xml-file, --xml-dir, or --xml-list".to_string())
            } else {
                // Support backwards compatibility: if fragments_db is specified but not new_fragments_db, use fragments_db
                let target_db = new_fragments_db.as_deref().or(fragments_db.as_deref());
                
                parse_tipitaka_xml(
                    xml_file.as_deref(),
                    xml_dir.as_deref(),
                    xml_list.as_deref(),
                    target_db,
                    reference_fragments_db.as_deref(),
                    dry_run,
                )
            }
        }

        Commands::ReconstructXmlFromFragments { fragments_db_path, xml_filename, output_path } => {
            reconstruct_xml_from_fragments(&fragments_db_path, &xml_filename, &output_path)
        }

        Commands::TipitakaXmlToUtf8 { input_xml_path, output_path } => {
            use std::fs;
            use tipitaka_xml_parser::encoding::read_xml_file;

            if !input_xml_path.exists() {
                Err(format!("Input XML file does not exist: {:?}", input_xml_path))
            } else if !input_xml_path.is_file() {
                Err(format!("Input path is not a file: {:?}", input_xml_path))
            } else {
                match read_xml_file(&input_xml_path) {
                    Ok(output_text) => {
                        match fs::write(&output_path, output_text) {
                            Ok(()) => {
                                logger::info(&format!("Wrote UTF-8 file to {:?}", output_path));
                                Ok(())
                            }
                            Err(e) => Err(format!("Failed to write output file {:?}: {}", output_path, e)),
                        }
                    }
                    Err(e) => Err(format!("Failed to read XML file: {}", e)),
                }
            }
        }

        Commands::ExportFragmentsToTsv { fragments_db_path, output_tsv_path } => {
            use tipitaka_xml_parser::export_fragments_to_tsv;

            if !fragments_db_path.exists() {
                Err(format!("Fragments database does not exist: {:?}", fragments_db_path))
            } else if !fragments_db_path.is_file() {
                Err(format!("Fragments database path is not a file: {:?}", fragments_db_path))
            } else {
                match export_fragments_to_tsv(&fragments_db_path, &output_tsv_path) {
                    Ok(count) => {
                        logger::info(&format!("Exported {} fragments to TSV: {:?}", count, output_tsv_path));
                        Ok(())
                    }
                    Err(e) => {
                        let error_msg = format!("Failed to export fragments to TSV: {}", e);
                        logger::error(&error_msg);
                        eprintln!("{}", error_msg);
                        Err(error_msg)
                    }
                }
            }
        }

        Commands::CheckTsvRegressions { old_tsv_path, new_tsv_path } => {
            use tipitaka_xml_parser::check_tsv_improvements;

            if !old_tsv_path.exists() {
                Err(format!("Old TSV file does not exist: {:?}", old_tsv_path))
            } else if !old_tsv_path.is_file() {
                Err(format!("Old TSV path is not a file: {:?}", old_tsv_path))
            } else if !new_tsv_path.exists() {
                Err(format!("New TSV file does not exist: {:?}", new_tsv_path))
            } else if !new_tsv_path.is_file() {
                Err(format!("New TSV path is not a file: {:?}", new_tsv_path))
            } else {
                match check_tsv_improvements(&old_tsv_path, &new_tsv_path) {
                    Ok(result) => {
                        logger::info(&format!("Checked {} total rows, {} rows differ", 
                            result.total_rows, result.differing_rows));
                        println!("Total rows: {}", result.total_rows);
                        println!("Differing rows: {}", result.differing_rows);
                        
                        if result.validation_errors.is_empty() {
                            let msg = "No regressions detected in the differing rows";
                            logger::info(&msg);
                            println!("{}", msg);
                            Ok(())
                        } else {
                            println!("\nFound {} regressions (values that were correct in old file are now incorrect):\n", result.validation_errors.len());
                            logger::error(&format!("Found {} regressions", result.validation_errors.len()));
                            
                            for error in &result.validation_errors {
                                println!("REGRESSION - File: {}, Fragment: {}", error.cst_file, error.frag_idx);
                                println!("  CST Code: {}", error.cst_code);
                                println!("  Old value was CORRECT, new value is INCORRECT");
                                println!("  New cst_sutta: {:?}", error.actual_cst_sutta);
                                println!("  Expected cst_sutta: {}", error.expected_cst_sutta);
                                println!();
                            }
                            
                            Err(format!("Found {} regressions in TSV - values that were correct are now incorrect", result.validation_errors.len()))
                        }
                    }
                    Err(e) => {
                        let error_msg = format!("Failed to check TSV improvements: {}", e);
                        logger::error(&error_msg);
                        eprintln!("{}", error_msg);
                        Err(error_msg)
                    }
                }
            }
        }

        Commands::WebUi { config, port } => {
            use tipitaka_xml_parser::web;

            // Helper function to avoid early return issues
            let run_webui = || -> Result<(), String> {
                // Load settings from config file
                let settings = if let Some(config_path) = &config {
                    // Load from custom config path
                    web::load_settings_from_path(config_path)
                        .map_err(|e| format!("Failed to load config from {:?}: {}", config_path, e))?
                } else {
                    // Load from default path or create default settings
                    web::load_or_create_default_settings()
                        .map_err(|e| format!("Failed to load settings: {}", e))?
                };

                // Port from command line overrides config file
                let actual_port = port.unwrap_or(settings.port);

                // Validate that db_path is configured
                if settings.db_path.is_empty() {
                    return Err("Database path not configured. Please configure web-ui-config.toml or run with --config flag.".to_string());
                }

                let db_path = PathBuf::from(&settings.db_path);
                if !db_path.exists() {
                    return Err(format!("Fragments database does not exist: {:?}", db_path));
                }
                if !db_path.is_file() {
                    return Err(format!("Fragments database path is not a file: {:?}", db_path));
                }

                logger::info(&format!("Starting web UI server on port {}", actual_port));
                logger::info(&format!("Database: {:?}", db_path));
                println!("Starting web UI on http://localhost:{}", actual_port);

                // This will block until the server is shut down
                match web::start_server(&db_path, actual_port) {
                    Ok(_) => {
                        logger::info("Web server shut down");
                        Ok(())
                    }
                    Err(e) => {
                        let error_msg = format!("Failed to start web server: {}", e);
                        logger::error(&error_msg);
                        Err(error_msg)
                    }
                }
            };

            run_webui()
        }

        Commands::Regenerate { config, fresh } => {
            use tipitaka_xml_parser::web;
            use tipitaka_xml_parser::regenerate::{RegenerateConfig, regenerate_fragments_db};
            use tipitaka_xml_parser::logger;

            // use_reference_db is true by default, unless --fresh is specified
            let use_reference_db = !fresh;

            // Helper function to avoid early return issues
            let run_regenerate =|| -> Result<(), String> {
                // Load settings from config file
                let mut settings = if let Some(config_path) = &config {
                    // Load from custom config path
                    web::load_settings_from_path(config_path)
                        .map_err(|e| format!("Failed to load config from {:?}: {}", config_path, e))?
                } else {
                    // Load from default path or create default settings
                    web::load_or_create_default_settings()
                        .map_err(|e| format!("Failed to load settings: {}", e))?
                };

                // Generate default paths
                web::generate_default_paths(&mut settings);

                // Try to load Pali titles from ArangoDB
                let pali_titles = match tokio::runtime::Runtime::new() {
                    Ok(runtime) => {
                        match runtime.block_on(async { web::arangodb::get_pali_titles().await }) {
                            Ok(titles) => {
                                logger::info(&format!("Loaded {} Pali titles from ArangoDB", titles.len()));
                                Some(titles)
                            }
                            Err(e) => {
                                logger::warn(&format!("Failed to load Pali titles from ArangoDB: {}", e));
                                None
                            }
                        }
                    }
                    Err(e) => {
                        logger::warn(&format!("Failed to create tokio runtime: {}", e));
                        None
                    }
                };

                // Validate required settings
                if settings.db_path.is_empty() {
                    return Err("Database path not configured. Please configure web-ui-config.toml or run with --config flag.".to_string());
                }
                if settings.xml_dir.is_empty() {
                    return Err("XML directory not configured. Please configure settings first.".to_string());
                }
                if settings.xml_filenames.is_empty() {
                    return Err("No XML filenames configured. Please configure settings first.".to_string());
                }

                let new_db_path = settings.new_fragments_db_path.as_ref()
                    .ok_or_else(|| "New fragments DB path not configured".to_string())?;
                let ref_db_path = settings.reference_fragments_db_path.as_ref()
                    .ok_or_else(|| "Reference fragments DB path not configured".to_string())?;
                let new_tsv_path = settings.new_fragments_tsv_path.as_ref()
                    .ok_or_else(|| "New fragments TSV path not configured".to_string())?;
                let ref_tsv_path = settings.reference_fragments_tsv_path.as_ref()
                    .ok_or_else(|| "Reference fragments TSV path not configured".to_string())?;

                // Build regeneration config from settings
                let regen_config = RegenerateConfig {
                    db_path: PathBuf::from(&settings.db_path),
                    new_db_path: PathBuf::from(new_db_path),
                    reference_db_path: PathBuf::from(ref_db_path),
                    new_tsv_path: PathBuf::from(new_tsv_path),
                    reference_tsv_path: PathBuf::from(ref_tsv_path),
                    xml_dir: PathBuf::from(&settings.xml_dir),
                    xml_filenames: settings.xml_filenames.clone(),
                    use_reference_db,
                    pali_titles,
                };

                logger::info(&format!("Regenerating fragments database (use_reference_db: {})", use_reference_db));
                println!("Regenerating fragments database...");
                println!("  Database: {}", settings.db_path);
                println!("  XML directory: {}", settings.xml_dir);
                println!("  XML files: {} files", settings.xml_filenames.len());
                println!("  Use reference DB: {}", use_reference_db);
                println!();

                // Run regeneration
                let result = regenerate_fragments_db(&regen_config);

                // Print output
                println!("{}", result.output);

                if result.success {
                    if result.db_replaced {
                        println!("Regeneration completed successfully. Database has been replaced.");
                    } else {
                        println!("Regeneration completed successfully.");
                    }
                    Ok(())
                } else {
                    Err("Regeneration failed. See output above for details.".to_string())
                }
            };

            run_regenerate()
        }

        Commands::TipitakaXmlToTestData { config, data_folder } => {
            use tipitaka_xml_parser::web;
            use tipitaka_xml_parser::encoding::read_xml_file;
            use std::fs;

            let run_tipitaka_xml_to_test_data = || -> Result<(), String> {
                let settings = if let Some(config_path) = &config {
                    web::load_settings_from_path(config_path)
                        .map_err(|e| format!("Failed to load config from {:?}: {}", config_path, e))?
                } else {
                    web::load_or_create_default_settings()
                        .map_err(|e| format!("Failed to load settings: {}", e))?
                };

                if settings.xml_dir.is_empty() {
                    return Err("XML directory not configured. Please configure web-ui-config.toml or run with --config flag.".to_string());
                }
                if settings.xml_filenames.is_empty() {
                    return Err("No XML filenames configured. Please configure web-ui-config.toml or run with --config flag.".to_string());
                }

                let data_folder = data_folder.unwrap_or_else(|| PathBuf::from("tests/data"));

                if !data_folder.exists() {
                    return Err(format!("Data folder does not exist: {:?}", data_folder));
                }

                let xml_dir = PathBuf::from(&settings.xml_dir);

                println!("Converting XML files to test data...");
                println!("  XML directory: {:?}", xml_dir);
                println!("  Data folder: {:?}", data_folder);
                println!("  XML files: {} files", settings.xml_filenames.len());
                println!();

                let mut errors = 0;

                for (idx, xml_filename) in settings.xml_filenames.iter().enumerate() {
                    let input_xml_path = xml_dir.join(xml_filename);
                    let output_path = data_folder.join(xml_filename);

                    println!("[{}/{}] Processing: {}", idx + 1, settings.xml_filenames.len(), xml_filename);

                    if !input_xml_path.exists() {
                        eprintln!("  ERROR: Input XML file does not exist: {:?}", input_xml_path);
                        errors += 1;
                        continue;
                    }

                    match read_xml_file(&input_xml_path) {
                        Ok(output_text) => {
                            match fs::write(&output_path, output_text) {
                                Ok(()) => {
                                    println!("  Wrote: {:?}", output_path);
                                }
                                Err(e) => {
                                    eprintln!("  ERROR: Failed to write output file {:?}: {}", output_path, e);
                                    errors += 1;
                                }
                            }
                        }
                        Err(e) => {
                            eprintln!("  ERROR: Failed to read XML file {:?}: {}", input_xml_path, e);
                            errors += 1;
                        }
                    }
                }

                println!();
                if errors > 0 {
                    Err(format!("Completed with {} errors", errors))
                } else {
                    println!("Successfully converted {} XML files to test data.", settings.xml_filenames.len());
                    Ok(())
                }
            };

            run_tipitaka_xml_to_test_data()
        }
    };

    if let Err(e) = command_result {
        logger::error(&format!("Error executing command: {}", e));
        exit(1);
    }
}
