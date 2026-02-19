//! Test for SN Atthakatha samyutta fragments without subheads
//!
//! Issue: In s0303a.att.xml, samyuttas sn3_5 (Uppādasaṃyuttavaṇṇanā) and
//! sn3_6 (Kilesasaṃyuttavaṇṇanā) have no <p rend="subhead"> elements.
//! They go directly from <head rend="chapter"> to <p rend="bodytext">.
//! Previously, the parser only split fragments when a subhead was encountered,
//! so these samyuttas were merged into frag_idx 98 (sn3_4's fragment).
//!
//! Each samyutta should produce its own fragment even without a subhead.

use std::fs;
use tipitaka_xml_parser::{
    nikaya_detector::detect_nikaya_structure,
    parsers::samyutta_nikaya_commentary::parse_into_fragments,
    types::{FragmentType, ParserOverrides},
};

#[test]
fn test_s0303a_att_samyutta_without_subhead_split() {
    let xml_path = "tests/data/s0303a.att.xml";
    let xml_content = fs::read_to_string(xml_path)
        .expect("Failed to read s0303a.att.xml");

    let structure = detect_nikaya_structure(&xml_content)
        .expect("Should detect SN structure");

    assert_eq!(structure.nikaya, "samyutta");

    let fragments = parse_into_fragments(&xml_content, &structure, "s0303a.att.xml", &ParserOverrides::default(), false)
        .expect("Should parse fragments");

    // Find fragments containing each samyutta's content
    let sn3_4_frags: Vec<_> = fragments.iter()
        .filter(|f| matches!(f.frag_type, FragmentType::Sutta) &&
                f.content_xml.contains("Okkantasaṃyutt"))
        .collect();

    let sn3_5_frags: Vec<_> = fragments.iter()
        .filter(|f| matches!(f.frag_type, FragmentType::Sutta) &&
                f.content_xml.contains("Uppādasaṃyutt"))
        .collect();

    let sn3_6_frags: Vec<_> = fragments.iter()
        .filter(|f| matches!(f.frag_type, FragmentType::Sutta) &&
                f.content_xml.contains("Kilesasaṃyutt"))
        .collect();

    let sn3_7_frags: Vec<_> = fragments.iter()
        .filter(|f| matches!(f.frag_type, FragmentType::Sutta) &&
                f.content_xml.contains("Sāriputtasaṃyutt"))
        .collect();

    // Each samyutta should be in its own fragment (not merged together)
    assert!(!sn3_4_frags.is_empty(),
        "Should have a fragment for sn3_4 (Okkantasaṃyuttaṃ)");
    assert!(!sn3_5_frags.is_empty(),
        "Should have a fragment for sn3_5 (Uppādasaṃyuttavaṇṇanā)");
    assert!(!sn3_6_frags.is_empty(),
        "Should have a fragment for sn3_6 (Kilesasaṃyuttavaṇṇanā)");
    assert!(!sn3_7_frags.is_empty(),
        "Should have a fragment for sn3_7 (Sāriputtasaṃyuttaṃ)");

    // Critically: sn3_5 and sn3_6 should NOT be in the same fragment as sn3_4
    let sn3_4_frag = sn3_4_frags[0];
    assert!(!sn3_4_frag.content_xml.contains("Uppādasaṃyutt"),
        "sn3_4 fragment should NOT contain sn3_5 content (Uppādasaṃyuttavaṇṇanā).\n\
         frag_idx_code: {}", sn3_4_frag.frag_idx_code);
    assert!(!sn3_4_frag.content_xml.contains("Kilesasaṃyutt"),
        "sn3_4 fragment should NOT contain sn3_6 content (Kilesasaṃyuttavaṇṇanā).\n\
         frag_idx_code: {}", sn3_4_frag.frag_idx_code);

    // sn3_5 fragment should not contain sn3_6 content
    let sn3_5_frag = sn3_5_frags[0];
    assert!(!sn3_5_frag.content_xml.contains("Kilesasaṃyutt"),
        "sn3_5 fragment should NOT contain sn3_6 content (Kilesasaṃyuttavaṇṇanā).\n\
         frag_idx_code: {}", sn3_5_frag.frag_idx_code);

    // Verify fragment indices are sequential (sn3_4 < sn3_5 < sn3_6 < sn3_7)
    // Note: frag_idx_code is a String like "99.0", so we must compare numerically
    let parse_idx = |s: &str| -> f64 { s.parse().unwrap_or(0.0) };

    assert!(parse_idx(&sn3_4_frag.frag_idx_code) < parse_idx(&sn3_5_frag.frag_idx_code),
        "sn3_4 frag_idx_code ({}) should be before sn3_5 frag_idx_code ({})",
        sn3_4_frag.frag_idx_code, sn3_5_frag.frag_idx_code);
    assert!(parse_idx(&sn3_5_frag.frag_idx_code) < parse_idx(&sn3_6_frags[0].frag_idx_code),
        "sn3_5 frag_idx_code ({}) should be before sn3_6 frag_idx_code ({})",
        sn3_5_frag.frag_idx_code, sn3_6_frags[0].frag_idx_code);
    assert!(parse_idx(&sn3_6_frags[0].frag_idx_code) < parse_idx(&sn3_7_frags[0].frag_idx_code),
        "sn3_6 frag_idx_code ({}) should be before sn3_7 frag_idx_code ({})",
        sn3_6_frags[0].frag_idx_code, sn3_7_frags[0].frag_idx_code);
}

#[test]
fn test_sn_att_samyutta_no_subhead_synthetic() {
    // Synthetic test to verify samyuttas without subheads get their own fragments
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<TEI.2>
<text>
<body>
<p rend="nikaya">Saṃyuttanikāye</p>
<div id="sn3" n="sn3" type="book">
<head rend="book">Khandhavagga-aṭṭhakathā</head>
<div id="sn3_1" n="sn3_1" type="samyutta">
<head rend="chapter">1. FirstSaṃyuttaṃ</head>
<p rend="subhead">1. Firstsuttavaṇṇanā</p>
<p rend="bodytext" n="1">First sutta content.</p>
</div>
<div id="sn3_2" n="sn3_2" type="samyutta">
<head rend="chapter">2. SecondSaṃyuttavaṇṇanā</head>
<p rend="bodytext" n="2-10">Second samyutta content, no subhead.</p>
</div>
<div id="sn3_3" n="sn3_3" type="samyutta">
<head rend="chapter">3. ThirdSaṃyuttavaṇṇanā</head>
<p rend="bodytext" n="11-20">Third samyutta content, no subhead.</p>
</div>
<div id="sn3_4" n="sn3_4" type="samyutta">
<head rend="chapter">4. FourthSaṃyuttaṃ</head>
<p rend="subhead">1. Fourthsuttavaṇṇanā</p>
<p rend="bodytext" n="21">Fourth sutta content.</p>
</div>
</div>
</body>
</text>
</TEI.2>"#;

    let structure = detect_nikaya_structure(xml).expect("Should detect SN structure");
    let fragments = parse_into_fragments(xml, &structure, "s0303a.att.xml", &ParserOverrides::default(), false)
        .expect("Should parse fragments");

    let sutta_fragments: Vec<_> = fragments.iter()
        .filter(|f| matches!(f.frag_type, FragmentType::Sutta))
        .collect();

    // We should have at least 4 sutta fragments (one per samyutta)
    // sn3_1 has a subhead so it's the first fragment
    // sn3_2 has no subhead but should still be its own fragment
    // sn3_3 has no subhead but should still be its own fragment
    // sn3_4 has a subhead so it starts a new fragment
    assert!(sutta_fragments.len() >= 4,
        "Should have at least 4 sutta fragments, got {}. Fragments:\n{}",
        sutta_fragments.len(),
        sutta_fragments.iter()
            .map(|f| format!("  frag_idx_code {}: {}", f.frag_idx_code,
                &f.content_xml[..100.min(f.content_xml.len())]))
            .collect::<Vec<_>>().join("\n"));

    // Each samyutta should be in a separate fragment
    let frag_with_second = sutta_fragments.iter()
        .find(|f| f.content_xml.contains("SecondSaṃyuttavaṇṇanā"))
        .expect("Should find fragment with second samyutta");
    assert!(!frag_with_second.content_xml.contains("ThirdSaṃyuttavaṇṇanā"),
        "Second samyutta fragment should not contain third samyutta content");

    let frag_with_third = sutta_fragments.iter()
        .find(|f| f.content_xml.contains("ThirdSaṃyuttavaṇṇanā"))
        .expect("Should find fragment with third samyutta");
    assert!(!frag_with_third.content_xml.contains("FourthSaṃyuttaṃ"),
        "Third samyutta fragment should not contain fourth samyutta content");
}
