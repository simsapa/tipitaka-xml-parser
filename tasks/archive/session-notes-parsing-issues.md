# Parsing Issues - Session Notes

## Overview

This document describes the parsing issues investigated and fixed in a development session focused on the tipitaka-xml-parser project.

---

## Issue 1: Subhead Without Space After Number

### Problem

In `s0303m.mul.xml`, frag_idx 177 was not splitting correctly. It included two suttas:

```xml
<p rend="subhead">8. Anattadhammasuttaṃ</p>
<p rend="bodytext" n="177"> ... </p>

<p rend="subhead">9.Khayadhammasuttaṃ</p>
<p rend="bodytext" n="178"> ... </p>
```

The problem was that there was no space after "9." in the subhead element. This caused the parser to not recognize "9.Khayadhammasuttaṃ" as a numbered sutta subhead, so it wasn't being detected as a fragment boundary.

### Root Cause

The `is_numbered_sutta_subhead` function in `src/parsers/helpers.rs` only handled the format "number. title" (with space after the dot):

```rust
pub fn is_numbered_sutta_subhead(text: &str) -> bool {
    text.split_whitespace()
        .next()
        .and_then(|first_word| first_word.strip_suffix('.'))
        .map_or(false, |num_part| {
            num_part.split('-').all(|part| !part.is_empty() && part.chars().all(|c| c.is_numeric()))
        })
}
```

This would fail for "9.Khayadhammasuttaṃ" because it splits on whitespace first, getting "9.Khayadhammasuttaṃ" as the first word, and "9.Khayadhammasuttaṃ".strip_suffix('.') returns None.

### Solution

Added a fallback to manually parse the string character by character to find digits followed by a dot, without requiring whitespace:

```rust
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
            if end_pos > 0 {
                let num_str = &text[..end_pos];
                if num_str.split('-').all(|part| !part.is_empty() && part.chars().all(|c| c.is_numeric())) {
                    return true;
                }
            }
            break;
        } else {
            break;
        }
    }

    false
}
```

### Tests Added

- Unit test `test_is_numbered_sutta_subhead` in `src/parsers/helpers.rs`
- Updated `test_s0303m_sc_code_propagation_sn9_to_sn10` with shifted frag_idx values (+1 due to new fragment)

---

## Issue 2: CST Code Range Not Converting to SC Code Range (TSV Lookup)

### Problem

For `s0303m.mul.xml` frag_idx 182, the title in content is:

```xml
<p rend="subhead">1-11. Mārādisuttaekādasakaṃ</p>
```

This is correctly parsed to cst_code `sn3.2.3.1-11`, but the sc_code was being parsed to `sn23.23` instead of `sn23.23-33`.

### Root Cause

The `populate_sc_fields_from_tsv_conditional` function looked up the cst_code in the TSV map and got back the sc_code, but it wasn't converting the sc_code to range format when the cst_code was a range.

The TSV has an entry:
```
sn3.2.3.1-11 → sn23.23 (not sn23.23-33)
```

When the cst_code is a range like `sn3.2.3.1-11`, the sc_code returned from TSV lookup (`sn23.23`) needs to be converted to range format (`sn23.23-33`).

### Solution

Added two helper functions:

1. `is_cst_code_range(cst_code: &str) -> bool` - detects if cst_code contains a range (e.g., "1-11")
2. `convert_sc_code_to_range(sc_code: &str, cst_code: &str) -> String` - converts sc_code to range format based on cst_code range

Updated `populate_sc_fields_from_tsv_conditional` to convert sc_code to range when cst_code is a range:

```rust
if let Some((sc_code, sc_sutta)) = tsv_map.get(cst_code) {
    // If cst_code is a range, convert sc_code to range format
    let sc_code = if is_cst_code_range(cst_code) {
        convert_sc_code_to_range(sc_code, cst_code)
    } else {
        sc_code.clone()
    };
    fragment.sc_code = Some(sc_code);
    fragment.sc_sutta = Some(sc_sutta.clone());
}
```

### Tests Added

- Test `test_s0303m_range_cst_code_to_sc_code` verifying frag_idx 182 gets sc_code "sn23.23-33"

---

## Issue 3: Propagation with Range CST Codes (ArangoDB Lookup)

### Problem

For `s0303t.tik.xml` frag_idx 95, the cst_code is `sn3.4.1.1-10` and content has:

```xml
<p rend="subhead">1-10. Cakkhusuttādivaṇṇanā</p>
```

But the sc_code was `sn25.1` instead of `sn25.1-10`.

The TSV doesn't have a mapping for `sn3.4.1.1-10` (it only has mappings for `sn3.4.4.x`), so the sc_code must be derived from the previous fragment and converted to range format.

### Root Cause

The `propagate_sc_codes_from_previous` function was:
1. Deriving the sc_code correctly (e.g., `sn25.1-10`)
2. Converting to range format correctly (using `convert_sc_code_to_range`)
3. But then looking up in TSV using the wrong key (derived_sc instead of sc_code_to_assign)
4. When ArangoDB pali_titles was available, it was falling back to non-range base because the range version wasn't found

### Solution

1. Fixed TSV lookup to use `sc_code_to_assign` (the range version) instead of `derived_sc`:
```rust
// Before (wrong):
if let Some((_, sc_sutta)) = tsv_map.get(&derived_sc) {

// After (correct):
if let Some((_, sc_sutta)) = tsv_map.get(&sc_code_to_assign) {
```

2. Fixed ArangoDB lookup to preserve the range sc_code when cst_code is a range:
```rust
let is_range = is_cst_code_range(&current_cst_code);

if is_range {
    // For range cst_codes, use the derived range sc_code directly
    // and try to get title from pali_titles
    if let Some(titles_cache) = pali_titles {
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
    }
}
```

### Tests Added

- Test `test_s0303t_tik_range_cst_code_propagation` verifying frag_idx 95 gets sc_code "sn25.1-10"

---

## Issue 4: Derivation Not Passing Range End

### Related Problem

When deriving sc_code from previous fragment, the range end from the cst_code was not being passed through to `derive_sc_code_with_components`, causing incorrect range derivation.

### Solution

Updated `derive_sc_code_from_previous` to extract and pass the range_end:

```rust
let (cst_code_base, range_end) = if let Some(dash_pos) = cst_code.rfind('-') {
    let base_part = &cst_code[..dash_pos];
    if base_part.rsplit('.').next().map_or(false, |s| s.chars().all(|c| c.is_ascii_digit())) {
        let end_str = &cst_code[dash_pos + 1..];
        let end = end_str.parse::<i32>().ok();
        (base_part.to_string(), end)
    } else {
        (cst_code.to_string(), None)
    }
} else {
    (cst_code.to_string(), None)
};
```

And updated `derive_sc_code_with_components` to use the range_end to generate range sc_code:

```rust
if cst_sutta == 1 && (prev_sutta > 1 || prev_sutta > 0) {
    if let Some(end) = range_end {
        return Some(format!("sn{}.{}-{}", prev_samyutta + 1, cst_sutta, end));
    }
    return Some(format!("sn{}.{}", prev_samyutta + 1, cst_sutta));
}
```

---

## Refactoring: Shared Helper Functions

### Duplicate Code Found

The `is_cst_code_range` function existed in two places:
- `src/parsers/helpers.rs` (private)
- `src/web/validation.rs` (private, different implementation)

### Solution

- Made `is_cst_code_range` public in `helpers.rs`
- Removed duplicate from `validation.rs`
- Updated `validation.rs` to import from `parsers::helpers`

---

## Summary of Files Modified

1. `src/parsers/helpers.rs`:
   - Added fallback logic to `is_numbered_sutta_subhead`
   - Made `is_cst_code_range` public
   - Added `convert_sc_code_to_range` function
   - Updated `populate_sc_fields_from_tsv_conditional` to convert sc_code to range
   - Updated `derive_sc_code_from_previous` to pass range_end
   - Updated `propagate_sc_codes_from_previous` to handle range cst_codes

2. `src/web/validation.rs`:
   - Removed duplicate `is_cst_code_range` function
   - Added import for shared function

3. `tests/test_s0303m_sc_code_propagation.rs`:
   - Updated test with shifted frag_idx values
   - Added test for range cst_code to sc_code conversion
   - Added test for tika range propagation
