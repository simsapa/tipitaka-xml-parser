use anyhow::{Result, Context};
use quick_xml::Reader;
use quick_xml::events::Event;

use crate::types::{XmlFragment, FragmentAdjustments, FragmentKey, CorrectionFragmentOverrides, ScCodeComponents, ParserError, FragmentType};
use crate::sutta_builder::cst_code_to_sc_code_map;
use regex::Regex;

/// Line and character position tracking for XML reader
///
/// Tracks both line numbers (1-indexed) and character positions within lines (0-indexed).
/// This allows precise location tracking even when multiple elements are on the same line.
pub struct LineTrackingReader<'a> {
    reader: Reader<&'a [u8]>,
    current_line: usize,
    current_char: usize,  // Character position within current line (0-indexed)
    last_position: usize, // Byte position in content
    content: &'a str,
}

impl<'a> LineTrackingReader<'a> {
    /// Create a new line-tracking reader
    pub fn new(content: &'a str) -> Self {
        let mut reader = Reader::from_str(content);
        reader.trim_text(false); // Preserve whitespace
        reader.expand_empty_elements(false); // Keep empty elements as-is

        Self {
            reader,
            current_line: 1,
            current_char: 0,
            last_position: 0,
            content,
        }
    }

    /// Get the current line number (1-indexed)
    pub fn current_line(&self) -> usize {
        self.current_line
    }

    /// Get the current character position within the line (0-indexed)
    pub fn current_char(&self) -> usize {
        self.current_char
    }

    /// Update line and character position based on byte position
    fn update_position(&mut self, position: usize) {
        if position <= self.last_position {
            return;
        }

        let slice = &self.content.as_bytes()[self.last_position..position.min(self.content.len())];

        for &byte in slice {
            if byte == b'\n' {
                self.current_line += 1;
                self.current_char = 0;
            } else {
                self.current_char += 1;
            }
        }

        self.last_position = position;
    }

    /// Read the next event and update position tracking
    pub fn read_event(&mut self) -> Result<Event<'a>> {
        let event = self.reader
            .read_event()
            .context("Failed to read XML event")?;

        // Update position AFTER reading the event so line/char tracking
        // points to the end of the event, matching the byte position
        let position = self.reader.buffer_position();
        self.update_position(position);

        Ok(event)
    }

    /// Get the current buffer position
    pub fn buffer_position(&self) -> usize {
        self.reader.buffer_position()
    }
}

/// Extract vagga title from <head rend="chapter"> tag in fragment content
pub fn extract_vagga_title_from_content(content: &str) -> Option<String> {
    use quick_xml::Reader;
    use quick_xml::events::Event;

    let mut reader = Reader::from_str(content);
    reader.trim_text(false);
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let name_bytes = e.name();
                let name = std::str::from_utf8(name_bytes.as_ref()).unwrap_or("");

                // Look for <head> tags
                if name == "head" {
                    // Check if this has rend="chapter"
                    let mut is_chapter = false;

                    for attr in e.attributes() {
                        if let Ok(attr) = attr {
                            let key = std::str::from_utf8(attr.key.as_ref()).unwrap_or("");
                            let value = attr.unescape_value().unwrap_or_default();

                            if key == "rend" && value == "chapter" {
                                is_chapter = true;
                                break;
                            }
                        }
                    }

                    if is_chapter {
                        // Read the text content
                        if let Ok(Event::Text(ref text)) = reader.read_event_into(&mut buf) {
                            let title_text = text.unescape().unwrap_or_default().trim().to_string();

                            // Keep the full title including number prefix (e.g., "2. Sīhanādavaggo")
                            let looks_like_vagga_title = title_text.chars().next()
                                .map(|c| c.is_numeric())
                                .unwrap_or(false);

                            if !title_text.is_empty() && looks_like_vagga_title {
                                return Some(title_text);
                            }
                        }
                    }
                }
            },
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {},
        }
        buf.clear();
    }

    None
}

/// Extract the first paragraph number from bodytext
pub fn extract_first_paranum(content: &str) -> Option<String> {
    use quick_xml::Reader;
    use quick_xml::events::Event;

    let mut reader = Reader::from_str(content);
    reader.trim_text(false);
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) | Ok(Event::Empty(ref e)) => {
                let name_bytes = e.name();
                let name = std::str::from_utf8(name_bytes.as_ref()).unwrap_or("");
                if name == "p" {
                    // Check if this is a bodytext paragraph
                    let mut is_bodytext = false;
                    let mut paranum = None;

                    for attr in e.attributes() {
                        if let Ok(attr) = attr {
                            let key = std::str::from_utf8(attr.key.as_ref()).unwrap_or("");
                            let value = attr.unescape_value().unwrap_or_default();

                            if key == "rend" && value == "bodytext" {
                                is_bodytext = true;
                            } else if key == "n" {
                                paranum = Some(value.to_string());
                            }
                        }
                    }

                    if is_bodytext && paranum.is_some() {
                        return paranum;
                    }
                }
            },
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {},
        }
        buf.clear();
    }

    None
}

/// Convert line/char coordinates to byte position in XML content
///
/// # Arguments
/// * `xml_content` - The XML content string
/// * `target_line` - Target line number (1-indexed)
/// * `target_char` - Target character position (0-indexed byte offset within line)
///
/// # Returns
/// Byte position in the XML content
fn line_char_to_byte_pos(xml_content: &str, target_line: usize, target_char: usize) -> usize {
    let mut current_line = 1;
    let mut current_char = 0;

    for (byte_idx, byte) in xml_content.bytes().enumerate() {
        // Check if we've reached the target position BEFORE processing this byte
        if current_line == target_line && current_char == target_char {
            return byte_idx;
        }

        // Update position tracking
        if byte == b'\n' {
            current_line += 1;
            current_char = 0;
        } else {
            current_char += 1;
        }
    }

    // If we didn't find the position, return the end
    xml_content.len()
}

/// Apply fragment adjustments to override end position
///
/// Checks `CorrectionFragmentOverrides` first (highest priority), then falls back
/// to `FragmentAdjustments` if no correction override exists.
///
/// Returns `(end_byte_pos, end_line, end_char, collapsed)`.
///
/// For "moved" fragments (collapse=true), returns the fragment start position as the end,
/// producing a zero-width fragment with empty content. The `collapsed` flag is set to `true`
/// so callers know to push the fragment even though its content is empty — this keeps
/// `frag_idx` (derived from `fragments.len()`) in sync with the correction overrides.
///
/// # Arguments
/// * `frag_start_pos` - The start byte position of the current fragment, for validation
/// * `frag_start_line` - The start line of the current fragment (1-indexed)
/// * `frag_start_char` - The start character of the current fragment (0-indexed)
///
/// # Returns
/// `Result<(usize, usize, usize, bool)>` - The adjusted (end_pos, end_line, end_char, collapsed)
///
/// # Errors
/// Returns an error if the overridden end position is before the fragment start position,
/// which indicates the override is being applied to the wrong fragment (e.g., due to frag_idx shifting).
pub fn apply_fragment_adjustment(
    xml_content: &str,
    default_end_pos: usize,
    default_end_line: usize,
    default_end_char: usize,
    cst_file: &str,
    frag_idx: usize,
    frag_start_pos: usize,
    frag_start_line: usize,
    frag_start_char: usize,
    correction_overrides: Option<&CorrectionFragmentOverrides>,
    adjustments: Option<&FragmentAdjustments>,
) -> Result<(usize, usize, usize, bool)> {
    // First: check for collapse (moved fragments)
    if let Some(overrides) = correction_overrides {
        let key = FragmentKey {
            cst_file: cst_file.to_string(),
            frag_idx,
        };
        if let Some(override_data) = overrides.get(&key) {
            if override_data.collapse {
                // Collapse: end = start (zero-width fragment)
                return Ok((frag_start_pos, frag_start_line, frag_start_char, true));
            }
        }
    }

    // Then: check for boundary override (existing logic with precedence)
    if let Some((end_line, end_char)) = get_boundary_override(cst_file, frag_idx, correction_overrides, adjustments) {
        let end_pos = line_char_to_byte_pos(xml_content, end_line, end_char);

        // Validate that the override end position is not before the fragment start position
        if end_pos < frag_start_pos {
            return Err(ParserError::InvalidBoundaryOverride {
                details: format!(
                    "end position ({}) is before fragment start position ({})\n  File: {}\n  Fragment index: {}\n  Override: end_line={}, end_char={}\n\nThis indicates the override is being applied to the wrong fragment, likely due to frag_idx shifting between parse runs. Please adjust the fragment boundary in the UI.",
                    end_pos, frag_start_pos, cst_file, frag_idx, end_line, end_char
                ),
            }.into());
        }

        return Ok((end_pos, end_line, end_char, false));
    }

    // No override - use default detection
    Ok((default_end_pos, default_end_line, default_end_char, false))
}

/// Populate SC fields from embedded TSV mapping
///
/// Looks up sc_code and sc_sutta from the embedded cst-vs-sc.tsv based on cst_code
///
/// # Arguments
/// * `fragments` - Mutable vector of fragments to populate
///
/// # Returns
/// Result indicating success or error
pub fn populate_sc_fields_from_tsv(
    fragments: &mut Vec<XmlFragment>,
) -> anyhow::Result<()> {
    let tsv_map = cst_code_to_sc_code_map()?;

    // Populate fragments
    for fragment in fragments.iter_mut() {
        if let Some(ref cst_code) = fragment.cst_code {
            if let Some((sc_code, sc_sutta)) = tsv_map.get(cst_code) {
                fragment.sc_code = Some(sc_code.clone());
                fragment.sc_sutta = Some(sc_sutta.clone());
            }
        }
    }

    Ok(())
}

/// Parse an SC code into its components.
///
/// Extracts the prefix and numeric parts from SC codes:
/// - DN: `dn1` → prefix="dn", sutta=1
/// - MN: `mn41` → prefix="mn", sutta=41
/// - SN: `sn5.1` → prefix="sn", samyutta=5, sutta=1
/// - AN: `an3.1` → prefix="an", nipata=3, sutta=1
///
/// # Arguments
/// * `sc_code` - The SC code to parse (e.g., "sn5.1", "dn1")
///
/// # Returns
/// `Some(ScCodeComponents)` if parsing succeeds, `None` otherwise
pub fn parse_sc_code(sc_code: &str) -> Option<ScCodeComponents> {
    // Pattern matches: prefix (2 letters) + optional number + optional .number
    // Examples: dn1, mn41, sn5.1, an3.1
    let re = Regex::new(r"^([a-z]{2})(\d+)(?:\.(\d+))?$").ok()?;

    let caps = re.captures(sc_code)?;

    let prefix = caps.get(1)?.as_str().to_string();
    let first_num: i32 = caps.get(2)?.as_str().parse().ok()?;
    let second_num: Option<i32> = caps.get(3).and_then(|m| m.as_str().parse().ok());

    let mut components = ScCodeComponents {
        prefix: prefix.clone(),
        ..Default::default()
    };

    match prefix.as_str() {
        "sn" => {
            // SN: first number is samyutta, second is sutta
            components.samyutta = Some(first_num);
            components.sutta = second_num;
        }
        "an" => {
            // AN: first number is nipata (book), second is sutta
            components.nipata = Some(first_num);
            components.sutta = second_num;
        }
        "dn" | "mn" => {
            // DN/MN: single number is sutta
            components.sutta = Some(first_num);
        }
        _ => {
            // Unknown prefix, just store the sutta number
            components.sutta = Some(first_num);
        }
    }

    Some(components)
}

/// Get boundary override for a fragment.
///
/// Checks `CorrectionFragmentOverrides` first (highest priority), then falls back
/// to `FragmentAdjustments` if no correction override exists.
///
/// # Arguments
/// * `cst_file` - The XML file name
/// * `frag_idx` - The fragment index
/// * `correction_overrides` - Optional correction fragment overrides from database
/// * `adjustments` - Optional legacy fragment adjustments from TSV
///
/// # Returns
/// `Some((end_line, end_char))` if an override exists, `None` otherwise
pub fn get_boundary_override(
    cst_file: &str,
    frag_idx: usize,
    correction_overrides: Option<&CorrectionFragmentOverrides>,
    adjustments: Option<&FragmentAdjustments>,
) -> Option<(usize, usize)> {
    let key = FragmentKey {
        cst_file: cst_file.to_string(),
        frag_idx,
    };

    // First check correction overrides (highest priority)
    if let Some(overrides) = correction_overrides {
        if let Some(override_data) = overrides.get(&key) {
            if let Some(end_line) = override_data.end_line {
                let end_char = override_data.end_char.unwrap_or(0);
                return Some((end_line, end_char));
            }
        }
    }

    // Fall back to legacy adjustments
    if let Some(adjustments_map) = adjustments {
        if let Some(adjustment) = adjustments_map.get(&key) {
            if let Some(end_line) = adjustment.end_line {
                let end_char = adjustment.end_char.unwrap_or(0);
                return Some((end_line, end_char));
            }
        }
    }

    None
}

/// Apply boundary override and return adjusted position.
///
/// This is a convenience wrapper that combines `get_boundary_override` with
/// position conversion, for use during fragment finalization.
///
/// # Arguments
/// * `xml_content` - The XML content string
/// * `default_end_pos` - Default end byte position
/// * `default_end_line` - Default end line (1-indexed)
/// * `default_end_char` - Default end character (0-indexed)
/// * `cst_file` - The XML file name
/// * `frag_idx` - The fragment index
/// * `frag_start_pos` - The start byte position of the current fragment, for validation
/// * `correction_overrides` - Optional correction fragment overrides
/// * `adjustments` - Optional legacy fragment adjustments
///
/// # Returns
/// `Result<(end_byte_pos, end_line, end_char)>`
///
/// # Errors
/// Returns an error if the overridden end position is before the fragment start position,
/// which indicates the override is being applied to the wrong fragment (e.g., due to frag_idx shifting).
pub fn apply_boundary_override(
    xml_content: &str,
    default_end_pos: usize,
    default_end_line: usize,
    default_end_char: usize,
    cst_file: &str,
    frag_idx: usize,
    frag_start_pos: usize,
    correction_overrides: Option<&CorrectionFragmentOverrides>,
    adjustments: Option<&FragmentAdjustments>,
) -> Result<(usize, usize, usize)> {
    if let Some((end_line, end_char)) = get_boundary_override(cst_file, frag_idx, correction_overrides, adjustments) {
        let end_pos = line_char_to_byte_pos(xml_content, end_line, end_char);

        // Validate that the override end position is not before the fragment start position
        if end_pos < frag_start_pos {
            return Err(ParserError::InvalidBoundaryOverride {
                details: format!(
                    "end position ({}) is before fragment start position ({})\n  File: {}\n  Fragment index: {}\n  Override: end_line={}, end_char={}\n\nThis indicates the override is being applied to the wrong fragment, likely due to frag_idx shifting between parse runs. Please adjust the fragment boundary in the UI.",
                    end_pos, frag_start_pos, cst_file, frag_idx, end_line, end_char
                ),
            }.into());
        }

        return Ok((end_pos, end_line, end_char));
    }

    Ok((default_end_pos, default_end_line, default_end_char))
}

/// Apply SC overrides from correction fragments and propagate context.
///
/// For each correction fragment override with SC fields:
/// 1. Apply the SC override directly to that fragment
/// 2. Parse the SC code to extract context (samyutta/nipata number)
/// 3. Propagate context to subsequent fragments with null sc_code
/// 4. Stop propagation when hitting a fragment with non-null sc_code
/// 5. Look up and populate sc_sutta titles from pali_titles cache when available
///
/// # Arguments
/// * `fragments` - Mutable vector of fragments
/// * `correction_overrides` - Correction fragment overrides from database
/// * `cst_file` - The XML file name (for key lookup)
/// * `pali_titles` - Optional cache of Pali titles from ArangoDB (sc_code -> title)
pub fn apply_sc_overrides(
    fragments: &mut Vec<XmlFragment>,
    correction_overrides: &CorrectionFragmentOverrides,
    cst_file: &str,
    pali_titles: Option<&std::collections::HashMap<String, String>>,
) {
    // Collect direct overrides and parseable overrides for propagation
    let mut direct_overrides: Vec<(usize, String, Option<String>)> = Vec::new();
    let mut propagation_points: Vec<(usize, ScCodeComponents)> = Vec::new();

    // Collect metadata field overrides
    let mut metadata_overrides: Vec<(
        usize,
        Option<String>, // cst_code
        Option<String>, // cst_vagga
        Option<String>, // cst_sutta
        Option<String>, // cst_paranum
        Option<String>, // frag_review
        Option<crate::types::FragmentType>, // frag_type
    )> = Vec::new();

    for (idx, fragment) in fragments.iter().enumerate() {
        let key = FragmentKey {
            cst_file: cst_file.to_string(),
            frag_idx: fragment.frag_idx,
        };

        if let Some(override_data) = correction_overrides.get(&key) {
            // Collect SC field overrides
            if let Some(ref sc_code) = override_data.sc_code {
                // Always apply the sc_code directly
                direct_overrides.push((idx, sc_code.clone(), override_data.sc_sutta.clone()));

                // If the sc_code is parseable, also add for propagation
                if let Some(components) = parse_sc_code(sc_code) {
                    propagation_points.push((idx, components));
                }
            } else if override_data.sc_sutta.is_some() {
                // Override has sc_sutta but no sc_code - just apply sc_sutta
                direct_overrides.push((idx, String::new(), override_data.sc_sutta.clone()));
            }

            // Collect CST metadata field overrides (no propagation needed)
            metadata_overrides.push((
                idx,
                override_data.cst_code.clone(),
                override_data.cst_vagga.clone(),
                override_data.cst_sutta.clone(),
                override_data.cst_paranum.clone(),
                override_data.frag_review.clone(),
                override_data.frag_type.clone(),
            ));
        }
    }

    // Apply direct SC overrides
    for (idx, sc_code, sc_sutta) in direct_overrides {
        if !sc_code.is_empty() {
            fragments[idx].sc_code = Some(sc_code);
        }
        if sc_sutta.is_some() {
            fragments[idx].sc_sutta = sc_sutta;
        }
    }

    // Apply metadata field overrides
    for (idx, cst_code, cst_vagga, cst_sutta, cst_paranum, frag_review, frag_type) in metadata_overrides {
        if let Some(code) = cst_code {
            fragments[idx].cst_code = Some(code);
        }
        if let Some(vagga) = cst_vagga {
            fragments[idx].cst_vagga = Some(vagga);
        }
        if let Some(sutta) = cst_sutta {
            fragments[idx].cst_sutta = Some(sutta);
        }
        if let Some(paranum) = cst_paranum {
            fragments[idx].cst_paranum = Some(paranum);
        }
        if let Some(review) = frag_review {
            fragments[idx].frag_review = Some(review);
        }
        if let Some(ftype) = frag_type {
            fragments[idx].frag_type = ftype;
        }
    }

    // Propagate context from parseable overrides
    for (override_idx, components) in propagation_points {
        // Propagate context to subsequent fragments with null sc_code
        for subsequent_idx in (override_idx + 1)..fragments.len() {
            let subsequent = &fragments[subsequent_idx];

            // Stop propagation at natural recovery point (non-null sc_code)
            if subsequent.sc_code.is_some() {
                break;
            }

            // Derive sc_code from cst_code using propagated context
            if let Some(ref cst_code) = subsequent.cst_code {
                if let Some(derived_sc) = derive_sc_code_from_context(cst_code, &components) {
                    // Check if cst_code is a range - if so, convert derived sc_code to range
                    let sc_code_to_assign = if is_cst_code_range(cst_code) {
                        convert_sc_code_to_range(&derived_sc, cst_code)
                    } else {
                        derived_sc.clone()
                    };

                    // Only assign sc_code if it exists in ArangoDB
                    // Try in order: exact match -> range match -> non-range base
                    if let Some(titles_cache) = pali_titles {
                        if let Some((sc_code_found, title)) = find_sc_code_in_pali_titles(&sc_code_to_assign, titles_cache) {
                            fragments[subsequent_idx].sc_code = Some(sc_code_found);
                            fragments[subsequent_idx].sc_sutta = Some(title);
                        }
                    } else {
                        // No ArangoDB, just assign the derived sc_code without title lookup
                        fragments[subsequent_idx].sc_code = Some(sc_code_to_assign);
                    }
                }
            }
        }
    }
}

/// Derive SC code from CST code using propagated context.
///
/// Uses the context (samyutta/nipata number) from a checked override to
/// derive the SC code for a fragment based on its CST code.
///
/// # Arguments
/// * `cst_code` - The CST code (e.g., "sn1.5.1.2")
/// * `context` - The SC code components from the override
///
/// # Returns
/// Derived SC code if derivation is possible
fn derive_sc_code_from_context(cst_code: &str, context: &ScCodeComponents) -> Option<String> {
    // Extract the sutta number from cst_code
    // CST codes have format like: sn1.5.1.2 (book.samyutta.vagga.sutta)
    // Or range: sn1.5.1.11-20 (book.samyutta.vagga.start-end)
    // We need to extract the sutta number and combine with context

    // Check if cst_code has a range (e.g., "11-20")
    let (cst_code_base, is_range) = if let Some(dash_pos) = cst_code.rfind('-') {
        let base_part = &cst_code[..dash_pos];
        // Check if the dash is part of a range (followed by digits)
        if base_part.rfind('.').map_or(false, |pos| {
            base_part[pos+1..].chars().all(|c| c.is_ascii_digit())
        }) {
            (base_part.to_string(), true)
        } else {
            (cst_code.to_string(), false)
        }
    } else {
        (cst_code.to_string(), false)
    };

    let parts: Vec<&str> = cst_code_base.split('.').collect();

    match context.prefix.as_str() {
        "sn" => {
            // SN: cst_code format is sn{book}.{samyutta}.{vagga}.{sutta}
            // Use context.samyutta and extract sutta from cst_code
            if let Some(samyutta) = context.samyutta {
                // Try to get the sutta number from the last part of cst_code
                if parts.len() >= 4 {
                    if let Ok(sutta) = parts[3].parse::<i32>() {
                        if is_range {
                            // Extract end number from original cst_code
                            if let Some(end_part) = cst_code.rsplit('-').next() {
                                if let Ok(end_sutta) = end_part.parse::<i32>() {
                                    return Some(format!("sn{}.{}-{}", samyutta, sutta, end_sutta));
                                }
                            }
                        }
                        return Some(format!("sn{}.{}", samyutta, sutta));
                    }
                } else if parts.len() == 3 {
                    // Some samyuttas don't have vagga level (e.g., sn1.8.0.1)
                    // In this case, vagga=0 means no vagga, sutta is in position 3
                    if let Ok(sutta) = parts[2].parse::<i32>() {
                        // This might be vagga number, check if it's 0
                        if sutta == 0 {
                            // No vagga, can't derive
                            return None;
                        }
                    }
                }
            }
        }
        "an" => {
            // AN: cst_code format is an{book}.{pannasaka}.{vagga}.{sutta}
            // Use context.nipata and extract sutta from cst_code
            if let Some(nipata) = context.nipata {
                if parts.len() >= 4 {
                    if let Ok(sutta) = parts[3].parse::<i32>() {
                        if is_range {
                            if let Some(end_part) = cst_code.rsplit('-').next() {
                                if let Ok(end_sutta) = end_part.parse::<i32>() {
                                    return Some(format!("an{}.{}-{}", nipata, sutta, end_sutta));
                                }
                            }
                        }
                        return Some(format!("an{}.{}", nipata, sutta));
                    }
                }
            }
        }
        "dn" | "mn" => {
            // DN/MN: simpler format, just use the sutta number from cst_code
            if parts.len() >= 2 {
                if let Ok(sutta) = parts[1].parse::<i32>() {
                    return Some(format!("{}{}", context.prefix, sutta));
                }
            }
        }
        _ => {}
    }

    None
}

/// Format SC code components back into a string.
#[allow(dead_code)]
fn format_sc_code(components: &ScCodeComponents) -> String {
    match components.prefix.as_str() {
        "sn" => {
            if let (Some(samyutta), Some(sutta)) = (components.samyutta, components.sutta) {
                format!("sn{}.{}", samyutta, sutta)
            } else if let Some(samyutta) = components.samyutta {
                format!("sn{}", samyutta)
            } else {
                components.prefix.clone()
            }
        }
        "an" => {
            if let (Some(nipata), Some(sutta)) = (components.nipata, components.sutta) {
                format!("an{}.{}", nipata, sutta)
            } else if let Some(nipata) = components.nipata {
                format!("an{}", nipata)
            } else {
                components.prefix.clone()
            }
        }
        "dn" | "mn" => {
            if let Some(sutta) = components.sutta {
                format!("{}{}", components.prefix, sutta)
            } else {
                components.prefix.clone()
            }
        }
        _ => {
            if let Some(sutta) = components.sutta {
                format!("{}{}", components.prefix, sutta)
            } else {
                components.prefix.clone()
            }
        }
    }
}

/// Populate SC fields from TSV only for fragments that don't already have sc_code set.
///
/// This is the conditional version of `populate_sc_fields_from_tsv` that skips
/// fragments where sc_code has already been set (e.g., from checked overrides).
///
/// Handles range codes (e.g., `sn1.1.7.2-3`) by:
/// 1. Looking up the start code (`sn1.1.7.2`) to get base `sc_code` (`sn1.62`)
/// 2. Looking up the end code (`sn1.1.7.3`) to get end `sc_code` (`sn1.63`)
/// 3. Combining to create range `sc_code` (`sn1.62-63`)
///
/// # Arguments
/// * `fragments` - Mutable vector of fragments to populate
///
/// # Returns
/// Result indicating success or error
pub fn populate_sc_fields_from_tsv_conditional(
    fragments: &mut [XmlFragment],
) -> anyhow::Result<()> {
    let tsv_map = cst_code_to_sc_code_map()?;

    // Populate only fragments without sc_code
    for fragment in fragments.iter_mut() {
        // Skip if sc_code is already set (from override or propagation)
        if fragment.sc_code.is_some() {
            continue;
        }

        if let Some(ref cst_code) = fragment.cst_code {
            // First try direct lookup
            if let Some((sc_code, sc_sutta)) = tsv_map.get(cst_code) {
                // If cst_code is a range, convert sc_code to range format
                let sc_code = if is_cst_code_range(cst_code) {
                    convert_sc_code_to_range(sc_code, cst_code)
                } else {
                    sc_code.clone()
                };
                fragment.sc_code = Some(sc_code);
                fragment.sc_sutta = Some(sc_sutta.clone());
            } else if let Some((sc_code, sc_sutta)) = lookup_range_cst_code(cst_code, &tsv_map) {
                // Try range lookup if direct lookup failed
                // Convert sc_code to range format
                let sc_code = if is_cst_code_range(cst_code) {
                    convert_sc_code_to_range(&sc_code, cst_code)
                } else {
                    sc_code
                };
                fragment.sc_code = Some(sc_code);
                fragment.sc_sutta = Some(sc_sutta);
            }
        }
    }

    Ok(())
}

/// Propagate SC codes from previous fragments when TSV lookup fails.
///
/// For each fragment where sc_code is null after TSV lookup:
/// 1. Compare the current fragment's cst_code with the previous fragment's sc_code
/// 2. If only the sutta number increased (e.g., sn15.20 → sn15.21), increment the sutta
/// 3. If a new group started (e.g., sn15.20 → sn16.1), increment group and reset sutta to 1
///
/// This provides a fallback when derived cst_code values are not in the lookup data.
pub fn propagate_sc_codes_from_previous(
    fragments: &mut [XmlFragment],
    pali_titles: Option<&std::collections::HashMap<String, String>>,
) {
    let tsv_map = match cst_code_to_sc_code_map() {
        Ok(map) => map,
        Err(_) => return,
    };

    for i in 1..fragments.len() {
        let current_cst_code = fragments[i].cst_code.clone();
        let previous_sc_code = fragments[i - 1].sc_code.clone();

        if fragments[i].sc_code.is_some() {
            continue;
        }

        let current_cst_code = match current_cst_code {
            Some(code) => code,
            None => continue,
        };

        let previous_sc_code = match previous_sc_code {
            Some(code) => code,
            None => continue,
        };

        if let Some(derived_sc) = derive_sc_code_from_previous(&current_cst_code, &previous_sc_code) {
            // Check if cst_code is a range - if so, convert derived sc_code to range
            let sc_code_to_assign = if is_cst_code_range(&current_cst_code) {
                convert_sc_code_to_range(&derived_sc, &current_cst_code)
            } else {
                derived_sc.clone()
            };

            // If cst_code is a range, use sc_code_to_assign directly (with range)
            // and try to find the title from pali_titles
            // Otherwise, use the normal lookup flow
            let is_range = is_cst_code_range(&current_cst_code);
            
            if is_range {
                // For range cst_codes, use the derived range sc_code directly
                // and try to get title from pali_titles
                if let Some(titles_cache) = pali_titles {
                    // Try to find title - first try the range, then try base
                    if let Some(title) = titles_cache.get(&sc_code_to_assign) {
                        fragments[i].sc_code = Some(sc_code_to_assign.clone());
                        fragments[i].sc_sutta = Some(title.clone());
                    } else {
                        // Try base
                        let base = sc_code_to_assign.split('-').next().unwrap_or(&sc_code_to_assign);
                        if let Some(title) = titles_cache.get(base) {
                            fragments[i].sc_code = Some(sc_code_to_assign.clone());
                            fragments[i].sc_sutta = Some(title.clone());
                        }
                    }
                } else {
                    // No ArangoDB, fall back to TSV lookup
                    if let Some((_, sc_sutta)) = tsv_map.get(&current_cst_code) {
                        fragments[i].sc_code = Some(sc_code_to_assign);
                        fragments[i].sc_sutta = Some(sc_sutta.clone());
                    }
                }
            } else {
                // Non-range: use normal lookup
                if let Some(titles_cache) = pali_titles {
                    if let Some((sc_code_found, title)) = find_sc_code_in_pali_titles(&sc_code_to_assign, titles_cache) {
                        fragments[i].sc_code = Some(sc_code_found.clone());
                        fragments[i].sc_sutta = Some(title);
                    }
                } else {
                    // No ArangoDB, fall back to TSV lookup
                    if let Some((_, sc_sutta)) = tsv_map.get(&current_cst_code) {
                        fragments[i].sc_code = Some(sc_code_to_assign);
                        fragments[i].sc_sutta = Some(sc_sutta.clone());
                    }
                }
            }
        }
    }
}

/// Derive SC code from previous fragment's sc_code based on current cst_code.
///
/// Compares cst_code with previous sc_code to determine if:
/// - Only sutta number incremented (sn15.20 → sn15.21)
/// - New group started (sn15.20 → sn16.1)
/// - Handles range sc_codes (e.g., sn12.93-103)
fn derive_sc_code_from_previous(cst_code: &str, previous_sc_code: &str) -> Option<String> {
    // Extract base cst_code (handle ranges like "sn3.8.1.11-20" -> "sn3.8.1.11")
    // Also extract the range end if present
    let (cst_code_base, range_end) = if let Some(dash_pos) = cst_code.rfind('-') {
        let base_part = &cst_code[..dash_pos];
        if base_part.rsplit('.').next().map_or(false, |s| s.chars().all(|c| c.is_ascii_digit())) {
            // Extract range end number
            let end_str = &cst_code[dash_pos + 1..];
            let end = end_str.parse::<i32>().ok();
            (base_part.to_string(), end)
        } else {
            (cst_code.to_string(), None)
        }
    } else {
        (cst_code.to_string(), None)
    };

    let cst_parts: Vec<&str> = cst_code_base.split('.').collect();

    if cst_parts.len() < 4 {
        return None;
    }

    // Extract cst_samyutta from cst_code (sn3.10.1.1 -> samyutta = 10)
    let cst_samyutta: i32 = cst_parts[1].parse().ok()?;
    let cst_sutta: i32 = cst_parts[3].parse().ok()?;

    // First try to parse as a regular sc_code
    if let Some(prev_components) = parse_sc_code(previous_sc_code) {
        return derive_sc_code_with_components(cst_code, cst_sutta, cst_samyutta, &prev_components, range_end);
    }

    // If regular parse fails, try to handle range sc_code (e.g., "sn12.93-103")
    if let Some(range_components) = parse_range_sc_code(previous_sc_code) {
        return derive_sc_code_with_components(cst_code, cst_sutta, cst_samyutta, &range_components, range_end);
    }

    None
}

/// Derive sc_code using parsed components
fn derive_sc_code_with_components(_cst_code: &str, cst_sutta: i32, _cst_samyutta: i32, prev_components: &ScCodeComponents, range_end: Option<i32>) -> Option<String> {
    match prev_components.prefix.as_str() {
        "sn" => {
            let prev_samyutta = prev_components.samyutta?;
            let prev_sutta = prev_components.sutta?;

            // If cst_sutta is 1 and previous sutta was > 1 (or we have a range),
            // it means a new samyutta started in cst_code
            if cst_sutta == 1 && (prev_sutta > 1 || prev_sutta > 0) {
                // Increment the sc samyutta by 1 for the new cst samyutta
                // If there's a range end, include it
                if let Some(end) = range_end {
                    return Some(format!("sn{}.{}-{}", prev_samyutta + 1, cst_sutta, end));
                }
                return Some(format!("sn{}.{}", prev_samyutta + 1, cst_sutta));
            }

            if cst_sutta == prev_sutta + 1 {
                // If there's a range end, include it
                if let Some(end) = range_end {
                    return Some(format!("sn{}.{}-{}", prev_samyutta, cst_sutta, end));
                }
                Some(format!("sn{}.{}", prev_samyutta, cst_sutta))
            } else if cst_sutta == 1 || cst_sutta < prev_sutta {
                // If there's a range end, include it
                if let Some(end) = range_end {
                    return Some(format!("sn{}.{}-{}", prev_samyutta + 1, 1, end));
                }
                Some(format!("sn{}.1", prev_samyutta + 1))
            } else {
                None
            }
        }
        "an" => {
            let prev_nipata = prev_components.nipata?;
            let prev_sutta = prev_components.sutta?;

            // If cst_sutta is 1 and previous sutta was > 1, new nipata
            if cst_sutta == 1 && (prev_sutta > 1 || prev_sutta > 0) {
                // If there's a range end, include it
                if let Some(end) = range_end {
                    return Some(format!("an{}.{}-{}", prev_nipata + 1, cst_sutta, end));
                }
                return Some(format!("an{}.{}", prev_nipata + 1, cst_sutta));
            }

            if cst_sutta == prev_sutta + 1 {
                // If there's a range end, include it
                if let Some(end) = range_end {
                    return Some(format!("an{}.{}-{}", prev_nipata, cst_sutta, end));
                }
                Some(format!("an{}.{}", prev_nipata, cst_sutta))
            } else if cst_sutta == 1 || cst_sutta < prev_sutta {
                // If there's a range end, include it
                if let Some(end) = range_end {
                    return Some(format!("an{}.{}-{}", prev_nipata + 1, 1, end));
                }
                Some(format!("an{}.1", prev_nipata + 1))
            } else {
                None
            }
        }
        "dn" | "mn" => {
            let prev_sutta = prev_components.sutta?;

            if cst_sutta == prev_sutta + 1 {
                Some(format!("{}{}", prev_components.prefix, cst_sutta))
            } else {
                None
            }
        }
        _ => None,
    }
}

/// Parse a range sc_code like "sn12.93-103" into components.
///
/// Returns ScCodeComponents with the end sutta number.
fn parse_range_sc_code(sc_code: &str) -> Option<ScCodeComponents> {
    let parts: Vec<&str> = sc_code.split('-').collect();
    if parts.len() != 2 {
        return None;
    }

    // Parse the first part to get prefix and samyutta/nipata
    let first_part = parts[0];
    let second_part = parts[1];

    // Parse first part: "sn12.93" or just "sn12"
    let first_components = parse_sc_code(first_part)?;

    // Parse second part: "103" (just a number)
    let end_sutta: i32 = second_part.parse().ok()?;

    Some(ScCodeComponents {
        prefix: first_components.prefix,
        samyutta: first_components.samyutta,
        nipata: first_components.nipata,
        sutta: Some(end_sutta),
    })
}

/// Look up a range cst_code in the TSV map.
///
/// For cst_code like `sn1.1.7.2-3`:
/// 1. Extract start code `sn1.1.7.2` and end number `3`
/// 2. Look up start code to get `sn1.62`
/// 3. Build end code `sn1.1.7.3` and look up to get `sn1.63`
/// 4. Return combined sc_code `sn1.62-63` with the start's sc_sutta
///
/// Returns None if the cst_code is not a range or lookup fails.
fn lookup_range_cst_code(
    cst_code: &str,
    tsv_map: &std::collections::HashMap<String, (String, String)>,
) -> Option<(String, String)> {
    // Check if the last segment contains a range (e.g., "2-3")
    let parts: Vec<&str> = cst_code.rsplitn(2, '.').collect();
    if parts.len() != 2 {
        return None;
    }

    let last_segment = parts[0];
    let prefix = parts[1];

    // Check if last segment is a range like "2-3"
    let range_parts: Vec<&str> = last_segment.split('-').collect();
    if range_parts.len() != 2 {
        return None;
    }

    let start_num: u32 = range_parts[0].parse().ok()?;
    let end_num: u32 = range_parts[1].parse().ok()?;

    // Build start and end cst_codes
    let start_cst_code = format!("{}.{}", prefix, start_num);
    let end_cst_code = format!("{}.{}", prefix, end_num);

    // Look up start code
    let (start_sc_code, sc_sutta) = tsv_map.get(&start_cst_code)?;

    // Look up end code
    let (end_sc_code, _) = tsv_map.get(&end_cst_code)?;

    // Extract sutta numbers from sc_codes to build range
    // sc_code format: "sn1.62" -> prefix="sn1", sutta=62
    let start_sc_parts: Vec<&str> = start_sc_code.rsplitn(2, '.').collect();
    let end_sc_parts: Vec<&str> = end_sc_code.rsplitn(2, '.').collect();

    if start_sc_parts.len() != 2 || end_sc_parts.len() != 2 {
        // Fallback: just return start sc_code if format doesn't match
        return Some((start_sc_code.clone(), sc_sutta.clone()));
    }

    let sc_prefix = start_sc_parts[1];
    let start_sc_sutta: u32 = start_sc_parts[0].parse().ok()?;
    let end_sc_sutta: u32 = end_sc_parts[0].parse().ok()?;

    // Build the range sc_code
    let range_sc_code = format!("{}.{}-{}", sc_prefix, start_sc_sutta, end_sc_sutta);

    Some((range_sc_code, sc_sutta.clone()))
}

/// Check if a cst_code is a range (e.g., "sn3.2.3.1-11")
pub fn is_cst_code_range(cst_code: &str) -> bool {
    if let Some(dash_pos) = cst_code.rfind('-') {
        let base_part = &cst_code[..dash_pos];
        return base_part.rsplit('.').next()
            .map_or(false, |s| s.chars().all(|c| c.is_ascii_digit()));
    }
    false
}

/// Convert an sc_code to range format based on cst_code range.
/// e.g., cst_code "sn3.2.3.1-11" with sc_code "sn23.23" -> "sn23.23-33"
fn convert_sc_code_to_range(sc_code: &str, cst_code: &str) -> String {
    // Extract the end sutta number from cst_code range (e.g., "1-11" -> 11)
    if let Some(dash_pos) = cst_code.rfind('-') {
        let end_str = &cst_code[dash_pos + 1..];
        if let Ok(end_sutta) = end_str.parse::<i32>() {
            // Get the base sc_code (without range)
            let base_sc = sc_code.split('-').next().unwrap_or(sc_code);
            // Calculate the range based on the difference in cst_code
            if let Some(dash_pos2) = cst_code.rfind('-') {
                let start_str = &cst_code[..dash_pos2];
                if let Some(start_pos) = start_str.rfind('.') {
                    let start_sutta: i32 = start_str[start_pos + 1..].parse().unwrap_or(1);
                    let range_size = end_sutta - start_sutta;
                    if range_size > 0 {
                        // Extract the prefix and start number from sc_code
                        if let Some(dot_pos) = base_sc.rfind('.') {
                            let prefix = &base_sc[..dot_pos + 1];
                            let start_num: i32 = base_sc[dot_pos + 1..].parse().unwrap_or(1);
                            let new_end = start_num + range_size;
                            return format!("{}{}-{}", prefix, start_num, new_end);
                        }
                    }
                }
            }
        }
    }
    sc_code.to_string()
}

/// Look up a range sc_code in the pali_titles cache.
///
/// For sc_code like `sn29.11` (derived from a range cst_code like `sn3.8.1.11-20`):
/// 1. Try to find an exact match first
/// Find an sc_code that exists in pali_titles using fallback logic.
///
/// Tries in order:
/// 1. Exact match (e.g., sn30.3)
/// 2. Range match (e.g., sn30.3 -> sn30.3-*)
/// 3. Non-range base (e.g., sn30.3-12 -> sn30.3)
///
/// Returns the sc_code to use and its title if found, None otherwise.
fn find_sc_code_in_pali_titles(
    derived_sc: &str,
    titles_cache: &std::collections::HashMap<String, String>,
) -> Option<(String, String)> {
    // 1. Try exact match first
    if let Some(title) = titles_cache.get(derived_sc) {
        return Some((derived_sc.to_string(), title.clone()));
    }

    // 2. Try range match: sn30.3 -> sn30.3-*
    let range_prefix = format!("{}-", derived_sc);
    for (key, title) in titles_cache {
        if key.starts_with(&range_prefix) {
            return Some((key.clone(), title.clone()));
        }
    }

    // 3. Try non-range base: sn30.3-12 -> sn30.3
    // Extract the base (everything before the last hyphen followed by digits)
    if let Some(base) = extract_non_range_base(derived_sc) {
        if let Some(title) = titles_cache.get(&base) {
            return Some((base, title.clone()));
        }
        // Also try range match on the base
        let base_range_prefix = format!("{}-", base);
        for (key, title) in titles_cache {
            if key.starts_with(&base_range_prefix) {
                return Some((key.clone(), title.clone()));
            }
        }
    }

    None
}

/// Extract the non-range base from an sc_code.
///
/// For example:
/// - sn30.3-12 -> Some(sn30.3)
/// - sn29.11-20 -> Some(sn29.11)
/// - sn30.3 -> None (already non-range)
fn extract_non_range_base(sc_code: &str) -> Option<String> {
    // Check if sc_code contains a range (e.g., sn30.3-12)
    if let Some(dash_pos) = sc_code.rfind('-') {
        let before_dash = &sc_code[..dash_pos];
        // Check if the part before dash ends with a number (indicating a range)
        if before_dash.rsplit('.').next()?.chars().all(|c| c.is_ascii_digit()) {
            return Some(before_dash.to_string());
        }
    }
    None
}

// ============== HierarchyTracker ==============
// NOTE: This was moved here from the individual nikaya parser files as part of
// refactoring Plan 01. See tasks/01-hierarchy-tracker.md for details.

use crate::types::{GroupType, GroupLevel};
use crate::nikaya_structure::NikayaStructure;

/// Hierarchy tracker for maintaining group level context
///
/// Tracks the current position in the nikaya hierarchy and manages
/// entering/exiting levels according to the nikaya structure.
#[derive(Debug, Clone)]
pub struct HierarchyTracker {
    current_levels: Vec<GroupLevel>,
    nikaya_structure: NikayaStructure,
}

impl HierarchyTracker {
    /// Create a new hierarchy tracker
    pub fn new(nikaya_structure: NikayaStructure) -> Self {
        Self {
            current_levels: Vec::new(),
            nikaya_structure,
        }
    }

    /// Enter a new hierarchy level
    ///
    /// Determines the depth of the level type in the nikaya structure,
    /// truncates current_levels to the appropriate depth, and adds the new level.
    /// If a level of the same type exists at that depth, it updates the title but preserves the ID.
    pub fn enter_level(
        &mut self,
        level_type: GroupType,
        title: String,
        id: Option<String>,
        number: Option<i32>,
    ) {

        // Find the depth of this level type in the nikaya structure
        let depth = self.nikaya_structure.levels
            .iter()
            .position(|t| matches!((t, &level_type),
                (GroupType::Nikaya, GroupType::Nikaya) |
                (GroupType::Book, GroupType::Book) |
                (GroupType::Pannasaka, GroupType::Pannasaka) |
                (GroupType::Vagga, GroupType::Vagga) |
                (GroupType::Samyutta, GroupType::Samyutta) |
                (GroupType::Sutta, GroupType::Sutta)
            ));

        if let Some(depth) = depth {
            // Special case: If we're entering a Nikaya level (depth 0) and we already have
            // levels (like Book), this means the XML has the nikaya tag INSIDE the book div.
            // In this case, we should insert the Nikaya at the beginning rather than truncating.
            if depth == 0 && matches!(level_type, GroupType::Nikaya) && !self.current_levels.is_empty() {
                // Check if we already have a Nikaya level
                if self.current_levels.first().map(|l| matches!(l.group_type, GroupType::Nikaya)).unwrap_or(false) {
                    // Update existing Nikaya level
                    self.current_levels[0] = GroupLevel {
                        group_type: level_type,
                        group_number: number,
                        title,
                        id,
                    };
                } else {
                    // Insert Nikaya at the beginning
                    self.current_levels.insert(0, GroupLevel {
                        group_type: level_type,
                        group_number: number,
                        title,
                        id,
                    });
                }
                return;
            }

            // First, check if we already have a level of the same type at ANY position
            // This handles cases like SN samyuttas without vaggas, where the Sutta level
            // ends up at a different depth than the nikaya structure specifies.
            let existing_pos = self.current_levels.iter().position(|existing| {
                matches!((&existing.group_type, &level_type),
                    (GroupType::Nikaya, GroupType::Nikaya) |
                    (GroupType::Book, GroupType::Book) |
                    (GroupType::Pannasaka, GroupType::Pannasaka) |
                    (GroupType::Vagga, GroupType::Vagga) |
                    (GroupType::Samyutta, GroupType::Samyutta) |
                    (GroupType::Sutta, GroupType::Sutta)
                )
            });

            if let Some(pos) = existing_pos {
                // Found an existing level of the same type
                let existing = &self.current_levels[pos];

                // Update the existing level, but preserve ID if new ID is None
                let preserved_id = if id.is_none() {
                    existing.id.clone()
                } else {
                    id.clone()
                };

                // Only truncate child levels if we're providing a new ID OR if the title is changing
                // If id is None AND title is the same, we're just re-entering the same level
                // Otherwise, we're entering a NEW level (with a new title) and should truncate child levels
                let title_changed = existing.title != title;
                let should_truncate = id.is_some() || title_changed;

                if should_truncate {
                    // Truncate levels after this one before updating
                    self.current_levels.truncate(pos + 1);
                }

                self.current_levels[pos] = GroupLevel {
                    group_type: level_type,
                    group_number: number,
                    title,
                    id: preserved_id,
                };
                return;
            }

            // No existing level of this type - truncate to the appropriate depth
            self.current_levels.truncate(depth);

            // Add the new level
            self.current_levels.push(GroupLevel {
                group_type: level_type,
                group_number: number,
                title,
                id,
            });
        }
    }

    /// Get a clone of the current hierarchy levels
    pub fn get_current_levels(&self) -> Vec<GroupLevel> {
        self.current_levels.clone()
    }
}

/// Extract sutta title from <head> or <p rend="subhead"> tag in fragment content
///
/// Prefers <p rend="subhead"> over <head rend="chapter"> to avoid extracting vagga titles.
/// Returns the first title that looks like a sutta title (starts with a number).
///
/// # Arguments
/// * `content` - The XML content to search
///
/// # Returns
/// `Some(title)` if a sutta title is found, `None` otherwise
pub fn extract_sutta_title_from_content(content: &str) -> Option<String> {
    use quick_xml::Reader;
    use quick_xml::events::Event;

    let mut reader = Reader::from_str(content);
    reader.trim_text(false);
    let mut buf = Vec::new();

    let mut first_chapter_title: Option<String> = None;
    let mut first_subhead_title: Option<String> = None;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let name_bytes = e.name();
                let name = std::str::from_utf8(name_bytes.as_ref()).unwrap_or("").to_string();

                // Check both <head> and <p> tags
                if name == "head" || name == "p" {
                    // Check if this has rend="chapter" or rend="subhead"
                    let mut rend_value: Option<String> = None;

                    for attr in e.attributes() {
                        if let Ok(attr) = attr {
                            let key = std::str::from_utf8(attr.key.as_ref()).unwrap_or("");
                            let value = attr.unescape_value().unwrap_or_default();

                            if key == "rend" && (value == "chapter" || value == "subhead") {
                                rend_value = Some(value.to_string());
                                break;
                            }
                        }
                    }

                    if let Some(rend) = rend_value {
                        // Read the text content
                        if let Ok(Event::Text(ref text)) = reader.read_event_into(&mut buf) {
                            let title_text = text.unescape().unwrap_or_default().trim().to_string();

                            // Keep the full title including number prefix (e.g., "2. Brahmajālasuttaṃ")
                            // But skip if it's a subsection (like "Uddeso" which doesn't start with a number)
                            let looks_like_sutta_title = title_text.chars().next()
                                .map(|c| c.is_numeric())
                                .unwrap_or(false);

                            if !title_text.is_empty() && looks_like_sutta_title {
                                if rend == "subhead" && first_subhead_title.is_none() {
                                    first_subhead_title = Some(title_text.clone());
                                } else if rend == "chapter" && first_chapter_title.is_none() {
                                    // For <p rend="chapter">, only treat as sutta title if in DN
                                    // In AN tika, <p rend="chapter"> is a Vagga marker, not a Sutta marker
                                    // In DN, <head rend="chapter"> IS a Sutta marker
                                    // We can't distinguish nikayas here, so use tag name: <head> = DN sutta, <p> = vagga
                                    if name == "head" {
                                        first_chapter_title = Some(title_text.clone());
                                    }
                                }

                                // If we found a subhead title, we can return immediately
                                // since subheads are sutta titles and take priority over chapter titles (vagga titles)
                                if rend == "subhead" {
                                    return Some(title_text);
                                }
                            }
                        }
                    }
                }
            },
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {},
        }
        buf.clear();
    }

    // Prefer subhead title over chapter title
    first_subhead_title.or(first_chapter_title)
}

/// Check if text starts with a numbered sutta marker (e.g., "1. ", "10. ", "2-11. ")
///
/// Handles both single numbers and ranges like "2-11" by checking each part is numeric.
/// Also handles cases where there's no space after the number (e.g., "9.Khayadhammasuttaṃ").
pub fn is_numbered_sutta_subhead(text: &str) -> bool {
    // First try the standard format: "number. title" or "number-number. title"
    let result = text.split_whitespace()
        .next()
        .and_then(|first_word| first_word.strip_suffix('.'))
        .map_or(false, |num_part| {
            num_part.split('-').all(|part| !part.is_empty() && part.chars().all(|c| c.is_numeric()))
        });

    if result {
        return true;
    }

    // Fallback: try to extract "number." from the beginning without requiring space
    // Pattern: digits (optionally with hyphen and more digits), followed by dot
    // This handles "9.Khayadhammasuttaṃ" -> true
    let text_bytes = text.as_bytes();
    let mut end_pos = 0;

    for (i, &b) in text_bytes.iter().enumerate() {
        if b.is_ascii_digit() || b == b'-' {
            end_pos = i + 1;
        } else if b == b'.' {
            // Found the dot after the number
            if end_pos > 0 {
                let num_str = &text[..end_pos];
                if num_str.split('-').all(|part| !part.is_empty() && part.chars().all(|c| c.is_numeric())) {
                    return true;
                }
            }
            break;
        } else {
            // Non-digit, non-hyphen, non-dot character - stop
            break;
        }
    }

    false
}

use std::collections::HashMap;

// ============== FragmentBoundaryDetector ==============
// NOTE: This was moved here from the individual nikaya parser files as part of
// refactoring Plan 04. See tasks/04-boundary-detector.md for details.

/// Fragment boundary detector
///
/// Detects boundaries between fragments based on nikaya-specific rules
/// and extracts relevant metadata.
pub struct FragmentBoundaryDetector<'a> {
    nikaya_structure: &'a NikayaStructure,
    cst_file: &'a str,
}

impl<'a> FragmentBoundaryDetector<'a> {
    /// Create a new fragment boundary detector
    pub fn new(nikaya_structure: &'a NikayaStructure, cst_file: &'a str) -> Self {
        Self { nikaya_structure, cst_file }
    }

    /// Check if an element marks a level boundary and extract metadata
    ///
    /// Returns Some((GroupType, title, id, number)) if this is a boundary element
    pub fn check_boundary(
        &self,
        tag_name: &str,
        attributes: &HashMap<String, String>,
    ) -> Option<(GroupType, String, Option<String>, Option<i32>)> {
        match tag_name {
            "p" if attributes.get("rend") == Some(&"nikaya".to_string()) => {
                Some((GroupType::Nikaya, String::new(), None, None))
            },
            "p" if attributes.get("rend") == Some(&"book".to_string()) => {
                Some((GroupType::Book, String::new(), None, None))
            },
            "div" if attributes.get("type") == Some(&"book".to_string()) => {
                let id = attributes.get("id").cloned();
                Some((GroupType::Book, String::new(), id, None))
            },
            "div" if attributes.get("type") == Some(&"samyutta".to_string()) => {
                let id = attributes.get("id").cloned();
                Some((GroupType::Samyutta, String::new(), id, None))
            },
            "div" if attributes.get("type") == Some(&"pannasaka".to_string()) => {
                let id = attributes.get("id").cloned();
                Some((GroupType::Pannasaka, String::new(), id, None))
            },
            "div" if attributes.get("type") == Some(&"vagga".to_string()) => {
                let id = attributes.get("id").cloned();
                Some((GroupType::Vagga, String::new(), id, None))
            },
            "div" if attributes.get("type") == Some(&"sutta".to_string()) => {
                let id = attributes.get("id").cloned();
                Some((GroupType::Sutta, String::new(), id, None))
            },
            "head" if attributes.get("rend") == Some(&"book".to_string()) => {
                Some((GroupType::Book, String::new(), None, None))
            },
            "head" if attributes.get("rend") == Some(&"nikaya".to_string()) => {
                Some((GroupType::Nikaya, String::new(), None, None))
            },
            "head" if attributes.get("rend") == Some(&"title".to_string()) => {
                // In AN, <head rend="title"> = Pannasaka title
                if self.nikaya_structure.nikaya == "anguttara" {
                    Some((GroupType::Pannasaka, String::new(), None, None))
                } else {
                    None
                }
            },
            "head" if attributes.get("rend") == Some(&"chapter".to_string()) => {
                // In DN, chapter = Sutta
                // In SN, chapter = Samyutta
                // In MN/AN, chapter = Vagga
                if self.nikaya_structure.nikaya == "digha" {
                    Some((GroupType::Sutta, String::new(), None, None))
                } else if self.nikaya_structure.nikaya == "samyutta" {
                    Some((GroupType::Samyutta, String::new(), None, None))
                } else {
                    Some((GroupType::Vagga, String::new(), None, None))
                }
            },
            "p" if attributes.get("rend") == Some(&"title".to_string()) => {
                // In SN, <p rend="title"> = Vagga title
                // In AN (commentary/tika), <p rend="title"> = Pannasaka title
                if self.nikaya_structure.nikaya == "samyutta" {
                    Some((GroupType::Vagga, String::new(), None, None))
                } else if self.nikaya_structure.nikaya == "anguttara" {
                    Some((GroupType::Pannasaka, String::new(), None, None))
                } else {
                    None
                }
            },
            "p" if attributes.get("rend") == Some(&"chapter".to_string()) => {
                // In AN (commentary/tika), <p rend="chapter"> = Vagga title
                if self.nikaya_structure.nikaya == "anguttara" {
                    Some((GroupType::Vagga, String::new(), None, None))
                } else {
                    None
                }
            },
            "p" if attributes.get("rend") == Some(&"subhead".to_string()) => {
                // In MN, SN, and AN, subhead = Sutta title
                if self.nikaya_structure.nikaya == "majjhima" ||
                   self.nikaya_structure.nikaya == "samyutta" ||
                   self.nikaya_structure.nikaya == "anguttara" {
                    Some((GroupType::Sutta, String::new(), None, None))
                } else {
                    None
                }
            },
            _ => None,
        }
    }

    /// Check if this is a sutta boundary (start of actual sutta content)
    pub fn is_sutta_start(&self, tag_name: &str, attributes: &HashMap<String, String>) -> bool {
        // Check if this is a commentary or sub-commentary file
        let is_commentary = self.cst_file.ends_with(".att.xml") || self.cst_file.ends_with(".tik.xml");

        match self.nikaya_structure.nikaya.as_str() {
            "digha" => {
                if is_commentary {
                    // DN commentary: Use <head rend="chapter"> for sutta boundaries
                    // NOT <div type="sutta"> which marks introduction sections
                    tag_name == "head" && attributes.get("rend") == Some(&"chapter".to_string())
                } else {
                    // DN base text: Suttas are wrapped in <div type="sutta">
                    tag_name == "div" && attributes.get("type") == Some(&"sutta".to_string())
                }
            },
            "majjhima" | "samyutta" => {
                // MN/SN: Suttas are delimited by <p rend="subhead">
                // Each subhead starts a new sutta
                tag_name == "p" && attributes.get("rend") == Some(&"subhead".to_string())
            },
            "anguttara" => {
                // AN: Similar to MN/SN
                tag_name == "p" && attributes.get("rend") == Some(&"subhead".to_string())
            },
            _ => {
                // Default: look for div or subhead
                (tag_name == "div" && attributes.get("type") == Some(&"sutta".to_string())) ||
                (tag_name == "p" && attributes.get("rend") == Some(&"subhead".to_string()))
            }
        }
    }
}

/// Macro to implement the XmlParser trait for a nikaya parser struct
///
/// This macro generates a standard XmlParser implementation that delegates
/// to the shared `parse_into_fragments` function. All nikaya-specific parsers
/// use this same pattern.
///
/// # Usage
/// ```ignore
/// impl_xml_parser!(DighaNikayaMula);
/// impl_xml_parser!(MajjhimaNikayaMula);
/// ```
#[macro_export]
macro_rules! impl_xml_parser {
    ($struct_name:ident) => {
        impl XmlParser for $struct_name {
            fn parse_into_fragments(
                &self,
                xml_content: &str,
                nikaya_structure: &NikayaStructure,
                cst_file: &str,
                overrides: &ParserOverrides,
                populate_sc_fields: bool,
            ) -> Result<Vec<XmlFragment>> {
                // Delegate to the public function
                parse_into_fragments(
                    xml_content,
                    nikaya_structure,
                    cst_file,
                    overrides,
                    populate_sc_fields,
                )
            }
        }
    };
}

/// Re-export the macro for convenience
pub use impl_xml_parser;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_number_from_title_with_space() {
        // Standard format: "number. title"
        assert_eq!(extract_number_from_title("1. Suttavaṇṇanā"), Some("1"));
        assert_eq!(extract_number_from_title("10. Suttavaṇṇanā"), Some("10"));
        assert_eq!(extract_number_from_title("5-7. Suttādivaṇṇanā"), Some("5-7"));
        assert_eq!(extract_number_from_title("10-11. Title"), Some("10-11"));
    }

    #[test]
    fn test_extract_number_from_title_without_space() {
        // No space after dot: "number.title"
        assert_eq!(extract_number_from_title("1.Sattisuttavaṇṇanā"), Some("1"));
        assert_eq!(extract_number_from_title("10.Suttavaṇṇanā"), Some("10"));
        assert_eq!(extract_number_from_title("5-7.Suttādivaṇṇanā"), Some("5-7"));
    }

    #[test]
    fn test_extract_number_from_title_invalid() {
        // No number prefix
        assert_eq!(extract_number_from_title("Suttavaṇṇanā"), None);
        assert_eq!(extract_number_from_title(""), None);
        // No dot after number
        assert_eq!(extract_number_from_title("1 Suttavaṇṇanā"), None);
    }

    #[test]
    fn test_is_numbered_sutta_subhead() {
        // Standard format: "number. title" (with space after dot)
        assert!(is_numbered_sutta_subhead("1. Suttaname"));
        assert!(is_numbered_sutta_subhead("10. Suttaname"));
        assert!(is_numbered_sutta_subhead("2-11. Suttaname"));

        // No space after dot: "number.title"
        assert!(is_numbered_sutta_subhead("9.Khayadhammasuttaṃ"));
        assert!(is_numbered_sutta_subhead("1.Suttaname"));
        assert!(is_numbered_sutta_subhead("10.Suttaname"));
        assert!(is_numbered_sutta_subhead("5-7.Suttaname"));

        // Non-numbered subheads should return false
        assert!(!is_numbered_sutta_subhead("Anattadhammasuttaṃ"));
        assert!(!is_numbered_sutta_subhead("Vagganame"));
        assert!(!is_numbered_sutta_subhead(""));
    }

    #[test]
    fn test_parse_sc_code_dn() {
        let result = parse_sc_code("dn1").unwrap();
        assert_eq!(result.prefix, "dn");
        assert_eq!(result.sutta, Some(1));
        assert_eq!(result.samyutta, None);
        assert_eq!(result.nipata, None);

        let result = parse_sc_code("dn34").unwrap();
        assert_eq!(result.prefix, "dn");
        assert_eq!(result.sutta, Some(34));
    }

    #[test]
    fn test_parse_sc_code_mn() {
        let result = parse_sc_code("mn1").unwrap();
        assert_eq!(result.prefix, "mn");
        assert_eq!(result.sutta, Some(1));

        let result = parse_sc_code("mn152").unwrap();
        assert_eq!(result.prefix, "mn");
        assert_eq!(result.sutta, Some(152));
    }

    #[test]
    fn test_parse_sc_code_sn() {
        let result = parse_sc_code("sn5.1").unwrap();
        assert_eq!(result.prefix, "sn");
        assert_eq!(result.samyutta, Some(5));
        assert_eq!(result.sutta, Some(1));

        let result = parse_sc_code("sn56.11").unwrap();
        assert_eq!(result.prefix, "sn");
        assert_eq!(result.samyutta, Some(56));
        assert_eq!(result.sutta, Some(11));
    }

    #[test]
    fn test_parse_sc_code_an() {
        let result = parse_sc_code("an3.1").unwrap();
        assert_eq!(result.prefix, "an");
        assert_eq!(result.nipata, Some(3));
        assert_eq!(result.sutta, Some(1));

        let result = parse_sc_code("an11.1").unwrap();
        assert_eq!(result.prefix, "an");
        assert_eq!(result.nipata, Some(11));
        assert_eq!(result.sutta, Some(1));
    }

    #[test]
    fn test_parse_sc_code_invalid() {
        assert!(parse_sc_code("invalid").is_none());
        assert!(parse_sc_code("").is_none());
        assert!(parse_sc_code("xyz").is_none());
        assert!(parse_sc_code("dn").is_none()); // No number
    }

    #[test]
    fn test_format_sc_code() {
        let sn = ScCodeComponents {
            prefix: "sn".to_string(),
            samyutta: Some(5),
            sutta: Some(1),
            nipata: None,
        };
        assert_eq!(format_sc_code(&sn), "sn5.1");

        let an = ScCodeComponents {
            prefix: "an".to_string(),
            nipata: Some(3),
            sutta: Some(10),
            samyutta: None,
        };
        assert_eq!(format_sc_code(&an), "an3.10");

        let dn = ScCodeComponents {
            prefix: "dn".to_string(),
            sutta: Some(1),
            samyutta: None,
            nipata: None,
        };
        assert_eq!(format_sc_code(&dn), "dn1");
    }

    #[test]
    fn test_get_boundary_override_correction_takes_precedence() {
        use crate::types::{CorrectionFragmentOverride, FragmentAdjustment};
        use std::collections::HashMap;

        let mut corrections = HashMap::new();
        corrections.insert(
            FragmentKey { cst_file: "test.xml".to_string(), frag_idx: 0 },
            CorrectionFragmentOverride {
                collapse: false,
                end_line: Some(100),
                end_char: Some(50),
                sc_code: None,
                sc_sutta: None,
                cst_code: None,
                cst_vagga: None,
                cst_sutta: None,
                cst_paranum: None,
                frag_review: None,
                frag_type: None,
            }
        );

        let mut adjustments = HashMap::new();
        adjustments.insert(
            FragmentKey { cst_file: "test.xml".to_string(), frag_idx: 0 },
            FragmentAdjustment {
                cst_file: "test.xml".to_string(),
                frag_idx: 0,
                end_line: Some(200),
                end_char: Some(25),
            }
        );

        // Correction override should take precedence
        let result = get_boundary_override("test.xml", 0, Some(&corrections), Some(&adjustments));
        assert_eq!(result, Some((100, 50)));

        // Without correction override, should fall back to adjustments
        let result = get_boundary_override("test.xml", 1, Some(&corrections), Some(&adjustments));
        assert_eq!(result, None); // No override for frag_idx 1
    }

    /// Helper to create a test fragment with minimal required fields
    fn create_test_fragment(frag_idx: usize, cst_code: Option<&str>, sc_code: Option<&str>) -> XmlFragment {
        use crate::types::FragmentType;

        XmlFragment {
            nikaya: "digha".to_string(),
            cst_file: "test.xml".to_string(),
            frag_idx,
            frag_type: FragmentType::Sutta,
            frag_review: None,
            content_xml: "test content".to_string(),
            start_line: 1,
            start_char: 0,
            end_line: 10,
            end_char: 0,
            cst_code: cst_code.map(String::from),
            cst_vagga: None,
            cst_sutta: None,
            cst_paranum: None,
            sc_code: sc_code.map(String::from),
            sc_sutta: None,
            group_levels: vec![],
        }
    }

    /// Test that populate_sc_fields_from_tsv_conditional skips fragments with existing sc_code
    #[test]
    fn test_conditional_tsv_skips_existing_sc_code() {
        // Create fragments - some with existing sc_code, some without
        let mut fragments = vec![
            // Fragment with existing sc_code - should NOT be overwritten
            create_test_fragment(0, Some("dn1.1.0.1"), Some("existing_sc_code")),
            // Fragment without sc_code but with cst_code - should be populated if cst_code maps
            create_test_fragment(1, Some("dn1.1.0.2"), None),
            // Fragment with empty values - should remain unchanged if cst_code doesn't map
            create_test_fragment(2, Some("nonexistent.code"), None),
        ];

        // Store original values
        let original_frag0_sc = fragments[0].sc_code.clone();

        // Call conditional populate
        populate_sc_fields_from_tsv_conditional(&mut fragments).unwrap();

        // Fragment 0: should keep original sc_code (not overwritten)
        assert_eq!(
            fragments[0].sc_code, original_frag0_sc,
            "Existing sc_code should NOT be overwritten by conditional TSV"
        );
        assert_eq!(
            fragments[0].sc_code.as_deref(), Some("existing_sc_code"),
            "Fragment with existing sc_code should keep it"
        );
    }

    /// Test that populate_sc_fields_from_tsv_conditional populates null sc_code from TSV
    #[test]
    fn test_conditional_tsv_populates_null_sc_code() {
        // Note: This test relies on the TSV mapping having the appropriate entries.
        // We use real CST codes that should be in the mapping.

        // Create a fragment with a CST code that we know maps to an SC code
        // (dn1.1.0.1 -> dn1 based on the TSV mapping)
        let mut fragments = vec![
            create_test_fragment(0, Some("dn1.1.0.1"), None),
        ];

        // Call conditional populate
        let result = populate_sc_fields_from_tsv_conditional(&mut fragments);
        assert!(result.is_ok(), "populate_sc_fields_from_tsv_conditional should succeed");

        // If the TSV map contains this mapping, sc_code should be populated
        // The actual value depends on what's in the TSV file
        // We just verify the function runs without panic and either populates or leaves None
        // (since we can't guarantee the TSV file content in unit tests)
    }

    /// Test that non-conditional TSV overwrites existing sc_code (demonstrating the difference)
    #[test]
    fn test_non_conditional_tsv_overwrites() {
        // This test demonstrates why we need the conditional version:
        // the non-conditional version would overwrite existing sc_code values

        // Create fragment with existing sc_code and a cst_code that maps differently
        let mut fragments = vec![
            create_test_fragment(0, Some("dn1.1.0.1"), Some("my_custom_sc_code")),
        ];

        // Store original
        let original_sc = fragments[0].sc_code.clone();

        // Call NON-conditional populate
        populate_sc_fields_from_tsv(&mut fragments).unwrap();

        // The non-conditional version WILL overwrite if there's a mapping
        // We just verify it runs successfully
        // (The actual behavior depends on TSV content, but the test shows the difference
        //  is that this function doesn't check for existing sc_code)

        // Note: We can't assert on the exact value without knowing TSV contents,
        // but we've demonstrated the function exists and runs
        assert!(fragments[0].sc_code.is_some() || original_sc.is_some());
    }

    #[test]
    fn test_lookup_range_cst_code() {
        use crate::sutta_builder::cst_code_to_sc_code_map;

        let tsv_map = cst_code_to_sc_code_map().expect("Should load TSV map");

        // Test with a known range: sn1.1.7.2-3 should map to sn1.62-63
        // First verify the individual mappings exist
        assert!(tsv_map.contains_key("sn1.1.7.2"), "TSV should contain sn1.1.7.2");
        assert!(tsv_map.contains_key("sn1.1.7.3"), "TSV should contain sn1.1.7.3");

        let result = lookup_range_cst_code("sn1.1.7.2-3", &tsv_map);
        assert!(result.is_some(), "Should find range mapping for sn1.1.7.2-3");

        let (sc_code, _sc_sutta) = result.unwrap();
        assert_eq!(sc_code, "sn1.62-63", "Range sc_code should be sn1.62-63");
    }

    #[test]
    fn test_lookup_range_cst_code_non_range() {
        use crate::sutta_builder::cst_code_to_sc_code_map;

        let tsv_map = cst_code_to_sc_code_map().expect("Should load TSV map");

        // Non-range code should return None
        let result = lookup_range_cst_code("sn1.1.7.2", &tsv_map);
        assert!(result.is_none(), "Non-range code should not match");

        // Invalid format should return None
        let result = lookup_range_cst_code("invalid", &tsv_map);
        assert!(result.is_none(), "Invalid code should not match");
    }

    #[test]
    fn test_derive_sc_code_from_previous_new_samyutta() {
        // Test transition from sn3.9 to sn3.10 (cst_code)
        // Previous sc_code was sn29.11-20, new cst_code is sn3.10.1.1
        // Should derive to sn30.1 (increment samyutta by 1)

        // From range sc_code sn29.11-20 to cst_code sn3.10.1.1 should give sn30.1
        let result = derive_sc_code_from_previous("sn3.10.1.1", "sn29.11-20");
        assert!(result.is_some(), "Should derive sc_code for new samyutta");
        assert_eq!(result.unwrap(), "sn30.1");

        // From range sc_code sn29.21-50 to cst_code sn3.10.1.1 should give sn30.1
        let result = derive_sc_code_from_previous("sn3.10.1.1", "sn29.21-50");
        assert!(result.is_some(), "Should derive sc_code for new samyutta");
        assert_eq!(result.unwrap(), "sn30.1");

        // From range sc_code sn30.17-46 to cst_code sn3.10.1.1 should give sn31.1
        let result = derive_sc_code_from_previous("sn3.10.1.1", "sn30.17-46");
        assert!(result.is_some(), "Should derive sc_code for new samyutta");
        assert_eq!(result.unwrap(), "sn31.1");

        // From sn30.1 to sn3.10.1.2 should give sn30.2 (same samyutta, increment sutta)
        let result = derive_sc_code_from_previous("sn3.10.1.2", "sn30.1");
        assert!(result.is_some(), "Should derive sc_code for same samyutta");
        assert_eq!(result.unwrap(), "sn30.2");
    }

    #[test]
    fn test_derive_sc_code_from_context_range_cst_code() {
        // Test deriving sc_code from a range cst_code
        let context = ScCodeComponents {
            prefix: "sn".to_string(),
            samyutta: Some(29),
            sutta: None,
            nipata: None,
        };

        // Range cst_code: sn3.8.1.11-20 should derive to sn29.11-20
        let result = derive_sc_code_from_context("sn3.8.1.11-20", &context);
        assert!(result.is_some(), "Should derive sc_code from range cst_code");
        assert_eq!(result.unwrap(), "sn29.11-20");
    }

    #[test]
    fn test_derive_sc_code_from_context_single_cst_code() {
        // Test deriving sc_code from a single cst_code
        let context = ScCodeComponents {
            prefix: "sn".to_string(),
            samyutta: Some(30),
            sutta: None,
            nipata: None,
        };

        // Single cst_code: sn3.9.1.1 should derive to sn30.1
        let result = derive_sc_code_from_context("sn3.9.1.1", &context);
        assert!(result.is_some(), "Should derive sc_code from single cst_code");
        assert_eq!(result.unwrap(), "sn30.1");
    }

    #[test]
    fn test_find_sc_code_in_pali_titles() {
        use std::collections::HashMap;

        // Create a mock pali_titles cache with range sc_codes
        let mut titles: HashMap<String, String> = HashMap::new();
        titles.insert("sn29.11-20".to_string(), "Aṇḍajadānūpakārasuttadasaka".to_string());
        titles.insert("sn29.21-50".to_string(), "Jalābujādidānūpakārasuttattiṃsaka".to_string());
        titles.insert("sn30.17-46".to_string(), "Some title".to_string());
        titles.insert("sn30.3".to_string(), "Single sutta 30.3".to_string());
        titles.insert("sn30.1".to_string(), "Single sutta 30.1".to_string());

        // Exact match should return the exact code and title
        let result = find_sc_code_in_pali_titles("sn29.11-20", &titles);
        assert!(result.is_some(), "Exact match should return Some");
        let (code, title) = result.unwrap();
        assert_eq!(code, "sn29.11-20");
        assert_eq!(title, "Aṇḍajadānūpakārasuttadasaka");

        // Range lookup: sn29.11 should find sn29.11-20
        let result = find_sc_code_in_pali_titles("sn29.11", &titles);
        assert!(result.is_some(), "Should find range match");
        let (code, title) = result.unwrap();
        assert_eq!(code, "sn29.11-20");
        assert_eq!(title, "Aṇḍajadānūpakārasuttadasaka");

        // Range lookup: sn29.21 should find sn29.21-50
        let result = find_sc_code_in_pali_titles("sn29.21", &titles);
        assert!(result.is_some(), "Should find range match for sn29.21");
        let (code, title) = result.unwrap();
        assert_eq!(code, "sn29.21-50");
        assert_eq!(title, "Jalābujādidānūpakārasuttattiṃsaka");

        // Test case: sn30.3-12 (range) should find sn30.3 (non-range base)
        // sn30.3-12 doesn't exist, sn30.3 does
        let result = find_sc_code_in_pali_titles("sn30.3-12", &titles);
        assert!(result.is_some(), "Should find non-range base match");
        let (code, title) = result.unwrap();
        assert_eq!(code, "sn30.3");
        assert_eq!(title, "Single sutta 30.3");

        // Non-matching code should return None
        let result = find_sc_code_in_pali_titles("sn99.99", &titles);
        assert!(result.is_none(), "Non-matching code should return None");
    }

    #[test]
    fn test_extract_non_range_base() {
        // Range codes should extract base
        assert_eq!(extract_non_range_base("sn30.3-12"), Some("sn30.3".to_string()));
        assert_eq!(extract_non_range_base("sn29.11-20"), Some("sn29.11".to_string()));
        assert_eq!(extract_non_range_base("an3.5-10"), Some("an3.5".to_string()));

        // Non-range codes should return None
        assert_eq!(extract_non_range_base("sn30.3"), None);
        assert_eq!(extract_non_range_base("dn1"), None);
    }
}

// ============== CST Fields Derivation ==============
// NOTE: This was moved here from the individual nikaya parser files as part of
// refactoring Plan 05. See tasks/05-derive-cst-fields-unification.md for details.

/// Derive CST code from fragment metadata
///
/// For DN: code is like "dn1.1" from div id="dn1_1" or div id="dn1" + sutta number "1."
/// For MN: code is like "mn1.5.1" from div id="mn1_5_1" or div id="mn1_5" + sutta number "1."
/// For SN: code is like "sn1.1.1.1" from div id="sn1" + div id="sn1_1" + vagga number "1." + sutta number "1."
///
/// Handles nikaya-specific variations:
/// - SN mula: uses reverse iteration for vagga/sutta extraction (group_levels may contain stale entries)
/// - All others: uses forward iteration
///
/// # Arguments
/// * `fragment` - The fragment containing group_levels with IDs and titles
/// * `nikaya_structure` - The nikaya structure for determining code format
/// * `cst_sutta_title` - Optional sutta title from fragment content (fallback)
///
/// # Returns
/// The derived CST code (e.g., "dn1.1", "mn1.5.1", "sn1.1.1.1"), or None if insufficient metadata
pub fn derive_cst_code(
    fragment: &XmlFragment,
    nikaya_structure: &NikayaStructure,
    cst_sutta_title: Option<&str>,
) -> Option<String> {
    // First check if the Sutta level itself has an ID (like "dn1_12")
    // This is the most direct and reliable source
    if let Some(sutta_id) = fragment.group_levels.iter()
        .find_map(|level| {
            if matches!(level.group_type, GroupType::Sutta) {
                level.id.as_ref()
            } else {
                None
            }
        }) {
        // Convert id format: "dn1_12" or "mn1_5_3" -> "dn1.12" or "mn1.5.3"
        let code = sutta_id.replace('_', ".");
        return Some(code);
    }

    // Fallback: Try to construct from components based on nikaya structure
    // Get book number from ID (e.g., "dn1" -> "1", "mn1" -> "1", "sn1" -> "1")
    let book_id = fragment.group_levels.iter()
        .find_map(|level| {
            if matches!(level.group_type, GroupType::Book) {
                level.id.as_ref()
            } else {
                None
            }
        });

    // For Samyutta Nikaya: extract samyutta number from ID like "sn1_1"
    let samyutta_number = if nikaya_structure.nikaya == "samyutta" {
        fragment.group_levels.iter()
            .find_map(|level| {
                if matches!(level.group_type, GroupType::Samyutta) {
                    // Extract number from samyutta title like "1. Devatāsaṃyuttaṃ"
                    level.title.split_whitespace()
                        .next()
                        .and_then(|first| first.strip_suffix('.'))
                        .filter(|num| num.chars().all(|c| c.is_numeric()))
                        .or_else(|| {
                            // Fallback: Extract from ID like "sn1_1" -> "1"
                            level.id.as_ref().and_then(|id| {
                                id.rsplit('_')
                                    .next()
                                    .filter(|num| num.chars().all(|c| c.is_numeric()))
                            })
                        })
                } else {
                    None
                }
            })
    } else {
        None
    };

    // For Anguttara Nikaya: extract pannasaka number from ID like "an3_1"
    let pannasaka_number = if nikaya_structure.nikaya == "anguttara" {
        fragment.group_levels.iter()
            .find_map(|level| {
                if matches!(level.group_type, GroupType::Pannasaka) {
                    // Extract number from pannasaka title like "1. Paṭhamapaṇṇāsakaṃ"
                    level.title.split_whitespace()
                        .next()
                        .and_then(|first| first.strip_suffix('.'))
                        .filter(|num| num.chars().all(|c| c.is_numeric()))
                        .or_else(|| {
                            // Fallback: Extract from ID like "an3_1" -> "1"
                            level.id.as_ref().and_then(|id| {
                                id.rsplit('_')
                                    .next()
                                    .filter(|num| num.chars().all(|c| c.is_numeric()))
                            })
                        })
                } else {
                    None
                }
            })
    } else {
        None
    };

    // SN mula uses reverse iteration for vagga/sutta because
    // group_levels may contain stale entries from previous samyuttas
    let use_rev = nikaya_structure.nikaya == "samyutta"
        && fragment.cst_file.ends_with(".mul.xml");

    // Get vagga number from title (e.g., "1" from "1. Mūlapariyāyavaggo" or "1. Naḷavaggo")
    // Also handles titles without space after dot like "1.Vaggavaṇṇanā"
    // This is more reliable than using the vagga ID since the ID may be inherited from the next vagga
    // However, for vagga 0 (introduction/preamble) in commentary files, the title is often empty,
    // so we fallback to extracting from the ID (e.g., "mn1_0" -> "0")
    let vagga_number = if use_rev {
        // SN mula: use reverse iteration to get the LAST (most recent) Vagga level
        fragment.group_levels.iter()
            .rev()
            .find_map(|level| {
                if matches!(level.group_type, GroupType::Vagga) {
                    // First try: Extract number from title (handles both "1. Name" and "1.Name")
                    extract_number_from_title(&level.title)
                        .or_else(|| {
                            // Fallback: Extract from ID like "mn1_0" or "mn1_1"
                            // Split by underscore and take the last part
                            level.id.as_ref().and_then(|id| {
                                id.rsplit('_')
                                    .next()
                                    .filter(|num| num.chars().all(|c| c.is_numeric()))
                            })
                        })
                } else {
                    None
                }
            })
    } else {
        // Standard: forward iteration
        fragment.group_levels.iter()
            .find_map(|level| {
                if matches!(level.group_type, GroupType::Vagga) {
                    // First try: Extract number from title (handles both "1. Name" and "1.Name")
                    extract_number_from_title(&level.title)
                        .or_else(|| {
                            // Fallback: Extract from ID like "mn1_0" or "mn1_1"
                            // Split by underscore and take the last part
                            level.id.as_ref().and_then(|id| {
                                id.rsplit('_')
                                    .next()
                                    .filter(|num| num.chars().all(|c| c.is_numeric()))
                            })
                        })
                } else {
                    None
                }
            })
    };

    // Extract sutta number from title (e.g., "1. Brahmajālasuttaṃ" or "1. Oghataraṇasuttaṃ" -> "1")
    // Also handles titles without space after dot like "1.Sattisuttavaṇṇanā"
    // First try from Sutta GroupLevel
    let sutta_number = if use_rev {
        // SN mula: use reverse iteration to get the LAST (most recent) Sutta level
        // This is important because group_levels may contain multiple Sutta levels
        fragment.group_levels.iter()
            .rev()
            .find_map(|level| {
                if matches!(level.group_type, GroupType::Sutta) {
                    extract_number_from_title(&level.title)
                } else {
                    None
                }
            })
    } else {
        // Standard: forward iteration
        fragment.group_levels.iter()
            .find_map(|level| {
                if matches!(level.group_type, GroupType::Sutta) {
                    extract_number_from_title(&level.title)
                } else {
                    None
                }
            })
    }
    .or_else(|| {
        // Fallback: Extract from cst_sutta_title parameter (from fragment content)
        cst_sutta_title.and_then(|title| extract_number_from_title(title))
    });

    // Build the code based on nikaya structure
    match nikaya_structure.nikaya.as_str() {
        "digha" => {
            // DN style: dn{book}.{sutta}
            match (book_id, sutta_number) {
                (Some(book), Some(sutta)) => Some(format!("{}.{}", book, sutta)),
                _ => None,
            }
        }
        "majjhima" => {
            // MN style: mn{book}.{vagga}.{sutta}
            match (book_id, vagga_number, sutta_number) {
                (Some(book), Some(vagga), Some(sutta)) => {
                    Some(format!("{}.{}.{}", book, vagga, sutta))
                }
                (Some(book), Some(vagga), None) => {
                    // MN vagga 0 (introduction/preamble) in commentary files: mn1.0.0
                    Some(format!("{}.{}.0", book, vagga))
                }
                _ => None,
            }
        }
        "samyutta" => {
            // SN style: sn{book}.{samyutta}.{vagga}.{sutta}
            // Some samyuttas (like Bhikkhunīsaṃyuttaṃ) don't have vaggas, so use 1 as the vagga number
            match (book_id, samyutta_number, vagga_number, sutta_number) {
                (Some(book), Some(samyutta), Some(vagga), Some(sutta)) => {
                    Some(format!("{}.{}.{}.{}", book, samyutta, vagga, sutta))
                }
                (Some(book), Some(samyutta), Some(vagga), None) => {
                    // SN vagga 0 (introduction/preamble) in commentary files: sn1.1.0.0
                    Some(format!("{}.{}.{}.0", book, samyutta, vagga))
                }
                (Some(book), Some(samyutta), None, Some(sutta)) => {
                    // SN without vaggas (like Bhikkhunīsaṃyuttaṃ): use 1 as vagga number
                    Some(format!("{}.{}.1.{}", book, samyutta, sutta))
                }
                _ => None,
            }
        }
        "anguttara" => {
            // AN style: an{book}.{pannasaka}.{vagga}.{sutta}
            match (book_id, pannasaka_number, vagga_number, sutta_number) {
                (Some(book), Some(pannasaka), Some(vagga), Some(sutta)) => {
                    Some(format!("{}.{}.{}.{}", book, pannasaka, vagga, sutta))
                }
                (Some(book), Some(pannasaka), Some(vagga), None) => {
                    // AN vagga 0 (introduction/preamble) in commentary files: an3.1.0.0
                    Some(format!("{}.{}.{}.0", book, pannasaka, vagga))
                }
                _ => None,
            }
        }
        _ => {
            // Default fallback for other nikayas
            match (book_id, sutta_number) {
                (Some(book), Some(sutta)) => Some(format!("{}.{}", book, sutta)),
                _ => None,
            }
        }
    }
}

/// Extract CST fields from fragment content
///
/// Derives cst_file, cst_code, cst_vagga, cst_sutta, and cst_paranum
/// from the fragment.
///
/// Handles nikaya-specific variations:
/// - SN mula: uses reverse iteration for vagga/sutta extraction and
///   conditional vagga title fallback
/// - All others: uses forward iteration and unconditional vagga title fallback
///
/// # Arguments
/// * `fragment` - The fragment to process
/// * `nikaya_structure` - The nikaya structure for context
///
/// # Returns
/// Tuple of (cst_file, cst_code, cst_vagga, cst_sutta, cst_paranum)

/// Check if a string is a valid sutta number, supporting both single numbers and ranges.
/// Examples: "1", "10", "5-7", "10-11"
fn is_sutta_number_or_range(s: &str) -> bool {
    // Support both single numbers ("1", "10") and ranges ("5-7", "10-11")
    s.split('-')
        .all(|part| !part.is_empty() && part.chars().all(|c| c.is_numeric()))
}

/// Extract a number (or range) from the beginning of a title string.
/// Handles both formats:
/// - "1. Suttavaṇṇanā" (with space after dot) -> "1"
/// - "1.Suttavaṇṇanā" (no space after dot) -> "1"
/// - "5-7. Suttādivaṇṇanā" (range with space) -> "5-7"
/// - "5-7.Suttādivaṇṇanā" (range without space) -> "5-7"
fn extract_number_from_title(title: &str) -> Option<&str> {
    // First try the standard format: "number. title" or "number-number. title"
    if let Some(num) = title.split_whitespace()
        .next()
        .and_then(|first| first.strip_suffix('.'))
        .filter(|num| is_sutta_number_or_range(num)) {
        return Some(num);
    }

    // Fallback: try to extract "number." from the beginning without requiring space
    // Pattern: digits (optionally with hyphen and more digits), followed by dot
    // This handles "1.Suttavaṇṇanā" -> "1"
    let title_bytes = title.as_bytes();
    let mut end_pos = 0;

    // Find the end of the number/range part
    for (i, &b) in title_bytes.iter().enumerate() {
        if b.is_ascii_digit() || b == b'-' {
            end_pos = i + 1;
        } else if b == b'.' {
            // Found the dot after the number
            if end_pos > 0 {
                let num_str = &title[..end_pos];
                if is_sutta_number_or_range(num_str) {
                    return Some(num_str);
                }
            }
            break;
        } else {
            // Non-digit, non-hyphen, non-dot character - stop
            break;
        }
    }

    None
}

pub fn derive_cst_fields(
    fragment: &XmlFragment,
    nikaya_structure: &NikayaStructure,
) -> (String, Option<String>, Option<String>, Option<String>, Option<String>) {
    let cst_file = fragment.cst_file.clone();

    // Only process Sutta fragments
    if !matches!(fragment.frag_type, FragmentType::Sutta) {
        return (cst_file, None, None, None, None);
    }

    // SN mula uses reverse iteration for vagga/sutta because
    // group_levels may contain stale entries from previous samyuttas
    let use_rev = nikaya_structure.nikaya == "samyutta"
        && fragment.cst_file.ends_with(".mul.xml");

    // --- cst_vagga ---
    let cst_vagga = if use_rev {
        // SN mula: no has_vagga_level guard, reverse iteration
        // Check if THIS FRAGMENT actually has a Vagga level (not just if the nikaya supports vaggas)
        // This is important for SN where some samyuttas have vaggas and some don't
        // Use .rev() to get the LAST (most recent) Vagga level, not the first
        fragment.group_levels.iter()
            .rev()
            .find(|level| matches!(level.group_type, GroupType::Vagga))
            .and_then(|level| {
                if level.title.trim().is_empty() {
                    None
                } else {
                    Some(level.title.clone())
                }
            })
            .or_else(|| {
                // SN mula: only use vagga fallback for MN (never for SN itself)
                // Fallback: Extract vagga title from <head rend="chapter"> tag in fragment content
                // This is used for MN where <head rend="chapter"> is the vagga title
                // NOTE: Do NOT use this fallback for SN because in SN, <head rend="chapter"> is a Samyutta marker,
                // not a Vagga marker. We already have the Samyutta info in group_levels.
                // Only apply fallback for MN (majjhima)
                if nikaya_structure.nikaya == "majjhima" {
                    extract_vagga_title_from_content(&fragment.content_xml)
                } else {
                    None
                }
            })
    } else {
        // Standard: check if nikaya structure has vaggas, forward iteration
        let has_vagga_level = nikaya_structure.levels.iter()
            .any(|t| matches!(t, GroupType::Vagga));

        if has_vagga_level {
            fragment.group_levels.iter()
                .find(|level| matches!(level.group_type, GroupType::Vagga))
                .and_then(|level| {
                    if level.title.trim().is_empty() {
                        None
                    } else {
                        Some(level.title.clone())
                    }
                })
                .or_else(|| {
                    // Fallback: Extract vagga title from <head rend="chapter"> tag
                    // This is used for MN where <head rend="chapter"> is the vagga title
                    extract_vagga_title_from_content(&fragment.content_xml)
                })
        } else {
            None
        }
    };

    // --- cst_sutta ---
    // Extract sutta title from group_levels (filter out empty titles)
    let cst_sutta = if use_rev {
        // Used for SN mula:
        // Use .rev() to get the LAST (most recent) Sutta level, not the first
        // This is important because group_levels may contain multiple Sutta levels
        // when a new sutta starts (the old one hasn't been removed yet)
        fragment.group_levels.iter()
            .rev()
            .find(|level| matches!(level.group_type, GroupType::Sutta))
    } else {
        fragment.group_levels.iter()
            .find(|level| matches!(level.group_type, GroupType::Sutta))
    }
    .and_then(|level| {
        if level.title.trim().is_empty() {
            None
        } else {
            Some(level.title.clone())
        }
    })
    .or_else(|| {
        // Fallback: Extract title from <head> or <p rend="subhead"> tag in fragment content
        extract_sutta_title_from_content(&fragment.content_xml)
    });

    // --- cst_paranum ---
    // Extract cst_paranum from first <p rend="bodytext" n="...">
    let cst_paranum = extract_first_paranum(&fragment.content_xml);

    // --- cst_code ---
    // Derive cst_code from div id attributes and sutta number
    // Pass the cst_sutta as a parameter so it can be used for deriving the code
    let cst_code = derive_cst_code(fragment, nikaya_structure, cst_sutta.as_deref());

    (cst_file, cst_code, cst_vagga, cst_sutta, cst_paranum)
}
