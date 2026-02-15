//! Saṃyutta Nikāya Ṭīkā (sub-commentary) XML fragment parser

use anyhow::{Result, Context};
use quick_xml::events::Event;
use std::collections::HashMap;

use crate::types::{XmlFragment, FragmentType, GroupType, GroupLevel, ParserOverrides};
use crate::nikaya_structure::NikayaStructure;
use crate::xml_parser_trait::XmlParser;
use crate::parsers::helpers::{
    LineTrackingReader,
    apply_fragment_adjustment,
    populate_sc_fields_from_tsv_conditional,
    propagate_sc_codes_from_previous,
    HierarchyTracker,
    impl_xml_parser,
    FragmentBoundaryDetector,
    derive_cst_fields as derive_cst_fields_shared,
    is_sutta_subhead,
};

pub struct SamyuttaNikayaTika;

impl SamyuttaNikayaTika {
    pub fn new() -> Self {
        SamyuttaNikayaTika
    }
}

/// Parse XML content into fragments with line tracking
///
/// # Arguments
/// * `xml_content` - The complete XML file content
/// * `nikaya_structure` - The structure configuration for this nikaya
/// * `cst_file` - Name of the XML file being parsed
/// * `adjustments` - Optional fragment adjustments to apply
/// * `populate_sc_fields` - Whether to populate SC fields from embedded TSV
///
/// # Returns
/// Vector of fragments or error if parsing fails
pub fn parse_into_fragments(
    xml_content: &str,
    nikaya_structure: &NikayaStructure,
    cst_file: &str,
    overrides: &ParserOverrides,
    populate_sc_fields: bool,
) -> Result<Vec<XmlFragment>> {
    let mut reader = LineTrackingReader::new(xml_content);
    let mut hierarchy = HierarchyTracker::new(nikaya_structure.clone());
    let detector = FragmentBoundaryDetector::new(nikaya_structure, cst_file);

    let mut fragments: Vec<XmlFragment> = Vec::new();
    // Track: (byte_pos, line_num, char_pos)
    let mut current_fragment_start: Option<(usize, usize, usize)>;
    let mut current_frag_type: Option<FragmentType>;
    // Store hierarchy levels at the time fragment starts
    let mut current_fragment_group_levels: Vec<GroupLevel>;
    let mut pending_title: Option<(GroupType, String, Option<String>, Option<i32>)> = None; // (type, title, id, number)
    let mut in_sutta_content = false;
    // For MN/SN: track if we just saw a subhead element (will check text to see if numbered)
    let mut pending_subhead_check: Option<(usize, usize, usize)> = None; // (pos, line, char) of the subhead tag
    let mut seen_body_tag = false; // Track if we've seen the <body> opening tag
    let mut seen_first_sutta = false; // Track if we've encountered the first sutta marker
    let mut seen_first_vagga_or_sutta = false; // Track if we've seen the first vagga or sutta div
    let mut div_depth = 0; // Track div nesting depth to know when a sutta closes
    let mut sutta_div_depth: Option<usize> = None; // Track the depth of the current sutta div
    // For DN commentary: track the position of <div type="sutta"> that precedes <head rend="chapter">
    let mut pending_sutta_div_pos: Option<(usize, usize, usize)> = None;
    // For MN/SN: track the position of <div type="vagga"> that precedes <p rend="subhead">
    let mut pending_vagga_div_pos: Option<(usize, usize, usize)> = None;
    // For SN: track position of vagga title to include it in the next sutta fragment
    // When a vagga title appears between suttas, it should be part of the NEXT sutta, not the previous one
    let mut pending_vagga_title_pos: Option<(usize, usize, usize)> = None;
    // Store the vagga title info to apply when the next sutta starts
    let mut pending_vagga_title_info: Option<(String, Option<String>, Option<i32>)> = None; // (title, id, number)
    // For SN: track position of <div type="samyutta"> to include in next sutta fragment
    // When a new samyutta starts, its header should be part of the FIRST sutta, not a separate fragment
    // We defer entering the hierarchy level until we actually use the pending position, to ensure
    // the previous sutta fragment gets the correct (old) group levels.
    let mut pending_samyutta_div_pos: Option<(usize, usize, usize)> = None;
    // Store the samyutta info (title, id, number) to apply when we start the new fragment
    let mut pending_samyutta_info: Option<(String, Option<String>, Option<i32>)> = None;

    // Start with a Header fragment at the beginning of the file
    current_fragment_start = Some((0, 1, 0));
    current_frag_type = Some(FragmentType::Header);
    current_fragment_group_levels = hierarchy.get_current_levels();

    loop {
        // Capture position BEFORE reading the event (this is the start of the tag)
        let event_start_pos = reader.buffer_position();
        let event_start_line = reader.current_line();
        let event_start_char = reader.current_char();

        let event = reader.read_event()?;

        // Capture position AFTER reading the event (this is the end of the tag)
        let current_line = reader.current_line();
        let current_char = reader.current_char();
        let current_pos = reader.buffer_position();

        match event {
            Event::Start(ref e) | Event::Empty(ref e) => {
                let name_bytes = e.name();
                let tag_name = std::str::from_utf8(name_bytes.as_ref())
                    .context("Invalid UTF-8 in tag name")?
                    .to_string();

                // Parse attributes
                let mut attributes = HashMap::new();
                for attr in e.attributes() {
                    let attr = attr.context("Failed to parse attribute")?;
                    let key = std::str::from_utf8(attr.key.as_ref())
                        .context("Invalid UTF-8 in attribute key")?;
                    let value = attr.unescape_value()
                        .context("Failed to unescape attribute value")?;
                    attributes.insert(key.to_string(), value.to_string());
                }

                // Special handling for <body> tag - close Header fragment after it
                // Content after <body> will be included in the first Sutta fragment
                if tag_name == "body" && !seen_body_tag {
                    seen_body_tag = true;

                    // Close the Header fragment right after the <body> tag
                    // Track the adjusted end position to use as the next fragment's start
                    let mut next_frag_start_pos = current_pos;
                    let mut next_frag_start_line = current_line;
                    let mut next_frag_start_char = current_char;

                    if let (Some((frag_start_pos, frag_start_line, frag_start_char)), Some(frag_type)) =
                        (current_fragment_start, current_frag_type.as_ref()) {

                        // Apply adjustments if any
                        let (end_pos, end_line, end_char, collapsed) = apply_fragment_adjustment(
                            xml_content,
                            current_pos,
                            current_line,
                            current_char,
                            cst_file,
                            fragments.len(),
                            frag_start_pos,
                            frag_start_line,
                            frag_start_char,
                            overrides.correction_overrides.as_ref(),
                            overrides.adjustments.as_ref(),
                        )?;

                        // Use adjusted end position as next fragment's start to ensure continuous boundaries
                        next_frag_start_pos = end_pos;
                        next_frag_start_line = end_line;
                        next_frag_start_char = end_char;

                        let content_xml = xml_content[frag_start_pos..end_pos].to_string();
                        if collapsed || !content_xml.trim().is_empty() {
                            fragments.push(XmlFragment {
                                nikaya: nikaya_structure.nikaya.clone(),
                                frag_type: frag_type.clone(),
                                content_xml,
                                start_line: frag_start_line,
                                end_line,
                                start_char: frag_start_char,
                                end_char,
                                group_levels: current_fragment_group_levels.clone(),
                                cst_file: cst_file.to_string(),
                                frag_idx: fragments.len(),
                                frag_review: None,
                                cst_code: None,
                                cst_vagga: None,
                                cst_sutta: None,
                                cst_paranum: None,
                                sc_code: None,
                                sc_sutta: None,
                            });
                        }
                    }

                    // Start a Sutta fragment at the adjusted end position of the Header fragment
                    // This ensures no gaps or overlaps when checked overrides are used
                    current_fragment_start = Some((next_frag_start_pos, next_frag_start_line, next_frag_start_char));
                    current_frag_type = Some(FragmentType::Sutta);
                    current_fragment_group_levels = hierarchy.get_current_levels();
                    in_sutta_content = true;
                }

                // Check for boundary
                if let Some((group_type, _, id, number)) = detector.check_boundary(&tag_name, &attributes) {
                    // For <div> elements with an ID, enter the level immediately to preserve the ID
                    // The title will be updated later from a child <head> element
                    if tag_name == "div" && id.is_some() {
                        // Before entering a new Samyutta, Vagga, or Sutta level, close any open sutta fragment
                        // This ensures the fragment uses the CURRENT level, not the next one
                        // BUT: Don't close for the FIRST vagga/sutta - that should include the preamble content
                        // ALSO: For Samyutta level, don't close immediately - store as pending and let the
                        // next sutta handle it, so the samyutta header is included with the first sutta
                        let is_samyutta_level = matches!(group_type, GroupType::Samyutta);
                        let is_structure_level = matches!(group_type, GroupType::Samyutta | GroupType::Vagga | GroupType::Sutta);
                        let is_vagga_or_sutta_level = matches!(group_type, GroupType::Vagga | GroupType::Sutta);
                        let is_first_vagga_or_sutta = !seen_first_vagga_or_sutta && is_vagga_or_sutta_level;

                        // Track whether we should skip entering the hierarchy level
                        // (used when storing a pending samyutta)
                        let mut skip_hierarchy_entry = false;

                        if is_first_vagga_or_sutta {
                            // Mark that we've seen the first vagga/sutta, but don't close the fragment
                            // The preamble content will be included with the first sutta
                            seen_first_vagga_or_sutta = true;
                        } else if is_samyutta_level && in_sutta_content && seen_first_sutta {
                            // For Samyutta level changes (after the first sutta), don't close the fragment yet.
                            // Store the position as pending - the samyutta header should be included with
                            // the FIRST sutta of the new samyutta, not as a separate fragment.
                            // IMPORTANT: We also defer entering the hierarchy level, so the previous sutta
                            // fragment gets the correct (old) group levels when it's closed.

                            // If there's already a pending samyutta that was never consumed by a subhead,
                            // flush it now: close the current fragment at the OLD pending position, enter
                            // the old samyutta hierarchy, and start a new fragment. This handles samyuttas
                            // that have no <p rend="subhead"> (e.g. grouped commentaries).
                            if let Some((old_pos, old_line, old_char)) = pending_samyutta_div_pos.take() {
                                // Close current fragment at the old pending samyutta position
                                if let (Some((frag_start_pos, frag_start_line, frag_start_char)), Some(frag_type)) =
                                    (current_fragment_start, current_frag_type.as_ref()) {
                                    if matches!(frag_type, FragmentType::Sutta) && frag_start_pos < old_pos {
                                        let (end_pos, end_line, end_char, collapsed) = apply_fragment_adjustment(
                                            xml_content,
                                            old_pos,
                                            old_line,
                                            old_char,
                                            cst_file,
                                            fragments.len(),
                                            frag_start_pos,
                                            frag_start_line,
                                            frag_start_char,
                                            overrides.correction_overrides.as_ref(),
                                            overrides.adjustments.as_ref(),
                                        )?;

                                        let content_xml = xml_content[frag_start_pos..end_pos].to_string();
                                        if collapsed || !content_xml.trim().is_empty() {
                                            fragments.push(XmlFragment {
                                                nikaya: nikaya_structure.nikaya.clone(),
                                                frag_type: frag_type.clone(),
                                                content_xml,
                                                start_line: frag_start_line,
                                                end_line,
                                                start_char: frag_start_char,
                                                end_char,
                                                group_levels: current_fragment_group_levels.clone(),
                                                cst_file: cst_file.to_string(),
                                                frag_idx: fragments.len(),
                                                frag_review: None,
                                                cst_code: None,
                                                cst_vagga: None,
                                                cst_sutta: None,
                                                cst_paranum: None,
                                                sc_code: None,
                                                sc_sutta: None,
                                            });
                                        }

                                        // Enter the old pending samyutta hierarchy before starting new fragment
                                        if let Some((samyutta_title, sam_id, sam_number)) = pending_samyutta_info.take() {
                                            hierarchy.enter_level(GroupType::Samyutta, samyutta_title, sam_id, sam_number);
                                        }

                                        // Start new fragment at the old samyutta position
                                        current_fragment_start = Some((end_pos, end_line, end_char));
                                        current_frag_type = Some(FragmentType::Sutta);
                                        current_fragment_group_levels = hierarchy.get_current_levels();
                                    }
                                }
                            }

                            pending_samyutta_div_pos = Some((event_start_pos, event_start_line, event_start_char));
                            pending_samyutta_info = Some((String::new(), id.clone(), number));
                            // DON'T close the current fragment here - it will be closed when the next
                            // sutta subhead is detected, using this pending position
                            // Skip entering the hierarchy level - we'll do that when we use the pending position
                            skip_hierarchy_entry = true;
                        } else if is_structure_level && in_sutta_content && !is_samyutta_level {
                            if let (Some((frag_start_pos, frag_start_line, frag_start_char)), Some(frag_type)) =
                                (current_fragment_start, current_frag_type.as_ref()) {

                                // Only close if this is a Sutta fragment and has actual sutta content
                                if matches!(frag_type, FragmentType::Sutta) {
                                    let tentative_content = xml_content[frag_start_pos..event_start_pos].to_string();
                                    let has_sutta_content = tentative_content.contains("rend=\"subhead\"") ||
                                                           tentative_content.contains("rend=\"chapter\"") ||
                                                           tentative_content.contains("rend=\"bodytext\"");

                                    if has_sutta_content {
                                        // Close at the current position (before the new vagga/sutta div)
                                        let (end_pos, end_line, end_char, collapsed) = apply_fragment_adjustment(
                                            xml_content,
                                            event_start_pos,
                                            event_start_line,
                                            event_start_char,
                                            cst_file,
                                            fragments.len(),
                                            frag_start_pos,
                                            frag_start_line,
                                            frag_start_char,
                                            overrides.correction_overrides.as_ref(),
                                            overrides.adjustments.as_ref(),
                                        )?;

                                        // Create content with adjusted end position
                                        let content_xml = xml_content[frag_start_pos..end_pos].to_string();

                                if collapsed || !content_xml.trim().is_empty() {
                                    fragments.push(XmlFragment {
                                        nikaya: nikaya_structure.nikaya.clone(),
                                        frag_type: frag_type.clone(),
                                        content_xml,
                                        start_line: frag_start_line,
                                        end_line,
                                        start_char: frag_start_char,
                                        end_char,
                                        group_levels: current_fragment_group_levels.clone(),
                                        cst_file: cst_file.to_string(),
                                        frag_idx: fragments.len(),
                                        frag_review: None,
                                        cst_code: None,
                                        cst_vagga: None,
                                        cst_sutta: None,
                                        cst_paranum: None,
                                        sc_code: None,
                                        sc_sutta: None,
                                    });
                                }

                                        // Start new fragment at the adjusted end position of the previous fragment
                                        // This ensures no gap in XML reconstruction when adjustments are used
                                        current_fragment_start = Some((end_pos, end_line, end_char));
                                        current_frag_type = Some(FragmentType::Sutta);
                                        // Note: we'll update group_levels AFTER entering the new level
                                    }
                                }
                            }
                        }

                        // Only enter the hierarchy level if we're not deferring it
                        // (pending samyutta divs defer hierarchy entry until the pending position is used)
                        if !skip_hierarchy_entry {
                            hierarchy.enter_level(group_type.clone(), String::new(), id, number);

                            // Update group_levels after entering any new level while a fragment is open
                            if current_fragment_start.is_some() {
                                current_fragment_group_levels = hierarchy.get_current_levels();
                            }
                        }

                        // Don't set pending_title - the next <head> will update the title
                    } else {
                        // For other elements, we'll get the title from the text content, so store it as pending
                        // EXCEPT for MN/SN/AN subheads which need text content validation
                        let is_sutta_subhead = (nikaya_structure.nikaya == "majjhima" ||
                                               nikaya_structure.nikaya == "samyutta" ||
                                               nikaya_structure.nikaya == "anguttara") &&
                                              matches!(group_type, GroupType::Sutta) &&
                                              tag_name == "p" &&
                                              attributes.get("rend") == Some(&"subhead".to_string());

                        // For AN tika/commentary: <p rend="chapter"> = Vagga boundary, should close fragments
                        let is_an_vagga_chapter = nikaya_structure.nikaya == "anguttara" &&
                                                 matches!(group_type, GroupType::Vagga) &&
                                                 tag_name == "p" &&
                                                 attributes.get("rend") == Some(&"chapter".to_string());

                        // For SN atthakatha/tika: <p rend="title"> = Vagga boundary, should close fragments
                        // This is needed because SN atthakatha files use <p rend="title"> for vagga titles
                        // and we need to close the current sutta BEFORE entering the new vagga
                        let is_sn_att_vagga_title = nikaya_structure.nikaya == "samyutta" &&
                                                    matches!(group_type, GroupType::Vagga) &&
                                                    tag_name == "p" &&
                                                    attributes.get("rend") == Some(&"title".to_string());

                        // Before entering a new Vagga level in AN tika, close any open sutta fragment
                        if is_an_vagga_chapter && in_sutta_content {
                            let is_first_vagga = !seen_first_vagga_or_sutta;

                            if is_first_vagga {
                                // Mark that we've seen the first vagga, but don't close the fragment
                                seen_first_vagga_or_sutta = true;
                            } else {
                                // Close current sutta fragment before entering new Vagga
                                if let (Some((frag_start_pos, frag_start_line, frag_start_char)), Some(frag_type)) =
                                    (current_fragment_start, current_frag_type.as_ref()) {

                                    if matches!(frag_type, FragmentType::Sutta) {
                                        let tentative_content = xml_content[frag_start_pos..event_start_pos].to_string();
                                        let has_sutta_content = tentative_content.contains("rend=\"subhead\"") ||
                                                               tentative_content.contains("rend=\"chapter\"") ||
                                                               tentative_content.contains("rend=\"bodytext\"");

                                        if has_sutta_content {
                                            // Close at the current position (before the new vagga chapter)
                                            let (end_pos, end_line, end_char, collapsed) = apply_fragment_adjustment(
                                                xml_content,
                                                event_start_pos,
                                                event_start_line,
                                                event_start_char,
                                                cst_file,
                                                fragments.len(),
                                                frag_start_pos,
                                                frag_start_line,
                                                frag_start_char,
                                                overrides.correction_overrides.as_ref(),
                                                overrides.adjustments.as_ref(),
                                            )?;

                                            let content_xml = xml_content[frag_start_pos..end_pos].to_string();

                                            if collapsed || !content_xml.trim().is_empty() {
                                                fragments.push(XmlFragment {
                                                    nikaya: nikaya_structure.nikaya.clone(),
                                                    frag_type: frag_type.clone(),
                                                    content_xml,
                                                    start_line: frag_start_line,
                                                    end_line,
                                                    start_char: frag_start_char,
                                                    end_char,
                                                    group_levels: current_fragment_group_levels.clone(),
                                                    cst_file: cst_file.to_string(),
                                                    frag_idx: fragments.len(),
                                                    frag_review: None,
                                                    cst_code: None,
                                                    cst_vagga: None,
                                                    cst_sutta: None,
                                                    cst_paranum: None,
                                                    sc_code: None,
                                                    sc_sutta: None,
                                                });
                                            }

                                            // Start new fragment at the adjusted end position
                                            current_fragment_start = Some((end_pos, end_line, end_char));
                                            current_frag_type = Some(FragmentType::Sutta);
                                            // Note: we'll update group_levels AFTER entering the new level via pending_title
                                        }
                                    }
                                }
                            }
                        }

                        // For SN atthakatha/tika: track vagga title to include in next sutta
                        // When a vagga title appears between suttas, it should be part of the NEXT sutta
                        // We delay updating the hierarchy until the next sutta starts
                        if is_sn_att_vagga_title && in_sutta_content && seen_first_sutta {
                            // Store the vagga title position - it will be used when closing the next sutta
                            pending_vagga_title_pos = Some((event_start_pos, event_start_line, event_start_char));
                            // Store the vagga info to apply when the next sutta starts
                            pending_vagga_title_info = Some((String::new(), id, number));
                            // Don't set pending_title - we'll handle it manually when the next sutta starts
                        } else if !is_sutta_subhead {
                            pending_title = Some((group_type.clone(), String::new(), id, number));
                        }
                    }
                }

                // Track div depth for nested div elements
                if tag_name == "div" {
                    div_depth += 1;

                    // For DN commentary: <div type="sutta"> precedes <head rend="chapter">
                    // Store its position to use when we encounter the <head> tag
                    let is_commentary = cst_file.ends_with(".att.xml") || cst_file.ends_with(".tik.xml");
                    if is_commentary &&
                       nikaya_structure.nikaya == "digha" &&
                       attributes.get("type") == Some(&"sutta".to_string()) {
                        pending_sutta_div_pos = Some((event_start_pos, event_start_line, event_start_char));
                    }

                    // For MN/SN: <div type="vagga"> precedes <p rend="subhead">
                    // Store its position to use when we encounter the subhead
                    if (nikaya_structure.nikaya == "majjhima" || nikaya_structure.nikaya == "samyutta") &&
                       attributes.get("type") == Some(&"vagga".to_string()) {
                        pending_vagga_div_pos = Some((event_start_pos, event_start_line, event_start_char));
                    }
                }

                // Handle sutta boundaries based on nikaya structure
                let is_potential_sutta_marker = detector.is_sutta_start(&tag_name, &attributes);

                // For MN/SN/AN, we need to check the text content to see if it's a numbered subhead
                if is_potential_sutta_marker &&
                   (nikaya_structure.nikaya == "majjhima" || nikaya_structure.nikaya == "samyutta" || nikaya_structure.nikaya == "anguttara") &&
                   tag_name == "p" && attributes.get("rend") == Some(&"subhead".to_string()) {
                    // Store START position of the tag for later text check
                    pending_subhead_check = Some((event_start_pos, event_start_line, event_start_char));
                } else if is_potential_sutta_marker {
                    // Check if this sutta marker is a div that should track depth
                    // For DN base text: <div type="sutta"> IS the sutta marker, so track depth
                    // For DN commentary: <head rend="chapter"> is the sutta marker, <div type="sutta"> is NOT
                    let is_commentary = cst_file.ends_with(".att.xml") || cst_file.ends_with(".tik.xml");
                    let should_track_div_depth = tag_name == "div" &&
                                                 attributes.get("type") == Some(&"sutta".to_string()) &&
                                                 !is_commentary;

                    // Check if this is the first sutta marker after <body>
                    if !seen_first_sutta && in_sutta_content {
                        // This is the FIRST sutta marker - don't close current fragment
                        // Just mark that we've seen it
                        seen_first_sutta = true;

                        // Only track div depth if this is a div-based sutta marker
                        if should_track_div_depth {
                            sutta_div_depth = Some(div_depth);
                        }
                        // Continue with the current fragment
                    } else if seen_first_sutta {
                        // This is a SUBSEQUENT sutta marker - start a new fragment
                        // For DN commentary, check if there's a pending <div type="sutta"> position
                        // If so, use that as the start position (and close position for previous fragment)
                        let (start_pos, start_line, start_char, close_pos, close_line, close_char) =
                            if let Some((div_pos, div_line, div_char)) = pending_sutta_div_pos.take() {
                                // Use the <div> position
                                (div_pos, div_line, div_char, div_pos, div_line, div_char)
                            } else {
                                // Use the current tag position (normal case)
                                (event_start_pos, event_start_line, event_start_char,
                                 event_start_pos, event_start_line, event_start_char)
                            };

                        // Close current sutta fragment (excluding this tag)
                        if let (Some((frag_start_pos, frag_start_line, frag_start_char)), Some(frag_type)) =
                            (current_fragment_start, current_frag_type.as_ref()) {

                            // Apply adjustments if any
                            let (end_pos, end_line, end_char, collapsed) = apply_fragment_adjustment(
                                xml_content,
                                close_pos,
                                close_line,
                                close_char,
                                cst_file,
                                fragments.len(),
                                frag_start_pos,
                                frag_start_line,
                                frag_start_char,
                                overrides.correction_overrides.as_ref(),
                                overrides.adjustments.as_ref(),
                            )?;

                            let content_xml = xml_content[frag_start_pos..end_pos].to_string();
                                 if collapsed || !content_xml.trim().is_empty() {
                                    fragments.push(XmlFragment {
                                        nikaya: nikaya_structure.nikaya.clone(),
                                        frag_type: frag_type.clone(),
                                        content_xml,
                                        start_line: frag_start_line,
                                        end_line,
                                        start_char: frag_start_char,
                                        end_char,
                                        group_levels: current_fragment_group_levels.clone(),
                                        cst_file: cst_file.to_string(),
                                        frag_idx: fragments.len(),
                                        frag_review: None,
                                        cst_code: None,
                                        cst_vagga: None,
                                        cst_sutta: None,
                                        cst_paranum: None,
                                        sc_code: None,
                                        sc_sutta: None,
                                    });

                                    // If we adjusted the end position, start the next fragment there
                                    // to avoid gaps in XML reconstruction
                                    current_fragment_start = Some((end_pos, end_line, end_char));
                                } else {
                                    // No content was written, start from the original position
                                    current_fragment_start = Some((start_pos, start_line, start_char));
                                }
                        } else {
                            // No previous fragment to close, start from the original position
                            current_fragment_start = Some((start_pos, start_line, start_char));
                        }

                        current_frag_type = Some(FragmentType::Sutta);
                        current_fragment_group_levels = hierarchy.get_current_levels();

                        // Only track div depth if this is a div-based sutta marker
                        if should_track_div_depth {
                            sutta_div_depth = Some(div_depth);
                        }
                        // Stay in_sutta_content = true
                    }
                }
            },

            Event::Text(ref e) => {
                let text = e.unescape()
                    .context("Failed to unescape text content")?
                    .trim()
                    .to_string();

                // Check if this text is for a pending subhead (MN/SN style)
                if let Some((subhead_pos, subhead_line, subhead_char)) = pending_subhead_check.take() {
                    let is_sutta_commentary = is_sutta_subhead(&text, &nikaya_structure.nikaya, cst_file);

                    if is_sutta_commentary {
                        // This is a sutta boundary!
                        // Check if this is the first sutta marker after <body>
                        if !seen_first_sutta && in_sutta_content {
                            // This is the FIRST sutta marker - don't close current fragment
                            seen_first_sutta = true;
                            // Clear pending_vagga_div_pos so it's not used for the next sutta
                            // The first sutta should include the preamble, so we don't split at the vagga div
                            pending_vagga_div_pos = None;
                            // Update hierarchy with sutta title
                            hierarchy.enter_level(GroupType::Sutta, text.clone(), None, None);
                            // Update group_levels to include the new Sutta level
                            current_fragment_group_levels = hierarchy.get_current_levels();
                            // Continue with the current fragment
                        } else if seen_first_sutta {
                            // This is a SUBSEQUENT sutta marker - start a new fragment
                            // Priority order for determining the fragment boundary position:
                            // 1. pending_samyutta_div_pos - samyutta header should be part of first sutta
                            // 2. pending_vagga_title_pos - vagga title should be part of next sutta
                            // 3. pending_vagga_div_pos - vagga div position
                            // 4. subhead position - normal case
                            //
                            // IMPORTANT: We must ensure the close position is >= current fragment start position
                            // to avoid invalid slice bounds
                            // Track if we used the pending samyutta position, so we can enter the hierarchy level later
                            let mut used_pending_samyutta = false;

                            let (start_pos, start_line, start_char, close_pos, close_line, close_char) =
                                if let (Some((samyutta_pos, samyutta_line, samyutta_char)), Some((frag_start_pos, _, _))) =
                                    (pending_samyutta_div_pos, current_fragment_start) {
                                    // Samyutta div position takes priority - the samyutta header should be
                                    // part of the first sutta of the new samyutta
                                    if frag_start_pos < samyutta_pos {
                                        pending_samyutta_div_pos = None; // Clear it since we're using it
                                        // Also clear vagga positions since they're within this samyutta boundary
                                        pending_vagga_title_pos = None;
                                        pending_vagga_title_info = None;
                                        pending_vagga_div_pos = None;
                                        used_pending_samyutta = true;
                                        (samyutta_pos, samyutta_line, samyutta_char, samyutta_pos, samyutta_line, samyutta_char)
                                    } else {
                                        // Fragment started after samyutta div, can't use it as boundary
                                        pending_samyutta_div_pos = None;
                                        pending_samyutta_info = None;
                                        (subhead_pos, subhead_line, subhead_char,
                                         subhead_pos, subhead_line, subhead_char)
                                    }
                                } else if let (Some((title_pos, title_line, title_char)), Some((frag_start_pos, _, _))) =
                                    (pending_vagga_title_pos, current_fragment_start) {
                                    // Only use vagga title position if the fragment started before it
                                    if frag_start_pos < title_pos {
                                        pending_vagga_title_pos = None; // Clear it since we're using it
                                        // Use the vagga title position - the vagga title should be part of the NEW sutta
                                        // So we close the previous sutta at the title position (excluding it)
                                        // and start the new sutta at the title position (including it)
                                        (title_pos, title_line, title_char, title_pos, title_line, title_char)
                                    } else {
                                        // Fragment started after vagga title, can't use it as boundary
                                        pending_vagga_title_pos = None;
                                        pending_vagga_title_info = None; // Also clear the title info
                                        // Fall through to check vagga div position
                                        if let Some((div_pos, div_line, div_char)) = pending_vagga_div_pos.take() {
                                            (div_pos, div_line, div_char, div_pos, div_line, div_char)
                                        } else {
                                            (subhead_pos, subhead_line, subhead_char,
                                             subhead_pos, subhead_line, subhead_char)
                                        }
                                    }
                                } else if let Some((div_pos, div_line, div_char)) = pending_vagga_div_pos.take() {
                                    // Use the vagga <div> position
                                    (div_pos, div_line, div_char, div_pos, div_line, div_char)
                                } else {
                                    // Use the subhead position (normal case)
                                    (subhead_pos, subhead_line, subhead_char,
                                     subhead_pos, subhead_line, subhead_char)
                                };

                            // Already in a sutta - close current and start new
                            if let (Some((frag_start_pos, frag_start_line, frag_start_char)), Some(frag_type)) =
                                (current_fragment_start, current_frag_type.as_ref()) {

                                // Apply adjustments if any
                                let (end_pos, end_line, end_char, collapsed) = apply_fragment_adjustment(
                                    xml_content,
                                    close_pos,
                                    close_line,
                                    close_char,
                                    cst_file,
                                    fragments.len(),
                                    frag_start_pos,
                                    frag_start_line,
                                    frag_start_char,
                                    overrides.correction_overrides.as_ref(),
                                    overrides.adjustments.as_ref(),
                                )?;

                                let content_xml = xml_content[frag_start_pos..end_pos].to_string();
                                if collapsed || !content_xml.trim().is_empty() {
                                    fragments.push(XmlFragment {
                                        nikaya: nikaya_structure.nikaya.clone(),
                                        frag_type: frag_type.clone(),
                                        content_xml,
                                        start_line: frag_start_line,
                                        end_line,
                                        start_char: frag_start_char,
                                        end_char,
                                        group_levels: current_fragment_group_levels.clone(),
                                        cst_file: cst_file.to_string(),
                                        frag_idx: fragments.len(),
                                        frag_review: None,
                                        cst_code: None,
                                        cst_vagga: None,
                                        cst_sutta: None,
                                        cst_paranum: None,
                                        sc_code: None,
                                        sc_sutta: None,
                                    });

                                    // If we adjusted the end position, start the next fragment there
                                    // to avoid gaps in XML reconstruction
                                    current_fragment_start = Some((end_pos, end_line, end_char));
                                } else {
                                    // No content was written, start from the original position
                                    current_fragment_start = Some((start_pos, start_line, start_char));
                                }
                            } else {
                                // No previous fragment to close, start from the original position
                                current_fragment_start = Some((start_pos, start_line, start_char));
                            }

                            // Apply pending samyutta info if we used a samyutta boundary
                            // This ensures the samyutta hierarchy level is entered before entering vagga/sutta
                            if used_pending_samyutta {
                                if let Some((samyutta_title, id, number)) = pending_samyutta_info.take() {
                                    // Enter the samyutta level that was deferred
                                    // The title may be empty here - it will be updated when we see the <head> text
                                    hierarchy.enter_level(GroupType::Samyutta, samyutta_title, id, number);
                                }
                            }

                            // Apply pending vagga title if there is one
                            // This ensures the vagga hierarchy is updated before the new sutta starts
                            if let Some((vagga_title, id, number)) = pending_vagga_title_info.take() {
                                if !vagga_title.is_empty() {
                                    hierarchy.enter_level(GroupType::Vagga, vagga_title, id, number);
                                }
                            }

                            // Update hierarchy with new sutta title
                            hierarchy.enter_level(GroupType::Sutta, text.clone(), None, None);

                            current_frag_type = Some(FragmentType::Sutta);
                            current_fragment_group_levels = hierarchy.get_current_levels();
                        }
                    }
                    // If not numbered, it's just a section heading within a sutta - ignore
                }

                // Handle vagga title text - store it for later when the next sutta starts
                // Note: We check without .take() first, and only update if text is not empty
                // This prevents whitespace from clearing the stored vagga title
                if pending_vagga_title_info.is_some() && !text.is_empty() {
                    // Update the stored vagga title with the actual text
                    if let Some((_, id, number)) = pending_vagga_title_info.take() {
                        pending_vagga_title_info = Some((text.clone(), id, number));
                    }
                }

                // If we have a pending title, update it with this text
                if let Some((group_type, _, id, number)) = pending_title.take() {
                    if !text.is_empty() {
                        // Special handling for Samyutta titles when we have a pending samyutta position:
                        // Don't enter the hierarchy yet - update pending_samyutta_info instead.
                        // The hierarchy will be entered when we use the pending samyutta position.
                        if matches!(group_type, GroupType::Samyutta) && pending_samyutta_div_pos.is_some() {
                            // Update pending_samyutta_info with the title text
                            if let Some((_, existing_id, existing_number)) = pending_samyutta_info.take() {
                                pending_samyutta_info = Some((text.clone(), existing_id.or(id), existing_number.or(number)));
                            } else {
                                pending_samyutta_info = Some((text.clone(), id, number));
                            }
                            // Don't enter the hierarchy or update group_levels yet
                        } else {
                            hierarchy.enter_level(group_type, text, id, number);

                            // Update group_levels after entering any new level while a fragment is open
                            if current_fragment_start.is_some() {
                                current_fragment_group_levels = hierarchy.get_current_levels();
                            }
                        }
                    }
                }
            },

            Event::End(ref e) => {
                let name_bytes = e.name();
                let tag_name = std::str::from_utf8(name_bytes.as_ref())
                    .context("Invalid UTF-8 in tag name")?
                    .to_string();

                // Track div depth - decrement when seeing closing div tags
                if tag_name == "div" {
                    // Check if this is closing the current sutta div
                    if in_sutta_content {
                        if let Some(sutta_depth) = sutta_div_depth {
                            if div_depth == sutta_depth {
                                // This closes the current sutta div
                                // DON'T close the fragment here - let the next sutta or </body> do it
                                // This allows the last sutta to include content after its </div>
                                sutta_div_depth = None;
                            }
                        }
                    }

                    // Decrement div depth after processing
                    div_depth = div_depth.saturating_sub(1);
                }

                // Check if this closes the body tag - now we exit sutta content
                if tag_name == "body" && seen_body_tag {
                    // Close any pending sutta fragment first
                    // The sutta fragment should include ALL content up to (but not including) </body>
                    if let (Some((frag_start_pos, frag_start_line, frag_start_char)), Some(frag_type)) =
                        (current_fragment_start, current_frag_type.as_ref()) {

                        // Apply adjustments if any
                        let (end_pos, end_line, end_char, collapsed) = apply_fragment_adjustment(
                            xml_content,
                            event_start_pos,
                            event_start_line,
                            event_start_char,
                            cst_file,
                            fragments.len(),
                            frag_start_pos,
                            frag_start_line,
                            frag_start_char,
                            overrides.correction_overrides.as_ref(),
                            overrides.adjustments.as_ref(),
                        )?;

                        // Include everything from start up to the adjusted end position
        let content_xml = xml_content[frag_start_pos..end_pos].to_string();
        if collapsed || !content_xml.trim().is_empty() {
            fragments.push(XmlFragment {
                nikaya: nikaya_structure.nikaya.clone(),
                frag_type: frag_type.clone(),
                content_xml,
                start_line: frag_start_line,
                end_line,
                start_char: frag_start_char,
                end_char,
                group_levels: current_fragment_group_levels.clone(),
                cst_file: cst_file.to_string(),
                frag_idx: fragments.len(),
                frag_review: None,
                cst_code: None,
                cst_vagga: None,
                cst_sutta: None,
                cst_paranum: None,
                sc_code: None,
                sc_sutta: None,
            });

            // Start the final Header fragment at the adjusted end position
            // to avoid gaps in XML reconstruction
            current_fragment_start = Some((end_pos, end_line, end_char));
        } else {
            // No content was written, start from the original position
            current_fragment_start = Some((event_start_pos, event_start_line, event_start_char));
        }
                    } else {
                        // No previous fragment, start from the original position
                        current_fragment_start = Some((event_start_pos, event_start_line, event_start_char));
                    }

                    current_frag_type = Some(FragmentType::Header);
                    current_fragment_group_levels = hierarchy.get_current_levels();
                    in_sutta_content = false;
                }
            },

            Event::Eof => break,

            _ => {},
        }
    }

    // Close any remaining fragment (usually the final Header fragment)
    if let (Some((frag_start_pos, frag_start_line, frag_start_char)), Some(frag_type)) =
        (current_fragment_start, current_frag_type) {

        // Apply adjustments if any
        let (end_pos, end_line, end_char, collapsed) = apply_fragment_adjustment(
            xml_content,
            xml_content.len(),
            reader.current_line(),
            reader.current_char(),
            cst_file,
            fragments.len(),
            frag_start_pos,
            frag_start_line,
            frag_start_char,
            overrides.correction_overrides.as_ref(),
            overrides.adjustments.as_ref(),
        )?;

        let content_xml = xml_content[frag_start_pos..end_pos].to_string();
        if collapsed || !content_xml.trim().is_empty() {
                            fragments.push(XmlFragment {
                                nikaya: nikaya_structure.nikaya.clone(),
                                frag_type: frag_type.clone(),
                                content_xml,
                                start_line: frag_start_line,
                                end_line,
                                start_char: frag_start_char,
                                end_char,
                                group_levels: current_fragment_group_levels.clone(),
                                cst_file: cst_file.to_string(),
                                frag_idx: fragments.len(),
                                frag_review: None,
                                cst_code: None,
                                cst_vagga: None,
                                cst_sutta: None,
                                cst_paranum: None,
                                sc_code: None,
                                sc_sutta: None,
                            });
        }
    }

    // Post-process fragments to derive CST fields
    for fragment in &mut fragments {
        let (cst_file, cst_code, cst_vagga, cst_sutta, cst_paranum) =
            derive_cst_fields_shared(fragment, nikaya_structure);

        fragment.cst_file = cst_file;
        fragment.cst_code = cst_code;
        fragment.cst_vagga = cst_vagga;
        fragment.cst_sutta = cst_sutta;
        fragment.cst_paranum = cst_paranum;
    }

    // Populate SC fields from embedded TSV if requested
    if populate_sc_fields {
        populate_sc_fields_from_tsv_conditional(&mut fragments)?;
        propagate_sc_codes_from_previous(&mut fragments, overrides.pali_titles.as_ref());
    }

    Ok(fragments)
}

impl_xml_parser!(SamyuttaNikayaTika);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::nikaya_detector::detect_nikaya_structure;

    /// Helper to create minimal DN XML for testing
    fn create_dn_sample_xml() -> String {
        r#"<?xml version="1.0" encoding="UTF-8"?>
<TEI.2>
<teiHeader></teiHeader>
<text>
<body>
<p rend="nikaya">Dīghanikāyo</p>
<div id="dn1" type="book">
<head rend="book">Sīlakkhandhavaggapāḷi</head>
<div id="dn1_1" type="sutta">
<head rend="chapter">1. Brahmajālasutta</head>
<p rend="subhead">Paribbājakakathā</p>
<p rend="bodytext" n="1">Evaṃ me sutaṃ – ekaṃ samayaṃ bhagavā antarā ca rājagahaṃ antarā ca nālandaṃ.</p>
<p rend="bodytext" n="2">Atha kho bhagavā ambalatthikāyaṃ rājāgārake ekarattivāsaṃ upagacchi.</p>
</div>
</div>
</body>
</text>
</TEI.2>"#.to_string()
    }

    /// Helper to create minimal MN XML for testing
    fn create_mn_sample_xml() -> String {
        r#"<?xml version="1.0" encoding="UTF-8"?>
<TEI.2>
<teiHeader></teiHeader>
<text>
<body>
<p rend="nikaya">Majjhimanikāyo</p>
<div id="mn1" type="book">
<head rend="book">Mūlapaṇṇāsapāḷi</head>
<div id="mn1_1" type="vagga">
<head rend="chapter">Mūlapariyāyavaggo</head>
<div id="mn1_1_1" type="sutta">
<p rend="subhead">1. Mūlapariyāyasutta</p>
<p rend="bodytext" n="1">Evaṃ me sutaṃ – ekaṃ samayaṃ bhagavā ukkaṭṭhāyaṃ viharati.</p>
<p rend="bodytext" n="2">Tatra kho bhagavā bhikkhū āmantesi – "bhikkhavo"ti.</p>
</div>
</div>
</div>
</body>
</text>
</TEI.2>"#.to_string()
    }

    #[test]
    fn test_parse_dn_sample_basic() {
        let xml = create_dn_sample_xml();
        let structure = detect_nikaya_structure(&xml).expect("Should detect DN structure");

        assert_eq!(structure.nikaya, "digha");

        let fragments = parse_into_fragments(&xml, &structure, "test.xml", &ParserOverrides::default(), false).expect("Should parse fragments");

        // Should have at least one fragment
        assert!(!fragments.is_empty(), "Should have at least one fragment");
    }

    #[test]
    fn test_parse_dn_fragment_count() {
        let xml = create_dn_sample_xml();
        let structure = detect_nikaya_structure(&xml).unwrap();
        let fragments = parse_into_fragments(&xml, &structure, "test.xml", &ParserOverrides::default(), false).unwrap();

        // Count sutta fragments
        let sutta_fragments: Vec<_> = fragments.iter()
            .filter(|f| matches!(f.frag_type, FragmentType::Sutta))
            .collect();

        // Should have one sutta fragment
        assert_eq!(sutta_fragments.len(), 1, "Should have exactly one sutta fragment");
    }

    #[test]
    fn test_parse_dn_line_tracking() {
        let xml = create_dn_sample_xml();
        let structure = detect_nikaya_structure(&xml).unwrap();
        let fragments = parse_into_fragments(&xml, &structure, "test.xml", &ParserOverrides::default(), false).unwrap();

        for fragment in &fragments {
            // Line numbers should be valid (start > 0, end >= start)
            assert!(fragment.start_line > 0, "Start line should be > 0");
            assert!(fragment.end_line >= fragment.start_line,
                    "End line should be >= start line");
        }
    }

    #[test]
    fn test_parse_mn_sample_basic() {
        let xml = create_mn_sample_xml();
        let structure = detect_nikaya_structure(&xml).expect("Should detect MN structure");

        assert_eq!(structure.nikaya, "majjhima");

        let fragments = parse_into_fragments(&xml, &structure, "test.xml", &ParserOverrides::default(), false).expect("Should parse fragments");

        assert!(!fragments.is_empty(), "Should have at least one fragment");
    }

    #[test]
    fn test_fragment_content_not_empty() {
        let xml = create_dn_sample_xml();
        let structure = detect_nikaya_structure(&xml).unwrap();
        let fragments = parse_into_fragments(&xml, &structure, "test.xml", &ParserOverrides::default(), false).unwrap();

        for fragment in &fragments {
            // Each fragment should have non-empty content
            assert!(!fragment.content_xml.trim().is_empty(),
                    "Fragment content should not be empty");
        }
    }

    #[test]
    fn test_character_position_tracking() {
        let xml = create_dn_sample_xml();
        let structure = detect_nikaya_structure(&xml).unwrap();
        let fragments = parse_into_fragments(&xml, &structure, "test.xml", &ParserOverrides::default(), false).unwrap();

        for fragment in &fragments {
            // Character positions should be valid
            assert!(fragment.start_char <= fragment.end_char || fragment.start_line < fragment.end_line,
                    "Character positions should be valid: start_line={}, start_char={}, end_line={}, end_char={}",
                    fragment.start_line, fragment.start_char, fragment.end_line, fragment.end_char);

            // If on same line, start_char should be < end_char
            if fragment.start_line == fragment.end_line {
                assert!(fragment.start_char < fragment.end_char,
                        "On same line, start_char ({}) should be < end_char ({})",
                        fragment.start_char, fragment.end_char);
            }
        }
    }

    #[test]
    fn test_same_line_multiple_elements() {
        // Create XML with multiple short elements on the same line
        let xml = r#"<?xml version="1.0"?>
<text><body><p rend="nikaya">Dīghanikāyo</p><div type="book"><head rend="book">Book1</head><div type="sutta"><head rend="chapter">Sutta1</head><p n="1">Text1</p></div></div></body></text>"#;

        let structure = detect_nikaya_structure(xml).unwrap();
        let fragments = parse_into_fragments(xml, &structure, "test.xml", &ParserOverrides::default(), false).unwrap();

        // Check that we can distinguish elements on the same line
        // by their character positions
        for i in 0..fragments.len() {
            for j in (i+1)..fragments.len() {
                let frag_i = &fragments[i];
                let frag_j = &fragments[j];

                // If both fragments are on the same line
                if frag_i.start_line == frag_j.start_line &&
                   frag_i.end_line == frag_j.end_line &&
                   frag_i.start_line == frag_i.end_line {
                    // They should have non-overlapping character ranges
                    let no_overlap = frag_i.end_char <= frag_j.start_char ||
                                    frag_j.end_char <= frag_i.start_char;
                    assert!(no_overlap,
                            "Fragments on same line should not overlap: \
                             frag[{}]: {}:{}-{}:{}, frag[{}]: {}:{}-{}:{}",
                            i, frag_i.start_line, frag_i.start_char,
                            frag_i.end_line, frag_i.end_char,
                            j, frag_j.start_line, frag_j.start_char,
                            frag_j.end_line, frag_j.end_char);
                }
            }
        }
    }

    #[test]
    fn test_cst_fields_dn() {
        // Test CST field derivation for DN
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<TEI.2>
<text>
<body>
<p rend="nikaya">Dīghanikāyo</p>
<div id="dn1" n="dn1" type="book">
<head rend="book">Sīlakkhandhavaggapāḷi</head>
<div id="dn1_1" n="dn1_1" type="sutta">
<head rend="chapter">1. Brahmajālasuttaṃ</head>
<p rend="subhead">Paribbājakakathā</p>
<p rend="bodytext" n="1">Evaṃ me sutaṃ</p>
</div>
</div>
</body>
</text>
</TEI.2>"#;

        let structure = detect_nikaya_structure(xml).unwrap();
        let fragments = parse_into_fragments(xml, &structure, "s0101m.mul.xml", &ParserOverrides::default(), false).unwrap();

        // Find the sutta fragment
        let sutta_frag = fragments.iter()
            .find(|f| matches!(f.frag_type, FragmentType::Sutta))
            .expect("Should have a sutta fragment");

        // Check CST fields
        assert_eq!(sutta_frag.cst_file.as_str(), "s0101m.mul.xml");
        assert_eq!(sutta_frag.cst_code.as_deref(), Some("dn1.1"));
        assert_eq!(sutta_frag.cst_vagga.as_deref(), None); // DN doesn't have vaggas
        assert_eq!(sutta_frag.cst_sutta.as_deref(), Some("1. Brahmajālasuttaṃ"));
        assert_eq!(sutta_frag.cst_paranum.as_deref(), Some("1"));
    }

    #[test]
    fn test_cst_fields_mn() {
        // Test CST field derivation for MN
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<TEI.2>
<text>
<body>
<p rend="nikaya">Majjhimanikāyo</p>
<div id="mn1" type="book">
<head rend="book">Mūlapaṇṇāsapāḷi</head>
<div id="mn1_5" n="mn1_5" type="vagga">
<head rend="chapter">5. Cūḷayamakavaggo</head>
<p rend="subhead">1. Sāleyyakasuttaṃ</p>
<p rend="bodytext" n="439">Evaṃ me sutaṃ</p>
</div>
</div>
</body>
</text>
</TEI.2>"#;

        let structure = detect_nikaya_structure(xml).unwrap();
        let fragments = parse_into_fragments(xml, &structure, "s0201m.mul.xml", &ParserOverrides::default(), false).unwrap();

        // Find the sutta fragment
        let sutta_frag = fragments.iter()
            .find(|f| matches!(f.frag_type, FragmentType::Sutta))
            .expect("Should have a sutta fragment");

        // Check CST fields
        assert_eq!(sutta_frag.cst_file.as_str(), "s0201m.mul.xml");
        assert_eq!(sutta_frag.cst_code.as_deref(), Some("mn1.5.1"));
        assert_eq!(sutta_frag.cst_vagga.as_deref(), Some("5. Cūḷayamakavaggo"));
        assert_eq!(sutta_frag.cst_sutta.as_deref(), Some("1. Sāleyyakasuttaṃ"));
        assert_eq!(sutta_frag.cst_paranum.as_deref(), Some("439"));
    }

    #[test]
    fn test_cst_fields_sn() {
        // Test CST field derivation for SN
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<TEI.2>
<text>
<body>
<p rend="nikaya">Saṃyuttanikāyo</p>
<div id="sn1" n="sn1" type="book">
<head rend="book">Sagāthāvaggo</head>
<div id="sn1_1" n="sn1_1" type="samyutta">
<head rend="chapter">1. Devatāsaṃyuttaṃ</head>
<p rend="title">1. Naḷavaggo</p>
<p rend="subhead">1. Oghataraṇasuttaṃ</p>
<p rend="bodytext" n="1">Evaṃ me sutaṃ – ekaṃ samayaṃ bhagavā sāvatthiyaṃ viharati</p>
</div>
</div>
</body>
</text>
</TEI.2>"#;

        let structure = detect_nikaya_structure(xml).unwrap();
        let fragments = parse_into_fragments(xml, &structure, "s0301m.mul.xml", &ParserOverrides::default(), false).unwrap();

        // Find the sutta fragment
        let sutta_frag = fragments.iter()
            .find(|f| matches!(f.frag_type, FragmentType::Sutta))
            .expect("Should have a sutta fragment");

        // Check CST fields
        assert_eq!(sutta_frag.cst_file.as_str(), "s0301m.mul.xml");
        assert_eq!(sutta_frag.cst_code.as_deref(), Some("sn1.1.1.1"));
        assert_eq!(sutta_frag.cst_vagga.as_deref(), Some("1. Naḷavaggo"));
        assert_eq!(sutta_frag.cst_sutta.as_deref(), Some("1. Oghataraṇasuttaṃ"));
        assert_eq!(sutta_frag.cst_paranum.as_deref(), Some("1"));
    }

    #[test]
    fn test_cst_fields_sn_vagga_with_11_suttas() {
        // Test that vaggas with more than 10 suttas are handled correctly
        // This is based on real XML from s0301m.mul.xml, vagga 8 (Chetvāvaggo) which has 11 suttas
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<TEI.2>
<text>
<body>
<p rend="nikaya">Saṃyuttanikāyo</p>
<div id="sn1" n="sn1" type="book">
<head rend="book">Sagāthāvaggo</head>
<div id="sn1_1" n="sn1_1" type="samyutta">
<head rend="chapter">1. Devatāsaṃyuttaṃ</head>
<p rend="title">8. Chetvāvaggo</p>
<p rend="subhead">10. Pajjotasuttaṃ</p>
<p rend="hangnum" n="80"><hi rend="paranum">80</hi></p>
<p rend="gatha1">''Kiṃsu lokasmi pajjoto, kiṃsu lokasmi jāgaro;</p>
<p rend="gathalast">Gāvo kamme sajīvānaṃ, sītassa iriyāpatho.</p>
<p rend="subhead">11. Araṇasuttaṃ</p>
<p rend="hangnum" n="81"><hi rend="paranum">81</hi></p>
<p rend="gatha1">''Kesūdha araṇā loke, kesaṃ vusitaṃ na nassati;</p>
<p rend="gathalast">Samaṇīdha jātihīnaṃ, abhivādenti khattiyā''ti.</p>
<p rend="centre">Chetvāvaggo aṭṭhamo.</p>
<p rend="bodytext">Tassuddānaṃ –</p>
<p rend="gatha1">Chetvā rathañca cittañca, vuṭṭhi bhītā najīrati;</p>
<p rend="gathalast">Issaraṃ kāmaṃ pātheyyaṃ, pajjoto araṇena cāti.</p>
<trailer rend="centre">Devatāsaṃyuttaṃ samattaṃ.</trailer>
</div>
<div id="sn1_2" n="sn1_2" type="samyutta">
<head rend="chapter">2. Devaputtasaṃyuttaṃ</head>
<p rend="title">1. Paṭhamavaggo</p>
<p rend="subhead">1. Paṭhamakassapasuttaṃ</p>
<p rend="bodytext" n="82">Evaṃ me sutaṃ</p>
</div>
</div>
</body>
</text>
</TEI.2>"#;

        let structure = detect_nikaya_structure(xml).unwrap();
        let fragments = parse_into_fragments(xml, &structure, "s0301m.mul.xml", &ParserOverrides::default(), false).unwrap();

        let sutta_fragments: Vec<_> = fragments.iter()
            .filter(|f| matches!(f.frag_type, FragmentType::Sutta))
            .collect();

        // Find sutta 10
        let sutta10 = sutta_fragments.iter()
            .find(|f| f.cst_sutta.as_ref().map(|s| s.contains("Pajjotasuttaṃ")).unwrap_or(false))
            .expect("Should find sutta 10");

        assert_eq!(sutta10.cst_code.as_deref(), Some("sn1.1.8.10"));
        assert_eq!(sutta10.cst_vagga.as_deref(), Some("8. Chetvāvaggo"));

        // Find sutta 11 - this is the critical test case
        let sutta11 = sutta_fragments.iter()
            .find(|f| f.cst_sutta.as_ref().map(|s| s.contains("Araṇasuttaṃ")).unwrap_or(false))
            .expect("Should find sutta 11");

        // Sutta 11 should have correct code even though it's the 11th sutta in a vagga
        // and is followed by a new samyutta div
        assert_eq!(sutta11.cst_code.as_deref(), Some("sn1.1.8.11"));
        assert_eq!(sutta11.cst_vagga.as_deref(), Some("8. Chetvāvaggo"));
        assert_eq!(sutta11.cst_sutta.as_deref(), Some("11. Araṇasuttaṃ"));

        // Verify that the samyutta level is still sn1_1, not sn1_2
        let samyutta_level = sutta11.group_levels.iter()
            .find(|l| matches!(l.group_type, GroupType::Samyutta));
        assert!(samyutta_level.is_some());
        assert_eq!(samyutta_level.unwrap().id.as_deref(), Some("sn1_1"));
    }

    #[test]
    fn test_cst_fields_an() {
        // Test CST field derivation for AN (mul file with div IDs)
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<TEI.2>
<text>
<body>
<p rend="nikaya">Aṅguttaranikāyo</p>
<div id="an3" n="an3" type="book">
<head rend="book">Tikanipātapāḷi</head>
<div id="an3_1" n="an3_1" type="pannasaka">
<head rend="title">1. Paṭhamapaṇṇāsakaṃ</head>
<div id="an3_1_1" n="an3_1_1" type="vagga">
<head rend="chapter">1. Bālavaggo</head>
<p rend="subhead">1. Bhayasuttaṃ</p>
<p rend="bodytext" n="1">Evaṃ me sutaṃ – ekaṃ samayaṃ bhagavā sāvatthiyaṃ viharati</p>
</div>
</div>
</div>
</body>
</text>
</TEI.2>"#;

        let structure = detect_nikaya_structure(xml).unwrap();
        let fragments = parse_into_fragments(xml, &structure, "s0402m2.mul.xml", &ParserOverrides::default(), false).unwrap();

        // Find the sutta fragment
        let sutta_frag = fragments.iter()
            .find(|f| matches!(f.frag_type, FragmentType::Sutta))
            .expect("Should have a sutta fragment");

        // Check CST fields
        assert_eq!(sutta_frag.cst_file.as_str(), "s0402m2.mul.xml");
        assert_eq!(sutta_frag.cst_code.as_deref(), Some("an3.1.1.1"));
        assert_eq!(sutta_frag.cst_vagga.as_deref(), Some("1. Bālavaggo"));
        assert_eq!(sutta_frag.cst_sutta.as_deref(), Some("1. Bhayasuttaṃ"));
        assert_eq!(sutta_frag.cst_paranum.as_deref(), Some("1"));
    }

    #[test]
    fn test_cst_fields_an_tika() {
        // Test CST field derivation for AN tika/commentary files
        // These use <p> tags instead of <div> tags for pannasaka and vagga
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<TEI.2>
<text>
<body>
<div id="an2" n="an2" type="book">
<p rend="nikaya">Aṅguttaranikāye</p>
<head rend="book">Dukanipāta-ṭīkā</head>
<p rend="title">1. Paṭhamapaṇṇāsakaṃ</p>
<p rend="chapter">1. Kammakāraṇavaggo</p>
<p rend="subhead">1. Vajjasuttavaṇṇanā</p>
<p rend="bodytext" n="1"><hi rend="paranum">1</hi>Dukanipātassa paṭhame</p>
<p rend="subhead">2. Padhānasuttavaṇṇanā</p>
<p rend="bodytext" n="2"><hi rend="paranum">2</hi>Dutiye</p>
</div>
</body>
</text>
</TEI.2>"#;

        let structure = detect_nikaya_structure(xml).unwrap();
        let fragments = parse_into_fragments(xml, &structure, "s0402t.tik.xml", &ParserOverrides::default(), false).unwrap();

        let sutta_fragments: Vec<_> = fragments.iter()
            .filter(|f| matches!(f.frag_type, FragmentType::Sutta))
            .collect();

        // Should have at least 2 sutta fragments
        assert!(sutta_fragments.len() >= 2, "Should have at least 2 sutta fragments");

        // Find first actual sutta (not preamble)
        let sutta1 = sutta_fragments.iter()
            .find(|f| f.cst_code.is_some() && f.cst_sutta.as_ref().map(|s| s.contains("Vajjasuttavaṇṇanā")).unwrap_or(false))
            .expect("Should find first sutta");

        // Check CST fields for first sutta
        assert_eq!(sutta1.cst_code.as_deref(), Some("an2.1.1.1"));
        assert_eq!(sutta1.cst_vagga.as_deref(), Some("1. Kammakāraṇavaggo"));
        assert_eq!(sutta1.cst_sutta.as_deref(), Some("1. Vajjasuttavaṇṇanā"));

        // Find second sutta
        let sutta2 = sutta_fragments.iter()
            .find(|f| f.cst_sutta.as_ref().map(|s| s.contains("Padhānasuttavaṇṇanā")).unwrap_or(false))
            .expect("Should find second sutta");

        // Check CST fields for second sutta
        assert_eq!(sutta2.cst_code.as_deref(), Some("an2.1.1.2"));
        assert_eq!(sutta2.cst_vagga.as_deref(), Some("1. Kammakāraṇavaggo"));
        assert_eq!(sutta2.cst_sutta.as_deref(), Some("2. Padhānasuttavaṇṇanā"));
    }

    #[test]
    fn test_cst_fields_an_tika_multi_vagga() {
        // Test that AN tika files properly split fragments on <p rend="chapter"> Vagga boundaries
        // This regression test ensures we don't have a single fragment spanning multiple vaggas
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<TEI.2>
<text>
<body>
<div id="an2" n="an2" type="book">
<p rend="nikaya">Aṅguttaranikāye</p>
<head rend="book">Dukanipāta-ṭīkā</head>
<p rend="title">1. Paṭhamapaṇṇāsakaṃ</p>
<p rend="chapter">1. Kammakāraṇavaggo</p>
<p rend="subhead">1. Vajjasuttavaṇṇanā</p>
<p rend="bodytext" n="1"><hi rend="paranum">1</hi>First vagga first sutta content</p>
<p rend="chapter">2. Adhikaraṇavaggavaṇṇanā</p>
<p rend="bodytext" n="11"><hi rend="paranum">11</hi>Second vagga content without sutta subhead</p>
<p rend="chapter">3. Bālavaggavaṇṇanā</p>
<p rend="bodytext" n="22"><hi rend="paranum">22</hi>Third vagga content without sutta subhead</p>
</div>
</body>
</text>
</TEI.2>"#;

        let structure = detect_nikaya_structure(xml).unwrap();
        let fragments = parse_into_fragments(xml, &structure, "s0402t.tik.xml", &ParserOverrides::default(), false).unwrap();

        let sutta_fragments: Vec<_> = fragments.iter()
            .filter(|f| matches!(f.frag_type, FragmentType::Sutta))
            .collect();

        // Should have at least 3 fragments (one for each vagga)
        assert!(sutta_fragments.len() >= 3, "Should have at least 3 sutta fragments, got {}", sutta_fragments.len());

        // Verify each fragment contains at most 1 chapter marker (its own vagga title)
        for frag in &sutta_fragments {
            let chapter_count = frag.content_xml.matches("rend=\"chapter\"").count();
            assert!(chapter_count <= 1,
                "Fragment should contain at most 1 chapter marker, found {} in fragment with vagga: {:?}",
                chapter_count, frag.cst_vagga);
        }

        // Find fragment with first vagga
        let vagga1_frag = sutta_fragments.iter()
            .find(|f| f.cst_vagga.as_ref().map(|v| v.contains("Kammakāraṇavaggo")).unwrap_or(false))
            .expect("Should find fragment with first vagga");

        assert_eq!(vagga1_frag.cst_code.as_deref(), Some("an2.1.1.1"));
        assert_eq!(vagga1_frag.cst_vagga.as_deref(), Some("1. Kammakāraṇavaggo"));
        assert_eq!(vagga1_frag.cst_sutta.as_deref(), Some("1. Vajjasuttavaṇṇanā"));

        // Find fragment with second vagga
        let vagga2_frag = sutta_fragments.iter()
            .find(|f| f.cst_vagga.as_ref().map(|v| v.contains("Adhikaraṇavaggavaṇṇanā")).unwrap_or(false))
            .expect("Should find fragment with second vagga");

        assert_eq!(vagga2_frag.cst_code.as_deref(), Some("an2.1.2.0"));
        assert_eq!(vagga2_frag.cst_vagga.as_deref(), Some("2. Adhikaraṇavaggavaṇṇanā"));
        assert_eq!(vagga2_frag.cst_sutta, None); // No sutta subhead

        // Find fragment with third vagga
        let vagga3_frag = sutta_fragments.iter()
            .find(|f| f.cst_vagga.as_ref().map(|v| v.contains("Bālavaggavaṇṇanā")).unwrap_or(false))
            .expect("Should find fragment with third vagga");

        assert_eq!(vagga3_frag.cst_code.as_deref(), Some("an2.1.3.0"));
        assert_eq!(vagga3_frag.cst_vagga.as_deref(), Some("3. Bālavaggavaṇṇanā"));
        assert_eq!(vagga3_frag.cst_sutta, None); // No sutta subhead
    }
}
