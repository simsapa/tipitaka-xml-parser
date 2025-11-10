use anyhow::{Result, Context};
use quick_xml::Reader;
use quick_xml::events::Event;

use crate::types::{XmlFragment, FragmentAdjustments, FragmentKey};
use crate::sutta_builder::cst_code_to_sc_code_map;

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
/// If adjustments are provided for this fragment, use the adjusted end_line and end_char.
/// Returns (end_byte_pos, end_line, end_char)
pub fn apply_fragment_adjustment(
    xml_content: &str,
    default_end_pos: usize,
    default_end_line: usize,
    default_end_char: usize,
    cst_file: &str,
    frag_idx: usize,
    adjustments: Option<&FragmentAdjustments>,
) -> (usize, usize, usize) {
    // Check if there's an adjustment for this fragment
    if let Some(adjustments_map) = adjustments {
        let key = FragmentKey {
            cst_file: cst_file.to_string(),
            frag_idx,
        };

        if let Some(adjustment) = adjustments_map.get(&key) {
            // Apply adjustments if end_line is provided
            // If end_char is not provided, default to 0 (start of line)
            if let Some(adj_end_line) = adjustment.end_line {
                let adj_end_char = adjustment.end_char.unwrap_or(0);
                // Convert adjusted line/char to byte position
                let adj_end_pos = line_char_to_byte_pos(xml_content, adj_end_line, adj_end_char);
                return (adj_end_pos, adj_end_line, adj_end_char);
            }
        }
    }

    // No adjustment - use default detection
    (default_end_pos, default_end_line, default_end_char)
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
