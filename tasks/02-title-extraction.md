# Plan 02: Extract extract_sutta_title_from_content to helpers.rs

Status: **TODO**

## Implementation Overview

**Risk Level**: 🟢 **Very Low** - 100% identical across all files

**Scope**: Move the `extract_sutta_title_from_content` function from all 13 parser files to `src/parsers/helpers.rs`.

**Why This Is Safe**:
- Confirmed 100% identical across DN, MN, SN, AN, and all text types (mula, atthakatha, tika)
- Pure function with no side effects
- Self-contained string parsing logic with no nikaya-specific behavior
- Already tested and working

**Estimated Impact**:
- Lines removed: ~1,040 lines (80 lines × 13 files)
- Lines added: ~80 lines (in helpers.rs)
- Net reduction: ~960 lines

## Current State

The `extract_sutta_title_from_content` function appears in every parser file at:
- Lines 366-445 in DN/MN/AN
- Lines 376-455 in SN (offset by 10 lines due to code differences above)

```rust
/// Extract sutta title from <head> or <p rend="subhead"> tag in fragment content
/// Prefers <p rend="subhead"> over <head rend="chapter"> to avoid extracting vagga titles
fn extract_sutta_title_from_content(content: &str) -> Option<String> {
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
```

## Implementation Steps

### Step 1: Add extract_sutta_title_from_content to helpers.rs

**File**: `src/parsers/helpers.rs`

Add after the existing `extract_vagga_title_from_content` function:

```rust
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
```

**Note**: Changed visibility from `fn` to `pub fn`.

### Step 2: Update imports in each parser file

The function is already being imported via the wildcard from helpers, but we should verify the import group:

**No change needed to imports** - the function will be available via the existing import.

### Step 3: Remove duplicate definitions

**Remove the function definition** (lines 366-445 or 376-455) from each of these files:
- `src/parsers/digha_nikaya_mula.rs`
- `src/parsers/digha_nikaya_atthakatha.rs`
- `src/parsers/digha_nikaya_tika.rs`
- `src/parsers/majjhima_nikaya_mula.rs`
- `src/parsers/majjhima_nikaya_atthakatha.rs`
- `src/parsers/majjhima_nikaya_tika.rs`
- `src/parsers/samyutta_nikaya_mula.rs`
- `src/parsers/samyutta_nikaya_atthakatha.rs`
- `src/parsers/samyutta_nikaya_tika.rs`
- `src/parsers/anguttara_nikaya_mula.rs`
- `src/parsers/anguttara_nikaya_atthakatha.rs`
- `src/parsers/anguttara_nikaya_tika.rs`
- `src/parsers/general.rs`

### Step 4: Verify compilation

```bash
cargo check
```

### Step 5: Run tests

```bash
cargo test
```

## Task List

- [ ] Add `extract_sutta_title_from_content` function to `src/parsers/helpers.rs`
- [ ] Remove function from `src/parsers/digha_nikaya_mula.rs`
- [ ] Remove function from `src/parsers/digha_nikaya_atthakatha.rs`
- [ ] Remove function from `src/parsers/digha_nikaya_tika.rs`
- [ ] Remove function from `src/parsers/majjhima_nikaya_mula.rs`
- [ ] Remove function from `src/parsers/majjhima_nikaya_atthakatha.rs`
- [ ] Remove function from `src/parsers/majjhima_nikaya_tika.rs`
- [ ] Remove function from `src/parsers/samyutta_nikaya_mula.rs`
- [ ] Remove function from `src/parsers/samyutta_nikaya_atthakatha.rs`
- [ ] Remove function from `src/parsers/samyutta_nikaya_tika.rs`
- [ ] Remove function from `src/parsers/anguttara_nikaya_mula.rs`
- [ ] Remove function from `src/parsers/anguttara_nikaya_atthakatha.rs`
- [ ] Remove function from `src/parsers/anguttara_nikaya_tika.rs`
- [ ] Remove function from `src/parsers/general.rs`
- [ ] Run `cargo check` to verify compilation
- [ ] Run `cargo test` to verify all tests pass

## Rollback Plan

If issues are encountered:
1. Revert changes to helpers.rs
2. Restore the function definition in each parser file where it was removed

## Verification Checklist

- [ ] All 13 parser files compile successfully
- [ ] All tests pass
- [ ] Function behavior is identical to before refactoring
- [ ] No duplicate function definitions remain in parser files

## Notes

- This function is used by `derive_cst_fields` in each parser
- The function is called with `extract_sutta_title_from_content(&fragment.content_xml)`
- The logic is self-contained and doesn't depend on nikaya-specific behavior
- The comments already explain the edge cases (DN vs AN tika handling)
