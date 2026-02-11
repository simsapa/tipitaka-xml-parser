# Unifying derive_cst_fields() Across Nikaya Parsers

Status: **DONE**

## Overview

`derive_cst_fields()` extracts CST metadata fields from a parsed fragment.
It is present in all 13 nikaya parser files plus the general parser (14 total).

**Signature** (identical in all files):
```rust
fn derive_cst_fields(
    fragment: &XmlFragment,
    nikaya_structure: &NikayaStructure,
) -> (String, Option<String>, Option<String>, Option<String>, Option<String>)
//    cst_file  cst_code      cst_vagga     cst_sutta     cst_paranum
```

**Call site** (identical in all files):
```rust
let (cst_file, cst_code, cst_vagga, cst_sutta, cst_paranum) =
    derive_cst_fields(fragment, nikaya_structure);
```

---

## Detailed Comparison

### Files Examined

| File | Lines | Variant |
|------|-------|---------|
| `digha_nikaya_mula.rs` | 40–98 | Standard |
| `digha_nikaya_atthakatha.rs` | 40–98 | Standard |
| `digha_nikaya_tika.rs` | 40–98 | Standard |
| `majjhima_nikaya_mula.rs` | 40–98 | Standard |
| `majjhima_nikaya_atthakatha.rs` | 40–98 | Standard |
| `majjhima_nikaya_tika.rs` | 40–98 | Standard |
| `anguttara_nikaya_mula.rs` | 42–100 | Standard |
| `anguttara_nikaya_atthakatha.rs` | 42–100 | Standard |
| `anguttara_nikaya_tika.rs` | 40–98 | Standard |
| `samyutta_nikaya_atthakatha.rs` | 42–100 | Standard |
| `samyutta_nikaya_tika.rs` | 40–98 | Standard |
| `general.rs` | 50–108 | Standard |
| `samyutta_nikaya_mula.rs` | 42–107 | **SN-Mula variant** |

### Structural Breakdown

The function has five logical sections. All parsers share the same sections,
but there are two variation points within them.

#### Section 1: Early return for non-Sutta fragments (IDENTICAL everywhere)

```rust
let cst_file = fragment.cst_file.clone();

// Only process Sutta fragments
if !matches!(fragment.frag_type, crate::types::FragmentType::Sutta) {
    return (cst_file, None, None, None, None);
}
```

#### Section 2: Extract cst_vagga from group_levels (VARIATION POINT #1)

**Standard variant** (12 files: all DN, MN, AN, SN atthakatha, SN tika, general):
```rust
// Check if the nikaya structure supports vaggas
let has_vagga_level = nikaya_structure.levels.iter()
    .any(|t| matches!(t, crate::types::GroupType::Vagga));

let cst_vagga = if has_vagga_level {
    fragment.group_levels.iter()
        .find(...)          // forward iteration, first match
        .and_then(|level| { /* filter empty */ })
        .or_else(|| {
            // Fallback: always try extract_vagga_title_from_content
            extract_vagga_title_from_content(&fragment.content_xml)
        })
} else {
    None
};
```

**SN-Mula variant** (1 file: `samyutta_nikaya_mula.rs`):
```rust
// No has_vagga_level check — just looks directly in group_levels
let cst_vagga = fragment.group_levels.iter()
    .rev()              // ← reverse iteration, last (most recent) match
    .find(...)
    .and_then(|level| { /* filter empty */ })
    .or_else(|| {
        // Conditional fallback: only for MN, not SN
        if nikaya_structure.nikaya == "majjhima" {
            extract_vagga_title_from_content(&fragment.content_xml)
        } else {
            None
        }
    });
```

**Differences summarized:**

| Aspect | Standard (12 files) | SN-Mula (1 file) |
|--------|-------------------|-------------------|
| Has vagga guard | `has_vagga_level` check wraps the whole block | No guard; always searches `group_levels` |
| Iteration direction | `.find()` — forward, first match | `.rev().find()` — reverse, last (most recent) match |
| Vagga title fallback | Always calls `extract_vagga_title_from_content` | Only calls for `nikaya == "majjhima"` (i.e. never for SN itself) |

#### Section 3: Extract cst_sutta from group_levels (VARIATION POINT #2)

**Standard variant** (12 files):
```rust
let cst_sutta = fragment.group_levels.iter()
    .find(|level| matches!(level.group_type, crate::types::GroupType::Sutta))
    .and_then(|level| { /* filter empty */ })
    .or_else(|| {
        extract_sutta_title_from_content(&fragment.content_xml)
    });
```

**SN-Mula variant** (1 file):
```rust
let cst_sutta = fragment.group_levels.iter()
    .rev()              // ← reverse iteration, last (most recent) match
    .find(|level| matches!(level.group_type, crate::types::GroupType::Sutta))
    .and_then(|level| { /* filter empty */ })
    .or_else(|| {
        extract_sutta_title_from_content(&fragment.content_xml)
    });
```

**Differences summarized:**

| Aspect | Standard (12 files) | SN-Mula (1 file) |
|--------|-------------------|-------------------|
| Iteration direction | `.find()` — forward, first match | `.rev().find()` — reverse, last match |
| Fallback | identical | identical |

#### Section 4: Extract cst_paranum (IDENTICAL everywhere)

```rust
let cst_paranum = extract_first_paranum(&fragment.content_xml);
```

#### Section 5: Derive cst_code and return (IDENTICAL everywhere)

```rust
let cst_code = derive_cst_code(fragment, nikaya_structure, cst_sutta.as_deref());

(cst_file, cst_code, cst_vagga, cst_sutta, cst_paranum)
```

Note: `derive_cst_code()` is itself a separate function with its own variations.
It is NOT part of `derive_cst_fields()` and will be unified separately.

---

## Summary of Variation Points

There are exactly **two variation points**, both in the SN-Mula parser only:

1. **Vagga extraction strategy**
   - Whether to gate on `has_vagga_level` or always search `group_levels`
   - Whether to iterate forward (`.find()`) or reverse (`.rev().find()`)
   - Whether vagga title fallback is unconditional or nikaya-conditional

2. **Sutta extraction iteration direction**
   - Forward (`.find()`) vs reverse (`.rev().find()`)

The SN-Mula variant uses `.rev()` because SN's group_levels may contain stale
entries from previous samyuttas, so the most recent (last) entry is the correct
one. The standard variant doesn't need this because DN/MN/AN don't have this
stacking issue.

**Note on SN atthakatha and SN tika**: These use the **Standard** variant
(no `.rev()`, same vagga fallback). This suggests that the SN-specific logic
may only be needed for the mula texts, or that the atthakatha/tika parsers
were adapted from the DN/MN/AN template without SN-specific adjustments.

---

## Plan for Generalization

### Approach

Create a single `pub fn derive_cst_fields()` in `helpers.rs` that accepts a
configuration parameter controlling the two variation points. The
`nikaya_structure.nikaya` field already carries enough context to make the
right decisions, so no extra configuration struct is needed.

### Stage 1: Create unified function in helpers.rs

**Task 1.1**: Add `pub fn derive_cst_fields()` to `src/parsers/helpers.rs`

The unified function uses `nikaya_structure.nikaya` to choose behavior:

```rust
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
/// * `cst_file_name` - The CST file name being parsed (needed for SN mula detection)
///
/// # Returns
/// Tuple of (cst_file, cst_code, cst_vagga, cst_sutta, cst_paranum)
pub fn derive_cst_fields(
    fragment: &XmlFragment,
    nikaya_structure: &NikayaStructure,
) -> (String, Option<String>, Option<String>, Option<String>, Option<String>) {
    let cst_file = fragment.cst_file.clone();

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
        fragment.group_levels.iter()
            .rev()
            .find(|level| matches!(level.group_type, GroupType::Vagga))
            .and_then(|level| {
                if level.title.trim().is_empty() { None }
                else { Some(level.title.clone()) }
            })
            .or_else(|| {
                // SN mula: only use vagga fallback for MN (never for SN itself)
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
                    if level.title.trim().is_empty() { None }
                    else { Some(level.title.clone()) }
                })
                .or_else(|| {
                    extract_vagga_title_from_content(&fragment.content_xml)
                })
        } else {
            None
        }
    };

    // --- cst_sutta ---
    let cst_sutta = if use_rev {
        fragment.group_levels.iter()
            .rev()
            .find(|level| matches!(level.group_type, GroupType::Sutta))
    } else {
        fragment.group_levels.iter()
            .find(|level| matches!(level.group_type, GroupType::Sutta))
    }
    .and_then(|level| {
        if level.title.trim().is_empty() { None }
        else { Some(level.title.clone()) }
    })
    .or_else(|| {
        extract_sutta_title_from_content(&fragment.content_xml)
    });

    // --- cst_paranum ---
    let cst_paranum = extract_first_paranum(&fragment.content_xml);

    // --- cst_code ---
    // derive_cst_code is still per-parser for now
    // Once it's also unified, this will call the shared version
    let cst_code = derive_cst_code(fragment, nikaya_structure, cst_sutta.as_deref());

    (cst_file, cst_code, cst_vagga, cst_sutta, cst_paranum)
}
```

**Decision point**: The `use_rev` flag is determined by checking
`nikaya_structure.nikaya == "samyutta"` AND `cst_file.ends_with(".mul.xml")`.
This matches the current behavior where only SN mula uses `.rev()` while SN
atthakatha and SN tika use forward iteration.

**Task 1.2**: `derive_cst_fields` calls `derive_cst_code`, which is also
per-parser. Since `derive_cst_code` has its own variations, we have two choices:

- **Option A**: Move `derive_cst_fields` to helpers.rs but keep calling each
  parser's local `derive_cst_code`. This requires passing `derive_cst_code` as
  a function pointer or making `derive_cst_code` also shared.

- **Option B** (recommended): Unify `derive_cst_code` first or simultaneously,
  so both can live in helpers.rs together.

Since `derive_cst_code` is the only remaining per-parser dependency within
`derive_cst_fields`, and `derive_cst_code` is already documented in the
existing refactoring analysis (Case 5 in `nikaya-parser-refactoring.md`),
the practical approach is:

→ **Unify `derive_cst_code` as part of the same stage**, since
`derive_cst_fields` cannot be fully extracted without it.

### Stage 2: Migrate all parsers to use the shared function

**Task 2.1**: Update imports in all 14 parser files to import
`derive_cst_fields` from `helpers.rs`

**Task 2.2**: Remove the local `derive_cst_fields` function from each file:
- `digha_nikaya_mula.rs`
- `digha_nikaya_atthakatha.rs`
- `digha_nikaya_tika.rs`
- `majjhima_nikaya_mula.rs`
- `majjhima_nikaya_atthakatha.rs`
- `majjhima_nikaya_tika.rs`
- `samyutta_nikaya_mula.rs`
- `samyutta_nikaya_atthakatha.rs`
- `samyutta_nikaya_tika.rs`
- `anguttara_nikaya_mula.rs`
- `anguttara_nikaya_atthakatha.rs`
- `anguttara_nikaya_tika.rs`
- `general.rs`

**Task 2.3**: Run `cargo test` to verify no regressions

### Stage 3: Verify correctness

**Task 3.1**: Run the full test suite: `cargo test`

**Task 3.2**: Spot-check output for each nikaya variant:
- DN mula (representative of the 12 Standard files)
- SN mula (the one SN-specific file)
- SN atthakatha (confirms SN non-mula uses Standard path)

### Stage 4: Clean up

**Task 4.1**: Remove `derive_cst_code` from individual parser files
(once it's also unified — this is a dependency)

**Task 4.2**: Update `nikaya-parser-refactoring.md` Case 4 to mark as DONE

---

## Risk Assessment

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| SN mula behavior change | Low | High | `use_rev` flag preserves exact current behavior |
| SN att/tika accidentally getting `.rev()` | Low | Medium | Guard checks both nikaya AND `.mul.xml` suffix |
| `derive_cst_code` coupling | Certain | Blocks full extraction | Must unify `derive_cst_code` simultaneously |
| Test coverage gap | Medium | Medium | Existing integration tests cover all nikayas |

## Lines of Code Impact

- **Current**: ~58 lines × 13 files = ~754 lines of duplicated `derive_cst_fields`
- **After**: ~60 lines in one location (helpers.rs) + 1 import line per parser
- **Net reduction**: ~680 lines

## Dependency Note

This task is **blocked by or should be done simultaneously with**
`derive_cst_code` unification (Case 5 in `nikaya-parser-refactoring.md`),
because `derive_cst_fields` calls `derive_cst_code` directly.
