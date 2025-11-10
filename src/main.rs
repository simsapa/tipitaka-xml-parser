use std::path::{Path, PathBuf};
use std::process::exit;

use clap::{Parser, Subcommand};
use dotenvy::dotenv;
use anyhow::Result;

use tipitaka_xml_parser::logger;

/// Parse Tipitaka XML files with fragment-based parser
fn parse_tipitaka_xml(
    xml_file: Option<&Path>,
    xml_dir: Option<&Path>,
    xml_list: Option<&Path>,
    fragments_db: Option<&Path>,
    dry_run: bool,
) -> Result<(), String> {
    use tipitaka_xml_parser::{
        TipitakaImporter,
        load_fragment_adjustments,
    };
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

    // Add fragment adjustments if provided
    if let Some(adj) = adjustments {
        importer = importer.with_adjustments(adj);
    }

    // Process each XML file
    let mut errors = 0;

    for (idx, xml_file) in xml_files.iter().enumerate() {
        logger::info(&format!("[{}/{}] Processing: {:?}",
                              idx + 1, xml_files.len(), xml_file.file_name().unwrap_or_default()));

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
                        
                        // If this is a reconstruction failure, exit immediately
                        // This is a critical error that indicates the fragment parser
                        // is not correctly splitting/storing the XML content
                        if e.to_string().contains("Reconstruction verification failed") {
                            eprintln!("Xml reconstruction verification failed, exiting.");
                            return Err(error_msg);
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
                    if stats.empty_both_codes > 0 {
                        let msg = format!("{} Sutta fragments have BOTH cst_code and sc_code empty", stats.empty_both_codes);
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

        /// Optional path to SQLite database for exporting fragments
        #[arg(long, value_name = "FRAGMENTS_DB_PATH")]
        fragments_db: Option<PathBuf>,

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
}

fn main() {
    // Attempt to load .env file with values such as ENABLE_PRINT_LOG=true
    let _ = dotenv();

    let cli = Cli::parse();

    // === Execute the requested command ===

    let command_result = match cli.command {
        Commands::ParseTipitakaXml { xml_file, xml_dir, xml_list, fragments_db, dry_run } => {
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
                parse_tipitaka_xml(
                    xml_file.as_deref(),
                    xml_dir.as_deref(),
                    xml_list.as_deref(),
                    fragments_db.as_deref(),
                    dry_run
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
    };

    if let Err(e) = command_result {
        logger::error(&format!("Error executing command: {}", e));
        exit(1);
    }
}
