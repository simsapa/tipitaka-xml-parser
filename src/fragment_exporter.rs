//! Export fragments and nikaya structure to SQLite database
//!
//! This module provides functionality to export parsed XML fragments and nikaya
//! structure to an SQLite database for inspection and debugging.

use anyhow::{Result, Context};
use diesel::prelude::*;
use diesel::sqlite::SqliteConnection;
use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};
use std::path::Path;

use crate::types::{XmlFragment, CorrectionFragmentOverride, CorrectionFragmentOverrides, FragmentKey, InsertedFragmentData, FragmentType, parse_frag_idx_code, compare_frag_idx_code};
use std::collections::HashMap;
use crate::nikaya_structure::NikayaStructure;
use crate::fragments_models::{NewNikayaStructure, NewXmlFragment};
use crate::fragments_schema::nikaya_structures;
use crate::sutta_builder::xml_to_html;

// Embed the fragments migrations
pub const FRAGMENTS_MIGRATIONS: EmbeddedMigrations = embed_migrations!("migrations/fragments/");

/// Export fragments and nikaya structure to SQLite database
///
/// Creates two tables via diesel migrations:
/// - `nikaya_structures`: Stores the NikayaStructure with unique nikaya field
/// - `xml_fragments`: Stores each XmlFragment with cst_file and nikaya foreign key
///
/// # Arguments
/// * `fragments` - Vector of parsed XML fragments
/// * `nikaya_structure` - The nikaya structure configuration
/// * `db_path` - Path to the SQLite database file (will be created if doesn't exist)
///
/// # Returns
/// Number of fragments exported or error
pub fn export_fragments_to_db(
    fragments: &[XmlFragment],
    nikaya_structure: &NikayaStructure,
    db_path: &Path,
) -> Result<usize> {
    // Connect to database and run migrations
    let mut conn = establish_connection_and_migrate(db_path)?;

    // Insert or get nikaya structure
    insert_nikaya_structure(&mut conn, nikaya_structure)?;
    
    // Insert fragments with nikaya foreign key
    let count = insert_fragments(&mut conn, fragments)?;
    
    Ok(count)
}

/// Run pending migrations for fragments database
fn run_migrations(conn: &mut SqliteConnection) -> Result<()> {
    conn.run_pending_migrations(FRAGMENTS_MIGRATIONS)
        .map_err(|e| anyhow::anyhow!("Failed to execute pending database migrations: {}", e))?;
    Ok(())
}

/// Establish a database connection and run any pending migrations.
///
/// This is the recommended way to connect to the fragments database.
/// It ensures that the database schema is always up-to-date by running
/// any pending migrations automatically.
///
/// # Arguments
/// * `db_path` - Path to the SQLite database file
///
/// # Returns
/// A connected `SqliteConnection` with migrations applied
pub fn establish_connection_and_migrate(db_path: &Path) -> Result<SqliteConnection> {
    let mut conn = SqliteConnection::establish(db_path.to_str().unwrap())
        .context("Failed to connect to fragments database")?;

    run_migrations(&mut conn)?;

    Ok(conn)
}

/// Insert nikaya structure into database using diesel model
fn insert_nikaya_structure(
    conn: &mut SqliteConnection,
    structure: &NikayaStructure,
) -> Result<()> {
    // Serialize levels as JSON
    let levels_json = serde_json::to_string(&structure.levels)
        .context("Failed to serialize nikaya levels")?;
    
    let new_nikaya = NewNikayaStructure {
        nikaya: &structure.nikaya,
        levels: &levels_json,
    };
    
    // Use INSERT OR IGNORE to handle duplicate nikayas
    diesel::insert_or_ignore_into(nikaya_structures::table)
        .values(&new_nikaya)
        .execute(conn)
        .context("Failed to insert nikaya structure")?;
    
    Ok(())
}

/// Insert fragments into database using diesel models
fn insert_fragments(
    conn: &mut SqliteConnection,
    fragments: &[XmlFragment],
) -> Result<usize> {
    use crate::fragments_schema::xml_fragments;
    
    // Delete existing fragments for this file first (if any)
    // This ensures that re-parsing a file replaces old fragments instead of duplicating them
    if let Some(first_fragment) = fragments.first() {
        let _deleted = diesel::delete(
            xml_fragments::table.filter(xml_fragments::cst_file.eq(&first_fragment.cst_file))
        )
        .execute(conn)
        .context("Failed to delete existing fragments for file")?;
    }
    
    let mut count = 0;
    
    for fragment in fragments {
        let frag_type = match fragment.frag_type {
            crate::types::FragmentType::Header => "Header",
            crate::types::FragmentType::Sutta => "Sutta",
        };
        
        let group_levels_json = serde_json::to_string(&fragment.group_levels)
            .context("Failed to serialize group levels")?;
        
        // Generate HTML from XML only for Sutta fragments
        let content_html = match fragment.frag_type {
            crate::types::FragmentType::Sutta => xml_to_html(&fragment.content_xml).ok(),
            crate::types::FragmentType::Header => None,
        };

        let new_fragment = NewXmlFragment {
            cst_file: &fragment.cst_file,
            frag_idx_code: &fragment.frag_idx_code,
            frag_type,
            frag_review: fragment.frag_review.as_deref(),
            nikaya: &fragment.nikaya,
            cst_code: fragment.cst_code.as_deref(),
            sc_code: fragment.sc_code.as_deref(),
            content_xml: &fragment.content_xml,
            content_html: content_html.as_deref(),
            cst_vagga: fragment.cst_vagga.as_deref(),
            cst_sutta: fragment.cst_sutta.as_deref(),
            cst_paranum: fragment.cst_paranum.as_deref(),
            sc_sutta: fragment.sc_sutta.as_deref(),
            start_line: fragment.start_line as i32,
            start_char: fragment.start_char as i32,
            end_line: fragment.end_line as i32,
            end_char: fragment.end_char as i32,
            group_levels: &group_levels_json,
        };
        
        diesel::insert_into(xml_fragments::table)
            .values(&new_fragment)
            .execute(conn)
            .context("Failed to insert fragment")?;
        
        count += 1;
    }
    
    Ok(count)
}

/// Statistics for database validation
#[derive(Debug, Clone, Default)]
pub struct ValidationStats {
    pub total_sutta_fragments: usize,
    pub empty_cst_code: usize,
    pub empty_sc_code: usize,
}

/// Validate the fragments database and return statistics
///
/// Checks Sutta type xml_fragments for missing cst_code or sc_code values
///
/// # Arguments
/// * `db_path` - Path to the fragments database
///
/// # Returns
/// ValidationStats with counts of fragments with missing codes
pub fn validate_fragments_db(db_path: &Path) -> Result<ValidationStats> {
    let mut conn = establish_connection_and_migrate(db_path)?;
    
    #[derive(QueryableByName)]
    struct CountResult {
        #[diesel(sql_type = diesel::sql_types::BigInt)]
        count: i64,
    }
    
    // Count total Sutta fragments
    let total_suttas: CountResult = diesel::sql_query(
        "SELECT COUNT(*) as count FROM xml_fragments WHERE frag_type = 'Sutta'"
    )
    .get_result(&mut conn)
    .context("Failed to count total Sutta fragments")?;
    
    // Count Suttas with empty or null cst_code
    let empty_cst: CountResult = diesel::sql_query(
        "SELECT COUNT(*) as count FROM xml_fragments 
         WHERE frag_type = 'Sutta' AND (cst_code IS NULL OR cst_code = '')"
    )
    .get_result(&mut conn)
    .context("Failed to count fragments with empty cst_code")?;
    
    // Count Suttas with empty or null sc_code
    let empty_sc: CountResult = diesel::sql_query(
        "SELECT COUNT(*) as count FROM xml_fragments 
         WHERE frag_type = 'Sutta' AND (sc_code IS NULL OR sc_code = '')"
    )
    .get_result(&mut conn)
    .context("Failed to count fragments with empty sc_code")?;
    
    Ok(ValidationStats {
        total_sutta_fragments: total_suttas.count as usize,
        empty_cst_code: empty_cst.count as usize,
        empty_sc_code: empty_sc.count as usize,
    })
}

/// Validate that first and last fragments are Header type.
///
/// This is a structural invariant: XML files must start and end with Header fragments
/// (containing metadata), not Sutta fragments. This validation ensures the parser
/// correctly identifies fragment boundaries.
///
/// # Arguments
/// * `fragments` - Parsed fragments to validate
/// * `filename` - Name of the XML file being processed (for error reporting)
///
/// # Returns
/// Ok(()) if validation passes, or HeaderValidationFailed error if not
pub fn validate_first_last_headers(
    fragments: &[XmlFragment],
    filename: &str,
) -> Result<()> {
    use crate::types::{FragmentType, ParserError};

    if fragments.is_empty() {
        return Err(ParserError::HeaderValidationFailed {
            filename: filename.to_string(),
            details: "no fragments found".to_string(),
        }.into());
    }

    // Check first fragment
    let first = &fragments[0];
    if first.frag_type != FragmentType::Header {
        return Err(ParserError::HeaderValidationFailed {
            filename: filename.to_string(),
            details: format!(
                "first fragment (frag_idx_code {}) is {:?}, expected Header",
                first.frag_idx_code,
                first.frag_type
            ),
        }.into());
    }

    // Check last fragment
    let last = &fragments[fragments.len() - 1];
    if last.frag_type != FragmentType::Header {
        return Err(ParserError::HeaderValidationFailed {
            filename: filename.to_string(),
            details: format!(
                "last fragment (frag_idx_code {}) is {:?}, expected Header",
                last.frag_idx_code,
                last.frag_type
            ),
        }.into());
    }

    Ok(())
}

/// Extract correction fragment overrides, frag_review status, and inserted fragments from the database.
///
/// Queries fragments where `frag_review` is not null, empty, or 'unchecked' for the given file.
/// Returns correction overrides, review status map, and inserted fragments (those with sub-index > 0).
///
/// "Moved" fragments get `collapse: true` with no boundary/SC overrides.
/// Other reviewed generated fragments get their boundary and SC values as overrides.
/// Inserted fragments (sub-index > 0) are extracted with full content and boundary data.
///
/// # Arguments
/// * `db_path` - Path to the fragments database
/// * `cst_file` - The XML file name to extract overrides for
///
/// # Returns
/// Tuple of:
/// - `CorrectionFragmentOverrides` - HashMap of overrides keyed by (cst_file, frag_idx_code)
/// - `HashMap<String, String>` - Map of frag_idx_code to frag_review status for restoration
/// - `Vec<InsertedFragmentData>` - Inserted fragments with full data for re-injection
pub fn extract_correction_overrides(
    db_path: &Path,
    cst_file: &str,
) -> Result<(CorrectionFragmentOverrides, HashMap<String, String>, Vec<InsertedFragmentData>)> {
    let mut conn = establish_connection_and_migrate(db_path)?;

    #[derive(QueryableByName)]
    struct CorrectionFragmentRow {
        #[diesel(sql_type = diesel::sql_types::Text)]
        frag_idx_code: String,
        #[diesel(sql_type = diesel::sql_types::Integer)]
        start_line: i32,
        #[diesel(sql_type = diesel::sql_types::Integer)]
        start_char: i32,
        #[diesel(sql_type = diesel::sql_types::Integer)]
        end_line: i32,
        #[diesel(sql_type = diesel::sql_types::Integer)]
        end_char: i32,
        #[diesel(sql_type = diesel::sql_types::Text)]
        content_xml: String,
        #[diesel(sql_type = diesel::sql_types::Nullable<diesel::sql_types::Text>)]
        sc_code: Option<String>,
        #[diesel(sql_type = diesel::sql_types::Nullable<diesel::sql_types::Text>)]
        sc_sutta: Option<String>,
        #[diesel(sql_type = diesel::sql_types::Nullable<diesel::sql_types::Text>)]
        cst_code: Option<String>,
        #[diesel(sql_type = diesel::sql_types::Nullable<diesel::sql_types::Text>)]
        cst_vagga: Option<String>,
        #[diesel(sql_type = diesel::sql_types::Nullable<diesel::sql_types::Text>)]
        cst_sutta: Option<String>,
        #[diesel(sql_type = diesel::sql_types::Nullable<diesel::sql_types::Text>)]
        cst_paranum: Option<String>,
        #[diesel(sql_type = diesel::sql_types::Nullable<diesel::sql_types::Text>)]
        frag_review: Option<String>,
        #[diesel(sql_type = diesel::sql_types::Text)]
        frag_type: String,
        #[diesel(sql_type = diesel::sql_types::Text)]
        nikaya: String,
        #[diesel(sql_type = diesel::sql_types::Text)]
        group_levels: String,
    }

    // Query fragments with frag_review status that indicates they've been reviewed
    // Excludes: NULL, empty string, and 'unchecked'
    // Now includes additional fields needed for inserted fragments
    let rows: Vec<CorrectionFragmentRow> = diesel::sql_query(
        "SELECT frag_idx_code, start_line, start_char, end_line, end_char, content_xml,
                sc_code, sc_sutta, cst_code, cst_vagga, cst_sutta, cst_paranum,
                frag_review, frag_type, nikaya, group_levels
         FROM xml_fragments
         WHERE cst_file = ?
           AND frag_review IS NOT NULL
           AND frag_review != ''
           AND frag_review != 'unchecked'"
    )
    .bind::<diesel::sql_types::Text, _>(cst_file)
    .load(&mut conn)
    .context("Failed to query correction fragments")?;

    let mut overrides = CorrectionFragmentOverrides::new();
    let mut review_status = HashMap::new();
    let mut inserted_fragments = Vec::new();

    for row in rows {
        let frag_idx_code = row.frag_idx_code.clone();
        let frag_review = row.frag_review.as_deref().unwrap_or("");

        // Parse frag_type from string to enum
        let frag_type_enum = match row.frag_type.as_str() {
            "Header" => FragmentType::Header,
            "Sutta" => FragmentType::Sutta,
            _ => FragmentType::Sutta, // Default to Sutta if unknown
        };

        // Check if this is an inserted fragment (sub-index > 0)
        let (_, minor) = parse_frag_idx_code(&frag_idx_code);
        let is_inserted = minor > 0;

        if is_inserted {
            // For inserted fragments, store full data for re-injection
            inserted_fragments.push(InsertedFragmentData {
                frag_idx_code: frag_idx_code.clone(),
                content_xml: row.content_xml,
                start_line: row.start_line as usize,
                start_char: row.start_char as usize,
                end_line: row.end_line as usize,
                end_char: row.end_char as usize,
                frag_type: frag_type_enum.clone(),
                frag_review: row.frag_review.clone(),
                cst_code: row.cst_code,
                sc_code: row.sc_code,
                cst_vagga: row.cst_vagga,
                cst_sutta: row.cst_sutta,
                cst_paranum: row.cst_paranum,
                sc_sutta: row.sc_sutta,
                nikaya: row.nikaya,
                group_levels: row.group_levels,
            });
        } else {
            // For generated fragments (sub-index = 0), store as correction override
            let override_data = match frag_review {
                "moved" => CorrectionFragmentOverride {
                    collapse: true,
                    // Don't use stale boundary values from moved fragments
                    start_line: None,
                    start_char: None,
                    end_line: None,
                    end_char: None,
                    // Moved fragments have no SC data
                    sc_code: None,
                    sc_sutta: None,
                    // Moved fragments have no CST metadata
                    cst_code: None,
                    cst_vagga: None,
                    cst_sutta: None,
                    cst_paranum: None,
                    // frag_review status is handled separately via review_status map
                    frag_review: row.frag_review.clone(),
                    frag_type: Some(frag_type_enum),
                },
                _ => CorrectionFragmentOverride {
                    collapse: false,
                    // Include start position for fragments that may follow an inserted fragment
                    start_line: Some(row.start_line as usize),
                    start_char: Some(row.start_char as usize),
                    end_line: Some(row.end_line as usize),
                    end_char: Some(row.end_char as usize),
                    sc_code: row.sc_code,
                    sc_sutta: row.sc_sutta,
                    cst_code: row.cst_code,
                    cst_vagga: row.cst_vagga,
                    cst_sutta: row.cst_sutta,
                    cst_paranum: row.cst_paranum,
                    frag_review: row.frag_review.clone(),
                    frag_type: Some(frag_type_enum),
                },
            };

            let key = FragmentKey {
                cst_file: cst_file.to_string(),
                frag_idx_code: frag_idx_code.clone(),
            };

            overrides.insert(key, override_data);
        }

        // Store the frag_review status for restoration (both generated and inserted)
        if let Some(status) = row.frag_review {
            review_status.insert(frag_idx_code, status);
        }
    }

    // Sort inserted fragments by frag_idx_code
    inserted_fragments.sort_by(|a, b| compare_frag_idx_code(&a.frag_idx_code, &b.frag_idx_code));

    Ok((overrides, review_status, inserted_fragments))
}

/// Extract correction fragment overrides and inserted fragments from the database for all files.
///
/// This variant queries ALL corrected fragments across all files, used during
/// full database regeneration. "Moved" fragments get `collapse: true`.
/// Inserted fragments (sub-index > 0) are extracted with full content and boundary data.
///
/// # Arguments
/// * `db_path` - Path to the fragments database
///
/// # Returns
/// Tuple of:
/// - `CorrectionFragmentOverrides` - HashMap of overrides keyed by (cst_file, frag_idx_code)
/// - `InsertedFragmentsMap` - HashMap of cst_file -> Vec<InsertedFragmentData>
pub fn extract_all_correction_overrides(
    db_path: &Path,
) -> Result<(CorrectionFragmentOverrides, crate::types::InsertedFragmentsMap)> {
    let mut conn = establish_connection_and_migrate(db_path)?;

    #[derive(QueryableByName)]
    struct CorrectionFragmentRow {
        #[diesel(sql_type = diesel::sql_types::Text)]
        cst_file: String,
        #[diesel(sql_type = diesel::sql_types::Text)]
        frag_idx_code: String,
        #[diesel(sql_type = diesel::sql_types::Integer)]
        start_line: i32,
        #[diesel(sql_type = diesel::sql_types::Integer)]
        start_char: i32,
        #[diesel(sql_type = diesel::sql_types::Integer)]
        end_line: i32,
        #[diesel(sql_type = diesel::sql_types::Integer)]
        end_char: i32,
        #[diesel(sql_type = diesel::sql_types::Text)]
        content_xml: String,
        #[diesel(sql_type = diesel::sql_types::Nullable<diesel::sql_types::Text>)]
        sc_code: Option<String>,
        #[diesel(sql_type = diesel::sql_types::Nullable<diesel::sql_types::Text>)]
        sc_sutta: Option<String>,
        #[diesel(sql_type = diesel::sql_types::Nullable<diesel::sql_types::Text>)]
        cst_code: Option<String>,
        #[diesel(sql_type = diesel::sql_types::Nullable<diesel::sql_types::Text>)]
        cst_vagga: Option<String>,
        #[diesel(sql_type = diesel::sql_types::Nullable<diesel::sql_types::Text>)]
        cst_sutta: Option<String>,
        #[diesel(sql_type = diesel::sql_types::Nullable<diesel::sql_types::Text>)]
        cst_paranum: Option<String>,
        #[diesel(sql_type = diesel::sql_types::Nullable<diesel::sql_types::Text>)]
        frag_review: Option<String>,
        #[diesel(sql_type = diesel::sql_types::Text)]
        frag_type: String,
        #[diesel(sql_type = diesel::sql_types::Text)]
        nikaya: String,
        #[diesel(sql_type = diesel::sql_types::Text)]
        group_levels: String,
    }

    // Query ALL fragments with reviewed status across all files
    // Now includes additional fields needed for inserted fragments
    let rows: Vec<CorrectionFragmentRow> = diesel::sql_query(
        "SELECT cst_file, frag_idx_code, start_line, start_char, end_line, end_char, content_xml,
                sc_code, sc_sutta, cst_code, cst_vagga, cst_sutta, cst_paranum,
                frag_review, frag_type, nikaya, group_levels
         FROM xml_fragments
         WHERE frag_review IS NOT NULL
           AND frag_review != ''
           AND frag_review != 'unchecked'"
    )
    .load(&mut conn)
    .context("Failed to query all correction fragments")?;

    let mut overrides = CorrectionFragmentOverrides::new();
    let mut inserted_fragments_map: crate::types::InsertedFragmentsMap = HashMap::new();

    for row in rows {
        let frag_review = row.frag_review.as_deref().unwrap_or("");

        // Parse frag_type from string to enum
        let frag_type_enum = match row.frag_type.as_str() {
            "Header" => FragmentType::Header,
            "Sutta" => FragmentType::Sutta,
            _ => FragmentType::Sutta, // Default to Sutta if unknown
        };

        // Check if this is an inserted fragment (sub-index > 0)
        let (_, minor) = parse_frag_idx_code(&row.frag_idx_code);
        let is_inserted = minor > 0;

        if is_inserted {
            // For inserted fragments, store full data for re-injection
            let inserted_data = InsertedFragmentData {
                frag_idx_code: row.frag_idx_code.clone(),
                content_xml: row.content_xml,
                start_line: row.start_line as usize,
                start_char: row.start_char as usize,
                end_line: row.end_line as usize,
                end_char: row.end_char as usize,
                frag_type: frag_type_enum.clone(),
                frag_review: row.frag_review.clone(),
                cst_code: row.cst_code,
                sc_code: row.sc_code,
                cst_vagga: row.cst_vagga,
                cst_sutta: row.cst_sutta,
                cst_paranum: row.cst_paranum,
                sc_sutta: row.sc_sutta,
                nikaya: row.nikaya,
                group_levels: row.group_levels,
            };

            inserted_fragments_map
                .entry(row.cst_file.clone())
                .or_default()
                .push(inserted_data);
        } else {
            // For generated fragments (sub-index = 0), store as correction override
            let override_data = match frag_review {
                "moved" => CorrectionFragmentOverride {
                    collapse: true,
                    start_line: None,
                    start_char: None,
                    end_line: None,
                    end_char: None,
                    sc_code: None,
                    sc_sutta: None,
                    cst_code: None,
                    cst_vagga: None,
                    cst_sutta: None,
                    cst_paranum: None,
                    frag_review: row.frag_review.clone(),
                    frag_type: Some(frag_type_enum),
                },
                _ => CorrectionFragmentOverride {
                    collapse: false,
                    start_line: Some(row.start_line as usize),
                    start_char: Some(row.start_char as usize),
                    end_line: Some(row.end_line as usize),
                    end_char: Some(row.end_char as usize),
                    sc_code: row.sc_code,
                    sc_sutta: row.sc_sutta,
                    cst_code: row.cst_code,
                    cst_vagga: row.cst_vagga,
                    cst_sutta: row.cst_sutta,
                    cst_paranum: row.cst_paranum,
                    frag_review: row.frag_review.clone(),
                    frag_type: Some(frag_type_enum),
                },
            };

            let key = FragmentKey {
                cst_file: row.cst_file,
                frag_idx_code: row.frag_idx_code,
            };

            overrides.insert(key, override_data);
        }
    }

    // Sort inserted fragments for each file by frag_idx_code
    for fragments in inserted_fragments_map.values_mut() {
        fragments.sort_by(|a, b| compare_frag_idx_code(&a.frag_idx_code, &b.frag_idx_code));
    }

    Ok((overrides, inserted_fragments_map))
}

/// Count fragments for a specific file in the database.
///
/// Used for row count validation during database regeneration.
///
/// # Arguments
/// * `db_path` - Path to the fragments database
/// * `cst_file` - The XML file name
///
/// # Returns
/// Number of fragments for this file in the database
pub fn count_fragments_in_db(db_path: &Path, cst_file: &str) -> Result<usize> {
    let mut conn = establish_connection_and_migrate(db_path)?;

    #[derive(QueryableByName)]
    struct CountResult {
        #[diesel(sql_type = diesel::sql_types::BigInt)]
        count: i64,
    }

    let result: CountResult = diesel::sql_query(
        "SELECT COUNT(*) as count FROM xml_fragments WHERE cst_file = ?"
    )
    .bind::<diesel::sql_types::Text, _>(cst_file)
    .get_result(&mut conn)
    .context("Failed to count fragments")?;

    Ok(result.count as usize)
}

/// Restore frag_review status for fragments after parsing.
///
/// After reparsing a file, the new fragments lose their frag_review status.
/// This function restores the status from a previously extracted map.
///
/// # Arguments
/// * `db_path` - Path to the fragments database
/// * `cst_file` - The XML file name
/// * `review_status` - Map of frag_idx_code to frag_review status
///
/// # Returns
/// Number of fragments updated
pub fn restore_frag_review_status(
    db_path: &Path,
    cst_file: &str,
    review_status: &HashMap<String, String>,
) -> Result<usize> {
    if review_status.is_empty() {
        return Ok(0);
    }

    let mut conn = establish_connection_and_migrate(db_path)?;

    let mut updated = 0;

    for (frag_idx_code, status) in review_status {
        let result = diesel::sql_query(
            "UPDATE xml_fragments SET frag_review = ? WHERE cst_file = ? AND frag_idx_code = ?"
        )
        .bind::<diesel::sql_types::Text, _>(status)
        .bind::<diesel::sql_types::Text, _>(cst_file)
        .bind::<diesel::sql_types::Text, _>(frag_idx_code.as_str())
        .execute(&mut conn)
        .context("Failed to update frag_review status")?;

        updated += result;
    }

    Ok(updated)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;
    use crate::types::{FragmentType, GroupLevel, GroupType};
    
    #[test]
    fn test_export_fragments() {
        let temp_db = NamedTempFile::new().unwrap();
        let db_path = temp_db.path();
        
        // Create test data
        let structure = NikayaStructure {
            nikaya: "digha".to_string(),
            levels: vec![GroupType::Nikaya, GroupType::Book, GroupType::Sutta],
        };
        
        let fragments = vec![
            XmlFragment {
                nikaya: "digha".to_string(),
                frag_type: FragmentType::Header,
                content_xml: "<p rend=\"nikaya\">Dīghanikāyo</p>".to_string(),
                start_line: 1,
                end_line: 1,
                start_char: 0,
                end_char: 34,
                group_levels: vec![],
                cst_file: "test.xml".to_string(),
                frag_idx_code: "0.0".to_string(),
                frag_review: None,
                cst_code: None,
                cst_vagga: None,
                cst_sutta: None,
                cst_paranum: None,
                sc_code: None,
                sc_sutta: None,
            },
            XmlFragment {
                nikaya: "digha".to_string(),
                frag_type: FragmentType::Sutta,
                content_xml: "<p>Test content</p>".to_string(),
                start_line: 2,
                end_line: 2,
                start_char: 0,
                end_char: 19,
                group_levels: vec![
                    GroupLevel {
                        group_type: GroupType::Nikaya,
                        group_number: None,
                        title: "Dīghanikāyo".to_string(),
                        id: None,
                    },
                ],
                cst_file: "test.xml".to_string(),
                frag_idx_code: "1.0".to_string(),
                frag_review: None,
                cst_code: None,
                cst_vagga: None,
                cst_sutta: None,
                cst_paranum: None,
                sc_code: None,
                sc_sutta: None,
            },
        ];
        
        // Export
        let count = export_fragments_to_db(&fragments, &structure, db_path).unwrap();
        assert_eq!(count, 2);
        
        // Verify by querying
        let mut conn = SqliteConnection::establish(db_path.to_str().unwrap()).unwrap();
        
        #[derive(QueryableByName)]
        struct CountResult {
            #[diesel(sql_type = diesel::sql_types::BigInt)]
            count: i64,
        }
        
        // Check nikaya_structures table
        let nikaya_result: CountResult = diesel::sql_query("SELECT COUNT(*) as count FROM nikaya_structures")
            .get_result(&mut conn)
            .unwrap();
        assert_eq!(nikaya_result.count, 1);
        
        // Check fragments table
        let fragment_result: CountResult = diesel::sql_query("SELECT COUNT(*) as count FROM xml_fragments")
            .get_result(&mut conn)
            .unwrap();
        assert_eq!(fragment_result.count, 2);
    }

    #[test]
    fn test_foreign_key_relationship() {
        let temp_db = NamedTempFile::new().unwrap();
        let db_path = temp_db.path();
        
        // Create test data with two different nikayas
        let structure1 = NikayaStructure {
            nikaya: "digha".to_string(),
            levels: vec![GroupType::Nikaya, GroupType::Book, GroupType::Sutta],
        };
        
        let structure2 = NikayaStructure {
            nikaya: "majjhima".to_string(),
            levels: vec![GroupType::Nikaya, GroupType::Book, GroupType::Vagga, GroupType::Sutta],
        };
        
        let fragments1 = vec![
            XmlFragment {
                nikaya: "digha".to_string(),
                frag_type: FragmentType::Header,
                content_xml: "<p rend=\"nikaya\">Dīghanikāyo</p>".to_string(),
                start_line: 1,
                end_line: 1,
                start_char: 0,
                end_char: 34,
                group_levels: vec![],
                cst_file: "dn1.xml".to_string(),
                frag_idx_code: "0.0".to_string(),
                frag_review: None,
                cst_code: None,
                cst_vagga: None,
                cst_sutta: None,
                cst_paranum: None,
                sc_code: None,
                sc_sutta: None,
            },
        ];
        
        let fragments2 = vec![
            XmlFragment {
                nikaya: "majjhima".to_string(),
                frag_type: FragmentType::Header,
                content_xml: "<p rend=\"nikaya\">Majjhimanikāyo</p>".to_string(),
                start_line: 1,
                end_line: 1,
                start_char: 0,
                end_char: 35,
                group_levels: vec![],
                cst_file: "mn1.xml".to_string(),
                frag_idx_code: "0.0".to_string(),
                frag_review: None,
                cst_code: None,
                cst_vagga: None,
                cst_sutta: None,
                cst_paranum: None,
                sc_code: None,
                sc_sutta: None,
            },
        ];
        
        // Export first nikaya and its fragments
        export_fragments_to_db(&fragments1, &structure1, db_path).unwrap();
        
        // Export second nikaya and its fragments
        export_fragments_to_db(&fragments2, &structure2, db_path).unwrap();
        
        // Verify relationships
        let mut conn = SqliteConnection::establish(db_path.to_str().unwrap()).unwrap();
        
        #[derive(QueryableByName)]
        struct CountResult {
            #[diesel(sql_type = diesel::sql_types::BigInt)]
            count: i64,
        }
        
        #[derive(QueryableByName)]
        #[allow(dead_code)]
        struct NikayaIdResult {
            #[diesel(sql_type = diesel::sql_types::BigInt)]
            _nikaya_id: i64,
        }

        // Check we have 2 nikayas
        let nikaya_count: CountResult = diesel::sql_query("SELECT COUNT(*) as count FROM nikaya_structures")
            .get_result(&mut conn)
            .unwrap();
        assert_eq!(nikaya_count.count, 2);
        
        // Check we have 2 fragments total
        let fragment_count: CountResult = diesel::sql_query("SELECT COUNT(*) as count FROM xml_fragments")
            .get_result(&mut conn)
            .unwrap();
        assert_eq!(fragment_count.count, 2);
        
        // Verify each fragment has a valid nikaya
        #[derive(QueryableByName)]
        struct NikayaResult {
            #[diesel(sql_type = diesel::sql_types::Text)]
            nikaya: String,
        }
        
        let nikaya_ids: Vec<NikayaResult> = diesel::sql_query(
            "SELECT DISTINCT nikaya FROM xml_fragments ORDER BY nikaya"
        )
        .load(&mut conn)
        .unwrap();
        
        assert_eq!(nikaya_ids.len(), 2, "Should have fragments for 2 different nikayas");
        assert_eq!(nikaya_ids[0].nikaya, "digha", "First nikaya should be digha");
        assert_eq!(nikaya_ids[1].nikaya, "majjhima", "Second nikaya should be majjhima");
        
        // Verify we can query fragments by nikaya
        let digha_fragments: CountResult = diesel::sql_query(
            "SELECT COUNT(*) as count FROM xml_fragments WHERE nikaya = 'digha'"
        )
        .get_result(&mut conn)
        .unwrap();
        assert_eq!(digha_fragments.count, 1, "Digha nikaya should have 1 fragment");
        
        let majjhima_fragments: CountResult = diesel::sql_query(
            "SELECT COUNT(*) as count FROM xml_fragments WHERE nikaya = 'majjhima'"
        )
        .get_result(&mut conn)
        .unwrap();
        assert_eq!(majjhima_fragments.count, 1, "Majjhima nikaya should have 1 fragment");
    }

    #[test]
    fn test_validate_fragments_db() {
        let temp_db = NamedTempFile::new().unwrap();
        let db_path = temp_db.path();
        
        // Create test data with some fragments missing codes
        let structure = NikayaStructure {
            nikaya: "digha".to_string(),
            levels: vec![GroupType::Nikaya, GroupType::Book, GroupType::Sutta],
        };
        
        let fragments = vec![
            // Fragment with both codes
            XmlFragment {
                nikaya: "digha".to_string(),
                frag_type: FragmentType::Sutta,
                content_xml: "<p>Fragment with both codes</p>".to_string(),
                start_line: 1,
                end_line: 1,
                start_char: 0,
                end_char: 30,
                group_levels: vec![],
                cst_file: "test.xml".to_string(),
                frag_idx_code: "0.0".to_string(),
                frag_review: None,
                cst_code: Some("dn1.1".to_string()),
                cst_vagga: None,
                cst_sutta: None,
                cst_paranum: None,
                sc_code: Some("dn1".to_string()),
                sc_sutta: None,
            },
            // Fragment with only cst_code
            XmlFragment {
                nikaya: "digha".to_string(),
                frag_type: FragmentType::Sutta,
                content_xml: "<p>Fragment with only cst_code</p>".to_string(),
                start_line: 2,
                end_line: 2,
                start_char: 0,
                end_char: 33,
                group_levels: vec![],
                cst_file: "test.xml".to_string(),
                frag_idx_code: "1.0".to_string(),
                frag_review: None,
                cst_code: Some("dn1.2".to_string()),
                cst_vagga: None,
                cst_sutta: None,
                cst_paranum: None,
                sc_code: None,
                sc_sutta: None,
            },
            // Fragment with only sc_code
            XmlFragment {
                nikaya: "digha".to_string(),
                frag_type: FragmentType::Sutta,
                content_xml: "<p>Fragment with only sc_code</p>".to_string(),
                start_line: 3,
                end_line: 3,
                start_char: 0,
                end_char: 32,
                group_levels: vec![],
                cst_file: "test.xml".to_string(),
                frag_idx_code: "2.0".to_string(),
                frag_review: None,
                cst_code: None,
                cst_vagga: None,
                cst_sutta: None,
                cst_paranum: None,
                sc_code: Some("dn3".to_string()),
                sc_sutta: None,
            },
            // Fragment with neither code
            XmlFragment {
                nikaya: "digha".to_string(),
                frag_type: FragmentType::Sutta,
                content_xml: "<p>Fragment with no codes</p>".to_string(),
                start_line: 4,
                end_line: 4,
                start_char: 0,
                end_char: 28,
                group_levels: vec![],
                cst_file: "test.xml".to_string(),
                frag_idx_code: "3.0".to_string(),
                frag_review: None,
                cst_code: None,
                cst_vagga: None,
                cst_sutta: None,
                cst_paranum: None,
                sc_code: None,
                sc_sutta: None,
            },
            // Header fragment (should not be counted in validation)
            XmlFragment {
                nikaya: "digha".to_string(),
                frag_type: FragmentType::Header,
                content_xml: "<p>Header</p>".to_string(),
                start_line: 5,
                end_line: 5,
                start_char: 0,
                end_char: 13,
                group_levels: vec![],
                cst_file: "test.xml".to_string(),
                frag_idx_code: "4.0".to_string(),
                frag_review: None,
                cst_code: None,
                cst_vagga: None,
                cst_sutta: None,
                cst_paranum: None,
                sc_code: None,
                sc_sutta: None,
            },
        ];
        
        // Export fragments
        export_fragments_to_db(&fragments, &structure, db_path).unwrap();
        
        // Validate
        let stats = super::validate_fragments_db(db_path).unwrap();
        
        assert_eq!(stats.total_sutta_fragments, 4, "Should have 4 Sutta fragments");
        assert_eq!(stats.empty_cst_code, 2, "Should have 2 fragments with empty cst_code");
        assert_eq!(stats.empty_sc_code, 2, "Should have 2 fragments with empty sc_code");
    }

    #[test]
    fn test_extract_correction_overrides() {
        let temp_db = NamedTempFile::new().unwrap();
        let db_path = temp_db.path();

        // Create test data with various frag_review statuses
        let structure = NikayaStructure {
            nikaya: "samyutta".to_string(),
            levels: vec![GroupType::Nikaya, GroupType::Samyutta, GroupType::Vagga, GroupType::Sutta],
        };

        let fragments = vec![
            // Fragment with 'checked' status - should be extracted
            XmlFragment {
                nikaya: "samyutta".to_string(),
                frag_type: FragmentType::Sutta,
                content_xml: "<p>Checked fragment</p>".to_string(),
                start_line: 1,
                end_line: 10,
                start_char: 0,
                end_char: 50,
                group_levels: vec![],
                cst_file: "s0301m.mul.xml".to_string(),
                frag_idx_code: "162.0".to_string(),
                frag_review: Some("checked".to_string()),
                cst_code: Some("sn1.5.0.1".to_string()),
                cst_vagga: None,
                cst_sutta: Some("Āḷavikāsutta".to_string()),
                cst_paranum: None,
                sc_code: Some("sn5.1".to_string()),
                sc_sutta: Some("Āḷavikāsutta".to_string()),
            },
            // Fragment with 'in-progress' status - should be extracted
            XmlFragment {
                nikaya: "samyutta".to_string(),
                frag_type: FragmentType::Sutta,
                content_xml: "<p>In-progress fragment</p>".to_string(),
                start_line: 11,
                end_line: 20,
                start_char: 0,
                end_char: 60,
                group_levels: vec![],
                cst_file: "s0301m.mul.xml".to_string(),
                frag_idx_code: "163.0".to_string(),
                frag_review: Some("in-progress".to_string()),
                cst_code: Some("sn1.5.0.2".to_string()),
                cst_vagga: None,
                cst_sutta: None,
                cst_paranum: None,
                sc_code: None,
                sc_sutta: None,
            },
            // Fragment with 'unchecked' status - should NOT be extracted
            XmlFragment {
                nikaya: "samyutta".to_string(),
                frag_type: FragmentType::Sutta,
                content_xml: "<p>Unchecked fragment</p>".to_string(),
                start_line: 21,
                end_line: 30,
                start_char: 0,
                end_char: 70,
                group_levels: vec![],
                cst_file: "s0301m.mul.xml".to_string(),
                frag_idx_code: "164.0".to_string(),
                frag_review: Some("unchecked".to_string()),
                cst_code: Some("sn1.5.0.3".to_string()),
                cst_vagga: None,
                cst_sutta: None,
                cst_paranum: None,
                sc_code: None,
                sc_sutta: None,
            },
            // Fragment with None status - should NOT be extracted
            XmlFragment {
                nikaya: "samyutta".to_string(),
                frag_type: FragmentType::Sutta,
                content_xml: "<p>No review fragment</p>".to_string(),
                start_line: 31,
                end_line: 40,
                start_char: 0,
                end_char: 80,
                group_levels: vec![],
                cst_file: "s0301m.mul.xml".to_string(),
                frag_idx_code: "165.0".to_string(),
                frag_review: None,
                cst_code: Some("sn1.5.0.4".to_string()),
                cst_vagga: None,
                cst_sutta: None,
                cst_paranum: None,
                sc_code: None,
                sc_sutta: None,
            },
        ];

        // Export fragments
        export_fragments_to_db(&fragments, &structure, db_path).unwrap();

        // Extract checked overrides
        let (overrides, review_status, _inserted_fragments) = super::extract_correction_overrides(db_path, "s0301m.mul.xml").unwrap();

        // Should have 2 overrides (checked and in-progress, not unchecked or None)
        assert_eq!(overrides.len(), 2, "Should extract 2 correction overrides");
        assert_eq!(review_status.len(), 2, "Should have 2 review statuses");

        // Verify collapse is false for non-moved fragments
        let key_162 = crate::types::FragmentKey {
            cst_file: "s0301m.mul.xml".to_string(),
            frag_idx_code: "162.0".to_string(),
        };
        assert!(!overrides.get(&key_162).unwrap().collapse, "Checked fragment should not be collapsed");

        // Check the checked fragment override
        let override_162 = overrides.get(&key_162).expect("Should have override for frag_idx_code 162");
        assert_eq!(override_162.sc_code, Some("sn5.1".to_string()));
        assert_eq!(override_162.sc_sutta, Some("Āḷavikāsutta".to_string()));
        assert_eq!(override_162.end_line, Some(10));
        assert_eq!(override_162.end_char, Some(50));

        // Check review status
        assert_eq!(review_status.get("162.0"), Some(&"checked".to_string()));
        assert_eq!(review_status.get("163.0"), Some(&"in-progress".to_string()));

        // Verify unchecked fragment is not included
        let key_164 = crate::types::FragmentKey {
            cst_file: "s0301m.mul.xml".to_string(),
            frag_idx_code: "164.0".to_string(),
        };
        assert!(overrides.get(&key_164).is_none(), "Unchecked fragment should not be extracted");
    }

    #[test]
    fn test_restore_frag_review_status() {
        let temp_db = NamedTempFile::new().unwrap();
        let db_path = temp_db.path();

        // Create test data
        let structure = NikayaStructure {
            nikaya: "samyutta".to_string(),
            levels: vec![GroupType::Nikaya, GroupType::Samyutta, GroupType::Vagga, GroupType::Sutta],
        };

        // Initially create fragments without review status
        let fragments = vec![
            XmlFragment {
                nikaya: "samyutta".to_string(),
                frag_type: FragmentType::Sutta,
                content_xml: "<p>Fragment 1</p>".to_string(),
                start_line: 1,
                end_line: 10,
                start_char: 0,
                end_char: 50,
                group_levels: vec![],
                cst_file: "test.xml".to_string(),
                frag_idx_code: "0.0".to_string(),
                frag_review: None,  // No review status initially
                cst_code: None,
                cst_vagga: None,
                cst_sutta: None,
                cst_paranum: None,
                sc_code: None,
                sc_sutta: None,
            },
            XmlFragment {
                nikaya: "samyutta".to_string(),
                frag_type: FragmentType::Sutta,
                content_xml: "<p>Fragment 2</p>".to_string(),
                start_line: 11,
                end_line: 20,
                start_char: 0,
                end_char: 60,
                group_levels: vec![],
                cst_file: "test.xml".to_string(),
                frag_idx_code: "1.0".to_string(),
                frag_review: None,  // No review status initially
                cst_code: None,
                cst_vagga: None,
                cst_sutta: None,
                cst_paranum: None,
                sc_code: None,
                sc_sutta: None,
            },
        ];

        // Export fragments
        export_fragments_to_db(&fragments, &structure, db_path).unwrap();

        // Create review status map (simulating previously extracted statuses)
        let mut review_status = std::collections::HashMap::new();
        review_status.insert("0.0".to_string(), "checked".to_string());
        review_status.insert("1.0".to_string(), "in-progress".to_string());

        // Restore the statuses
        let updated = super::restore_frag_review_status(db_path, "test.xml", &review_status).unwrap();
        assert_eq!(updated, 2, "Should update 2 fragments");

        // Verify the statuses were restored
        let mut conn = SqliteConnection::establish(db_path.to_str().unwrap()).unwrap();

        #[allow(dead_code)]
        #[derive(QueryableByName)]
        struct FragReviewResult {
            #[diesel(sql_type = diesel::sql_types::Text)]
            frag_idx_code: String,
            #[diesel(sql_type = diesel::sql_types::Nullable<diesel::sql_types::Text>)]
            frag_review: Option<String>,
        }

        let results: Vec<FragReviewResult> = diesel::sql_query(
            "SELECT frag_idx_code, frag_review FROM xml_fragments WHERE cst_file = 'test.xml' ORDER BY frag_idx_code"
        )
        .load(&mut conn)
        .unwrap();

        assert_eq!(results.len(), 2);
        assert_eq!(results[0].frag_review, Some("checked".to_string()));
        assert_eq!(results[1].frag_review, Some("in-progress".to_string()));
    }

    #[test]
    fn test_extract_all_correction_overrides() {
        let temp_db = NamedTempFile::new().unwrap();
        let db_path = temp_db.path();

        // Create test data with checked fragments in multiple files
        let structure = NikayaStructure {
            nikaya: "samyutta".to_string(),
            levels: vec![GroupType::Nikaya, GroupType::Samyutta, GroupType::Vagga, GroupType::Sutta],
        };

        let fragments1 = vec![
            XmlFragment {
                nikaya: "samyutta".to_string(),
                frag_type: FragmentType::Sutta,
                content_xml: "<p>File 1 checked</p>".to_string(),
                start_line: 1,
                end_line: 10,
                start_char: 0,
                end_char: 50,
                group_levels: vec![],
                cst_file: "file1.xml".to_string(),
                frag_idx_code: "0.0".to_string(),
                frag_review: Some("checked".to_string()),
                cst_code: None,
                cst_vagga: None,
                cst_sutta: None,
                cst_paranum: None,
                sc_code: Some("sn1.1".to_string()),
                sc_sutta: None,
            },
        ];

        let fragments2 = vec![
            XmlFragment {
                nikaya: "samyutta".to_string(),
                frag_type: FragmentType::Sutta,
                content_xml: "<p>File 2 checked</p>".to_string(),
                start_line: 1,
                end_line: 10,
                start_char: 0,
                end_char: 50,
                group_levels: vec![],
                cst_file: "file2.xml".to_string(),
                frag_idx_code: "5.0".to_string(),
                frag_review: Some("checked".to_string()),
                cst_code: None,
                cst_vagga: None,
                cst_sutta: None,
                cst_paranum: None,
                sc_code: Some("sn2.1".to_string()),
                sc_sutta: None,
            },
            XmlFragment {
                nikaya: "samyutta".to_string(),
                frag_type: FragmentType::Sutta,
                content_xml: "<p>File 2 unchecked</p>".to_string(),
                start_line: 11,
                end_line: 20,
                start_char: 0,
                end_char: 60,
                group_levels: vec![],
                cst_file: "file2.xml".to_string(),
                frag_idx_code: "6.0".to_string(),
                frag_review: Some("unchecked".to_string()),  // Should not be extracted
                cst_code: None,
                cst_vagga: None,
                cst_sutta: None,
                cst_paranum: None,
                sc_code: None,
                sc_sutta: None,
            },
        ];

        // Export both files
        export_fragments_to_db(&fragments1, &structure, db_path).unwrap();
        export_fragments_to_db(&fragments2, &structure, db_path).unwrap();

        // Extract all checked overrides
        let (overrides, _inserted_fragments) = super::extract_all_correction_overrides(db_path).unwrap();

        // Should have 2 overrides (one from each file, not the unchecked one)
        assert_eq!(overrides.len(), 2, "Should extract 2 checked overrides from all files");

        // Verify file1 override
        let key1 = crate::types::FragmentKey {
            cst_file: "file1.xml".to_string(),
            frag_idx_code: "0.0".to_string(),
        };
        assert!(overrides.get(&key1).is_some(), "Should have override from file1");
        assert_eq!(overrides.get(&key1).unwrap().sc_code, Some("sn1.1".to_string()));

        // Verify file2 override
        let key2 = crate::types::FragmentKey {
            cst_file: "file2.xml".to_string(),
            frag_idx_code: "5.0".to_string(),
        };
        assert!(overrides.get(&key2).is_some(), "Should have override from file2");
        assert_eq!(overrides.get(&key2).unwrap().sc_code, Some("sn2.1".to_string()));
    }

    #[test]
    fn test_validate_first_last_headers_success() {
        // Valid case: first and last are Headers
        let fragments = vec![
            XmlFragment {
                nikaya: "digha".to_string(),
                frag_type: FragmentType::Header,
                content_xml: "<header>Start</header>".to_string(),
                start_line: 1,
                end_line: 1,
                start_char: 0,
                end_char: 20,
                group_levels: vec![],
                cst_file: "test.xml".to_string(),
                frag_idx_code: "0.0".to_string(),
                frag_review: None,
                cst_code: None,
                cst_vagga: None,
                cst_sutta: None,
                cst_paranum: None,
                sc_code: None,
                sc_sutta: None,
            },
            XmlFragment {
                nikaya: "digha".to_string(),
                frag_type: FragmentType::Sutta,
                content_xml: "<p>Content</p>".to_string(),
                start_line: 2,
                end_line: 2,
                start_char: 0,
                end_char: 15,
                group_levels: vec![],
                cst_file: "test.xml".to_string(),
                frag_idx_code: "1.0".to_string(),
                frag_review: None,
                cst_code: Some("dn1.1".to_string()),
                cst_vagga: None,
                cst_sutta: None,
                cst_paranum: None,
                sc_code: Some("dn1".to_string()),
                sc_sutta: None,
            },
            XmlFragment {
                nikaya: "digha".to_string(),
                frag_type: FragmentType::Header,
                content_xml: "<header>End</header>".to_string(),
                start_line: 3,
                end_line: 3,
                start_char: 0,
                end_char: 18,
                group_levels: vec![],
                cst_file: "test.xml".to_string(),
                frag_idx_code: "2.0".to_string(),
                frag_review: None,
                cst_code: None,
                cst_vagga: None,
                cst_sutta: None,
                cst_paranum: None,
                sc_code: None,
                sc_sutta: None,
            },
        ];

        let result = validate_first_last_headers(&fragments, "test.xml");
        assert!(result.is_ok(), "Should pass validation when first and last are Headers");
    }

    #[test]
    fn test_validate_first_last_headers_first_not_header() {
        // Invalid case: first is not a Header
        let fragments = vec![
            XmlFragment {
                nikaya: "digha".to_string(),
                frag_type: FragmentType::Sutta,
                content_xml: "<p>Content</p>".to_string(),
                start_line: 1,
                end_line: 1,
                start_char: 0,
                end_char: 15,
                group_levels: vec![],
                cst_file: "test.xml".to_string(),
                frag_idx_code: "0.0".to_string(),
                frag_review: None,
                cst_code: Some("dn1.1".to_string()),
                cst_vagga: None,
                cst_sutta: None,
                cst_paranum: None,
                sc_code: Some("dn1".to_string()),
                sc_sutta: None,
            },
            XmlFragment {
                nikaya: "digha".to_string(),
                frag_type: FragmentType::Header,
                content_xml: "<header>End</header>".to_string(),
                start_line: 2,
                end_line: 2,
                start_char: 0,
                end_char: 18,
                group_levels: vec![],
                cst_file: "test.xml".to_string(),
                frag_idx_code: "1.0".to_string(),
                frag_review: None,
                cst_code: None,
                cst_vagga: None,
                cst_sutta: None,
                cst_paranum: None,
                sc_code: None,
                sc_sutta: None,
            },
        ];

        let result = validate_first_last_headers(&fragments, "test.xml");
        assert!(result.is_err(), "Should fail validation when first is not Header");

        let err = result.unwrap_err();
        let err_msg = err.to_string();
        assert!(err_msg.contains("first fragment"), "Error should mention first fragment");
        assert!(err_msg.contains("Sutta"), "Error should mention actual type");
    }

    #[test]
    fn test_validate_first_last_headers_last_not_header() {
        // Invalid case: last is not a Header
        let fragments = vec![
            XmlFragment {
                nikaya: "digha".to_string(),
                frag_type: FragmentType::Header,
                content_xml: "<header>Start</header>".to_string(),
                start_line: 1,
                end_line: 1,
                start_char: 0,
                end_char: 20,
                group_levels: vec![],
                cst_file: "test.xml".to_string(),
                frag_idx_code: "0.0".to_string(),
                frag_review: None,
                cst_code: None,
                cst_vagga: None,
                cst_sutta: None,
                cst_paranum: None,
                sc_code: None,
                sc_sutta: None,
            },
            XmlFragment {
                nikaya: "digha".to_string(),
                frag_type: FragmentType::Sutta,
                content_xml: "<p>Content</p>".to_string(),
                start_line: 2,
                end_line: 2,
                start_char: 0,
                end_char: 15,
                group_levels: vec![],
                cst_file: "test.xml".to_string(),
                frag_idx_code: "1.0".to_string(),
                frag_review: None,
                cst_code: Some("dn1.1".to_string()),
                cst_vagga: None,
                cst_sutta: None,
                cst_paranum: None,
                sc_code: Some("dn1".to_string()),
                sc_sutta: None,
            },
        ];

        let result = validate_first_last_headers(&fragments, "test.xml");
        assert!(result.is_err(), "Should fail validation when last is not Header");

        let err = result.unwrap_err();
        let err_msg = err.to_string();
        assert!(err_msg.contains("last fragment"), "Error should mention last fragment");
        assert!(err_msg.contains("Sutta"), "Error should mention actual type");
    }

    #[test]
    fn test_validate_first_last_headers_empty() {
        // Invalid case: no fragments
        let fragments = vec![];

        let result = validate_first_last_headers(&fragments, "test.xml");
        assert!(result.is_err(), "Should fail validation when no fragments");

        let err = result.unwrap_err();
        let err_msg = err.to_string();
        assert!(err_msg.contains("no fragments found"), "Error should mention no fragments");
    }
}
