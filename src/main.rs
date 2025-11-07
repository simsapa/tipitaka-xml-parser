use std::path::{Path, PathBuf};
use std::process::exit;

use clap::{Parser, Subcommand};
use dotenvy::dotenv;
use anyhow::Result;

use tipitaka_xml_parser::logger;

/// Parse Tipitaka XML files with fragment-based parser
fn parse_tipitaka_xml(
    input_path: &Path,
    fragments_db: Option<&Path>,
    adjust_fragments_tsv: Option<&Path>,
    dry_run: bool,
) -> Result<(), String> {
    use tipitaka_xml_parser::{
        TipitakaImporter,
        load_fragment_adjustments,
    };
    use std::fs;

    // Load fragment adjustments if provided
    let adjustments = if let Some(tsv_path) = adjust_fragments_tsv {
        match load_fragment_adjustments(&PathBuf::from(tsv_path)) {
            Ok(adj) => {
                logger::info(&format!("Loaded {} fragment adjustments", adj.len()));
                Some(adj)
            }
            Err(e) => {
                return Err(format!("Failed to load fragment adjustments: {}", e));
            }
        }
    } else {
        None
    };

    // Collect XML files to process
    let xml_files: Vec<PathBuf> = if input_path.is_file() {
        logger::info(&format!("Processing single file: {:?}", input_path));
        vec![input_path.to_path_buf()]
    } else if input_path.is_dir() {
        logger::info(&format!("Processing folder: {:?}", input_path));
        let files: Vec<PathBuf> = fs::read_dir(input_path)
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
    } else {
        return Err(format!("Input path does not exist: {:?}", input_path));
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
        if let Some(frag_db_path) = fragments_db {
            if !dry_run {
                match importer.export_fragments(xml_file, frag_db_path) {
                    Ok(count) => {
                        logger::info(&format!("Exported {} fragments to {:?}", count, frag_db_path));
                    }
                    Err(e) => {
                        logger::error(&format!("Error exporting fragments: {}", e));
                        errors += 1;
                        continue;
                    }
                }
            }
        }

        logger::info("Processing complete");
    }

    if errors > 0 {
        logger::error(&format!("Total errors: {}", errors));
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
        /// Path to a single XML file or folder containing XML files
        #[arg(value_name = "INPUT_PATH")]
        input_path: PathBuf,

        /// Optional path to SQLite database for exporting fragments
        #[arg(long, value_name = "FRAGMENTS_DB_PATH")]
        fragments_db: Option<PathBuf>,

        /// Optional path to TSV file containing manual fragment adjustments
        #[arg(long, value_name = "ADJUST_FRAGMENTS_TSV")]
        adjust_fragments_tsv: Option<PathBuf>,

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
}

fn main() {
    // Attempt to load .env file with values such as ENABLE_PRINT_LOG=true
    let _ = dotenv();

    let cli = Cli::parse();

    // === Execute the requested command ===

    let command_result = match cli.command {
        Commands::ParseTipitakaXml { input_path, fragments_db, adjust_fragments_tsv, dry_run } => {
            parse_tipitaka_xml(&input_path, fragments_db.as_deref(), adjust_fragments_tsv.as_deref(), dry_run)
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
                    Ok(input_text) => {
                        let output_text = input_text.replace(r#"encoding="UTF-16""#, r#"encoding="UTF-8""#);
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
    };

    if let Err(e) = command_result {
        logger::error(&format!("Error executing command: {}", e));
        exit(1);
    }
}
