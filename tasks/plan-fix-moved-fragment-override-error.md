# Plan: Fix Moved Fragment Boundary Override Error

## Problem

When regenerating the whole database using "Regenerate Using Current DB as Reference", the parser crashes with:

```
Error exporting fragments from "s0402m1.mul.xml": Invalid boundary override:
end position (5714) is before fragment start position (11937)
  File: s0402m1.mul.xml
  Fragment index: 2
  Override: end_line=64, end_char=0
```

### Root Cause

The `extract_checked_overrides()` / `extract_all_checked_overrides()` functions query fragments where `frag_review NOT IN (NULL, '', 'unchecked')`. This includes **"moved" fragments**, which have stale boundary values.

When a fragment is "moved" (via the move-fragment UI operation):
- Its content is emptied
- Its metadata (cst_code, sc_code, etc.) is cleared
- Its `end_line`/`end_char` retain the **original values** from before the move
- Its `frag_review` is set to `"moved"`

During a fresh reparse, the parser creates fragments from the XML. The "moved" fragment's stale `end_line`/`end_char` from the DB is picked up as a boundary override and applied to the freshly parsed fragment at the same `frag_idx`. The stale end position can be **before** the fragment's natural start position in the fresh parse, causing the `InvalidBoundaryOverride` error.

### The "Moved" Fragment Semantics

"Moved" fragments indicate that the content of that fragment has been moved to an adjacent (typically next) fragment:

- There can be **multiple successive** "moved" fragments
- The final "checked" fragment in the sequence collects the XML data from the preceding "moved" fragments
- After a move: the moved fragment has empty content, and the checked fragment has expanded boundaries covering both

During reparse, the correct behavior for "moved" fragments is to **collapse** them to zero-width (end = start), so the checked fragment that follows absorbs their content naturally.

## Solution Overview

1. **Rename** `CheckedFragmentOverrides` to `CorrectionFragmentOverrides` throughout the codebase
2. **Add collapse semantics** for "moved" fragments: instead of applying stale boundary overrides, signal the parser to make the fragment zero-width
3. **Keep "moved" fragments in the override map** so they participate correctly during parsing

### How Collapse Works

When a fragment is collapsed (end = start):
- The fragment's content becomes empty: `xml_content[start..start] = ""`
- The **next** fragment starts at the collapsed fragment's start position (since parsers use the overridden end position as the next fragment's start)
- The "checked" fragment after one or more collapsed "moved" fragments naturally absorbs all the moved content

This matches the user's intent: moved fragments are empty, and the checked fragment has the merged content.

For multiple consecutive moved fragments (N, N+1, ..., N+k) followed by a checked fragment (N+k+1):
- Fragment N: collapse → empty, next starts at N's start
- Fragment N+1: collapse → empty, next starts at N's start (same position)
- ...
- Fragment N+k+1 (checked): starts at N's start, ends at its overridden end → absorbs all content

## Detailed Changes

### 0. Null Out Boundary Data on Move (`src/fragment_operations.rs`)

**Defense-in-depth**: When the user clicks "Empty and move content to previous/next", the move operation currently empties content and clears metadata but **preserves the original boundary values** (`start_line`, `start_char`, `end_line`, `end_char`). These stale boundaries are the direct source of the override error.

The move operation (`move_fragment_content()`, lines 164-176) should null out the boundary fields for the moved (source) fragment:

```rust
// Current code preserves stale boundaries:
let current_content_update = UpdateFragmentBoundary {
    start_line: current_fragment.start_line,  // stale
    start_char: current_fragment.start_char,  // stale
    end_line: current_fragment.end_line,      // stale
    end_char: current_fragment.end_char,      // stale
    content_xml: String::new(),
};

// New code nulls them out:
let current_content_update = UpdateFragmentBoundary {
    start_line: 0,
    start_char: 0,
    end_line: 0,
    end_char: 0,
    content_xml: String::new(),
};
```

This ensures that even if the override extraction code ever reads the boundary values of a moved fragment, they won't produce misleading positions. Combined with the `collapse: true` flag in the override struct (described below), this provides two layers of protection.

**Note on nullable boundaries**: The schema stores `start_line`/`end_line` as non-nullable integers. Setting them to `0` is used as a sentinel value meaning "no valid boundary". This is consistent with 1-indexed line numbers (a valid line is always >= 1). If the schema supports nullable integers for these fields, `None`/`NULL` would be preferable; otherwise `0` is the correct sentinel.

### 1. Types (`src/types.rs`)

**Rename and extend the override struct:**

```rust
// Rename: CheckedFragmentOverride → CorrectionFragmentOverride
#[derive(Debug, Clone, Default)]
pub struct CorrectionFragmentOverride {
    /// If true, collapse this fragment to zero-width (for "moved" fragments).
    /// The parser will set end = start, producing an empty fragment.
    pub collapse: bool,
    /// Override end line (1-indexed). Applied during fragment finalization.
    /// Ignored when collapse is true.
    pub end_line: Option<usize>,
    /// Override end character position (0-indexed). Applied during fragment finalization.
    /// Ignored when collapse is true.
    pub end_char: Option<usize>,
    /// Override SC reference code (e.g., "sn5.1"). Applied in post-processing.
    pub sc_code: Option<String>,
    /// Override SC sutta name. Applied in post-processing.
    pub sc_sutta: Option<String>,
}

// Rename: CheckedFragmentOverrides → CorrectionFragmentOverrides
pub type CorrectionFragmentOverrides = HashMap<FragmentKey, CorrectionFragmentOverride>;
```

**Update `ParserOverrides`:**

```rust
pub struct ParserOverrides {
    pub adjustments: Option<FragmentAdjustments>,
    // Rename: checked_overrides → correction_overrides
    pub correction_overrides: Option<CorrectionFragmentOverrides>,
}
```

### 2. Fragment Exporter (`src/fragment_exporter.rs`)

**Rename functions:**
- `extract_checked_overrides()` → `extract_correction_overrides()`
- `extract_all_checked_overrides()` → `extract_all_correction_overrides()`

**Update override construction based on `frag_review` value:**

```rust
// In the extraction loop:
for row in rows {
    let frag_idx = row.frag_idx as usize;
    let frag_review = row.frag_review.as_deref().unwrap_or("");

    let override_data = match frag_review {
        "moved" => CorrectionFragmentOverride {
            collapse: true,
            // Don't use stale boundary values from moved fragments
            end_line: None,
            end_char: None,
            // Moved fragments have no SC data
            sc_code: None,
            sc_sutta: None,
        },
        _ => CorrectionFragmentOverride {
            collapse: false,
            end_line: row.end_line.map(|v| v as usize),
            end_char: row.end_char.map(|v| v as usize),
            sc_code: row.sc_code,
            sc_sutta: row.sc_sutta,
        },
    };

    overrides.insert(key, override_data);
}
```

The SQL queries remain the same (already include "moved" fragments).

### 3. Parser Helpers (`src/parsers/helpers.rs`)

**Update `apply_fragment_adjustment()` signature** - add `frag_start_line` and `frag_start_char` parameters:

```rust
pub fn apply_fragment_adjustment(
    xml_content: &str,
    default_end_pos: usize,
    default_end_line: usize,
    default_end_char: usize,
    cst_file: &str,
    frag_idx: usize,
    frag_start_pos: usize,
    frag_start_line: usize,   // NEW
    frag_start_char: usize,   // NEW
    correction_overrides: Option<&CorrectionFragmentOverrides>,
    adjustments: Option<&FragmentAdjustments>,
) -> Result<(usize, usize, usize)> {
    // First: check for collapse (moved fragments)
    if let Some(overrides) = correction_overrides {
        let key = FragmentKey { cst_file: cst_file.to_string(), frag_idx };
        if let Some(override_data) = overrides.get(&key) {
            if override_data.collapse {
                // Collapse: end = start (zero-width fragment)
                return Ok((frag_start_pos, frag_start_line, frag_start_char));
            }
        }
    }

    // Then: check for boundary override (existing logic)
    if let Some((end_line, end_char)) = get_boundary_override(
        cst_file, frag_idx, correction_overrides, adjustments
    ) {
        // ... existing validation and conversion logic
    }

    Ok((default_end_pos, default_end_line, default_end_char))
}
```

**The collapse check must come BEFORE `get_boundary_override()`** since moved fragments have `end_line: None`, `get_boundary_override()` would return None for them and fall through to the default - which is correct behavior if the collapse check isn't there, but we want the explicit collapse.

**Rename parameter references** in `get_boundary_override()`, `apply_boundary_override()`, `apply_sc_overrides()`, and `populate_sc_fields_from_tsv_conditional()`:
- `checked_overrides` → `correction_overrides`
- `CheckedFragmentOverrides` → `CorrectionFragmentOverrides`

### 4. All Parser Files (14 files)

Each parser calls `apply_fragment_adjustment()`. All call sites must be updated to pass two additional parameters (`frag_start_line`, `frag_start_char`).

**Files to update:**
- `src/parsers/anguttara_nikaya_mula.rs`
- `src/parsers/anguttara_nikaya_atthakatha.rs`
- `src/parsers/anguttara_nikaya_tika.rs`
- `src/parsers/samyutta_nikaya_mula.rs`
- `src/parsers/samyutta_nikaya_atthakatha.rs`
- `src/parsers/samyutta_nikaya_tika.rs`
- `src/parsers/majjhima_nikaya_mula.rs`
- `src/parsers/majjhima_nikaya_atthakatha.rs`
- `src/parsers/majjhima_nikaya_tika.rs`
- `src/parsers/digha_nikaya_mula.rs`
- `src/parsers/digha_nikaya_atthakatha.rs`
- `src/parsers/digha_nikaya_tika.rs`
- `src/parsers/general.rs`

**Pattern at each call site** - the callers already have `frag_start_line` and `frag_start_char` available from `current_fragment_start`:

```rust
// Existing pattern:
if let (Some((frag_start_pos, frag_start_line, frag_start_char)), Some(frag_type)) =
    (current_fragment_start, current_frag_type.as_ref()) {

    let (end_pos, end_line, end_char) = apply_fragment_adjustment(
        xml_content,
        event_start_pos,
        event_start_line,
        event_start_char,
        cst_file,
        fragments.len(),
        frag_start_pos,
        frag_start_line,  // ADD
        frag_start_char,  // ADD
        overrides.correction_overrides.as_ref(),  // RENAME
        overrides.adjustments.as_ref(),
    )?;
```

Also update references from `overrides.checked_overrides` → `overrides.correction_overrides`.

### 5. Web Routes (`src/web/routes.rs`)

**In `reparse_file()` endpoint:**
- Update function call: `extract_checked_overrides` → `extract_correction_overrides`
- Update variable name: `checked_overrides` → `correction_overrides`
- Update `ParserOverrides` field: `checked_overrides` → `correction_overrides`
- Update log messages from "checked overrides" to "correction overrides"

**In `regenerate()` endpoint (if it calls extraction directly):**
- Same renames as above (though most of the logic goes through `main.rs`)

### 6. Main (`src/main.rs`)

- Update function call: `extract_all_checked_overrides` → `extract_all_correction_overrides`
- Update `ParserOverrides` field: `checked_overrides` → `correction_overrides`
- Update log messages

### 7. Integration (`src/integration.rs`)

- Update any references to `checked_overrides` → `correction_overrides`

### 8. Tests

**Update existing tests** in `src/parsers/helpers.rs` (unit tests):
- `test_get_boundary_override_checked_takes_precedence` - update type names
- Add new test: `test_get_boundary_override_moved_fragment_collapse`

**Add new integration test** for the collapse behavior:
- Create fragments with "moved" and "checked" review status
- Run parser with correction overrides
- Verify moved fragments produce empty content
- Verify checked fragments absorb moved content

## Interaction with Existing Flows

### Single-File Reparse Flow

1. `extract_correction_overrides()` from current DB → includes moved fragments with `collapse: true`
2. Parse XML with overrides → moved fragments are collapsed, checked fragments absorb content
3. `export_fragments_to_db()` → stores collapsed fragments (empty content) and expanded checked fragments
4. `restore_frag_review_status()` → restores "moved" and "checked" statuses

Result: Moved fragments are empty with `frag_review="moved"`, checked fragments have merged content with `frag_review="checked"`.

### Full Database Regeneration Flow

1. Copy current DB → reference DB
2. `extract_all_correction_overrides()` from reference DB → includes moved fragments with `collapse: true`
3. Parse ALL XML files with overrides → moved fragments collapsed, checked fragments absorb content
4. `export_fragments_to_db()` → stores parsed fragments
5. `copy_reviewed_fragments_from_reference()` → **replaces** moved/checked fragments entirely from reference DB

Result: In Phase 3-4, the collapse prevents the boundary validation error. In Phase 5, the fragments are replaced with the reference data anyway. The collapse is a necessary "safe pass-through" that prevents errors during parsing.

### SC Override Propagation

No changes needed. Moved fragments have `sc_code: None` in the override, so `apply_sc_overrides()` skips them. The checked fragment has valid SC data and propagates context as before.

## Files Summary

| File | Changes |
|------|---------|
| `src/fragment_operations.rs` | Null out boundary fields (`start_line`, `start_char`, `end_line`, `end_char` → 0) for moved fragments |
| `src/types.rs` | Rename types, add `collapse` field |
| `src/fragment_exporter.rs` | Rename functions, differentiate moved vs checked in override construction |
| `src/parsers/helpers.rs` | Add collapse handling, update signatures with `frag_start_line`/`frag_start_char`, rename params |
| `src/parsers/*.rs` (13 parser files) | Update `apply_fragment_adjustment()` calls (2 new params + rename) |
| `src/web/routes.rs` | Rename function calls and variables |
| `src/main.rs` | Rename function calls and variables |
| `src/integration.rs` | Rename references |

## Implementation Order

1. **Fragment operations** (`fragment_operations.rs`) - Null out boundary fields on move
2. **Types** (`types.rs`) - Rename types, add `collapse` field
3. **Fragment exporter** (`fragment_exporter.rs`) - Rename functions, handle moved vs checked
4. **Parser helpers** (`helpers.rs`) - Add collapse handling, update signature
5. **All parser files** - Update call sites (mechanical change)
6. **Web routes** (`routes.rs`) - Rename references
7. **Main** (`main.rs`) - Rename references
8. **Integration** (`integration.rs`) - Rename references
9. **Build & test** - `cargo build && cargo test`

## Verification

1. `cargo build` - no compilation errors
2. `cargo test` - all existing tests pass
3. Manual test: regenerate database with a DB containing "moved" fragments → no crash
4. Manual test: single-file reparse with moved fragments → moved fragments are empty, checked fragments have correct content
