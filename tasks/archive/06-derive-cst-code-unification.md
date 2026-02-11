# Unifying derive_cst_code() Across Parsers

Status: **DONE**

## Background

As part of the ongoing parser unification effort, `derive_cst_fields()` was
previously extracted into `src/parsers/helpers.rs` as a shared function. During
that refactoring, `derive_cst_code()` was kept as a per-parser function and
passed as a closure parameter to `derive_cst_fields()`:

```rust
pub fn derive_cst_fields<F>(
    fragment: &XmlFragment,
    nikaya_structure: &NikayaStructure,
    derive_cst_code_fn: F,
) -> (String, Option<String>, Option<String>, Option<String>, Option<String>)
where
    F: Fn(&XmlFragment, &NikayaStructure, Option<&str>) -> Option<String>,
```

Each of the 13 parser files contains its own local `derive_cst_code()` function
that it passes into `derive_cst_fields_shared()`. This document analyzes
differences across those implementations and proposes a plan for unification.

## File Inventory

| # | Parser file | Lines |
|---|-------------|-------|
| 1 | `src/parsers/general.rs` | 43-228 |
| 2 | `src/parsers/digha_nikaya_mula.rs` | 33-218 |
| 3 | `src/parsers/digha_nikaya_atthakatha.rs` | 33-218 |
| 4 | `src/parsers/digha_nikaya_tika.rs` | 33-218 |
| 5 | `src/parsers/majjhima_nikaya_mula.rs` | 33-218 |
| 6 | `src/parsers/majjhima_nikaya_atthakatha.rs` | 33-218 |
| 7 | `src/parsers/majjhima_nikaya_tika.rs` | 33-218 |
| 8 | `src/parsers/samyutta_nikaya_mula.rs` | 33-228 |
| 9 | `src/parsers/samyutta_nikaya_atthakatha.rs` | 33-218 |
| 10 | `src/parsers/samyutta_nikaya_tika.rs` | 33-218 |
| 11 | `src/parsers/anguttara_nikaya_mula.rs` | 33-218 |
| 12 | `src/parsers/anguttara_nikaya_atthakatha.rs` | 33-218 |
| 13 | `src/parsers/anguttara_nikaya_tika.rs` | 33-218 |

## Analysis: Identical vs. Different

### Signature (IDENTICAL across all 13 files)

```rust
fn derive_cst_code(
    fragment: &XmlFragment,
    nikaya_structure: &NikayaStructure,
    cst_sutta_title: Option<&str>,
) -> Option<String>
```

### Logical Sections

The function has 6 sequential sections. The analysis below compares each section
across all 13 parsers.

#### Section 1: Sutta ID early return (IDENTICAL - all 13 files)

Checks if a `Sutta` group level has an `id` attribute. If found, replaces `_`
with `.` and returns it immediately.

```rust
if let Some(sutta_id) = fragment.group_levels.iter()
    .find_map(|level| {
        if matches!(level.group_type, GroupType::Sutta) {
            level.id.as_ref()
        } else {
            None
        }
    }) {
    let code = sutta_id.replace('_', ".");
    return Some(code);
}
```

#### Section 2: Extract book_id (IDENTICAL - all 13 files)

Looks up `GroupType::Book` level and gets its `id`.

```rust
let book_id = fragment.group_levels.iter()
    .find_map(|level| {
        if matches!(level.group_type, GroupType::Book) {
            level.id.as_ref()
        } else {
            None
        }
    });
```

#### Section 3: Extract samyutta_number (IDENTICAL - all 13 files)

Conditional on `nikaya_structure.nikaya == "samyutta"`. Extracts a number from
the `Samyutta` group level's title (e.g., "1. Devatāsaṃyuttaṃ" -> "1"), with a
fallback to extracting from the ID.

#### Section 4: Extract pannasaka_number (IDENTICAL - all 13 files)

Conditional on `nikaya_structure.nikaya == "anguttara"`. Same pattern as
samyutta_number but for `GroupType::Pannasaka`.

#### Section 5: Extract vagga_number (VARIATION - SN mula differs)

**11 of 13 files** (all except SN mula) use forward iteration:

```rust
let vagga_number = fragment.group_levels.iter()
    .find_map(|level| { ... })
```

**SN mula** uses **reverse** iteration:

```rust
// Use .rev() to get the LAST (most recent) Vagga level, not the first
let vagga_number = fragment.group_levels.iter()
    .rev()
    .find_map(|level| { ... })
```

The inner logic (matching `GroupType::Vagga`, extracting number from title, ID
fallback) is identical. The only difference is `.rev()` in SN mula.

**SN atthakatha** uses forward iteration (no `.rev()`), same as the majority.

#### Section 6: Extract sutta_number (VARIATION - SN mula differs)

**11 of 13 files** use forward iteration:

```rust
let sutta_number = fragment.group_levels.iter()
    .find_map(|level| { ... })
```

**SN mula** uses **reverse** iteration:

```rust
// Use .rev() to get the LAST (most recent) Sutta level, not the first
let sutta_number = fragment.group_levels.iter()
    .rev()
    .find_map(|level| { ... })
```

Again, the inner logic is identical; only `.rev()` is different in SN mula.

#### Section 7: Build code from nikaya structure (VARIATION - SN mula differs)

**11 of 13 files** have this SN match arm:

```rust
"samyutta" => {
    match (book_id, samyutta_number, vagga_number, sutta_number) {
        (Some(book), Some(samyutta), Some(vagga), Some(sutta)) => {
            Some(format!("{}.{}.{}.{}", book, samyutta, vagga, sutta))
        }
        (Some(book), Some(samyutta), Some(vagga), None) => {
            Some(format!("{}.{}.{}.0", book, samyutta, vagga))
        }
        _ => None,
    }
}
```

**SN mula** has an **additional match arm** for samyuttas without vaggas:

```rust
"samyutta" => {
    match (book_id, samyutta_number, vagga_number, sutta_number) {
        (Some(book), Some(samyutta), Some(vagga), Some(sutta)) => {
            Some(format!("{}.{}.{}.{}", book, samyutta, vagga, sutta))
        }
        (Some(book), Some(samyutta), Some(vagga), None) => {
            Some(format!("{}.{}.{}.0", book, samyutta, vagga))
        }
        // Extra arm only in SN mula:
        (Some(book), Some(samyutta), None, Some(sutta)) => {
            // SN without vaggas (like Bhikkhunīsaṃyuttaṃ): use 1 as vagga number
            Some(format!("{}.{}.1.{}", book, samyutta, sutta))
        }
        _ => None,
    }
}
```

All other match arms (`"digha"`, `"majjhima"`, `"anguttara"`, `_` default) are
identical across all 13 files.

## Summary of Differences

| Variation | Which file(s) | What differs |
|-----------|--------------|--------------|
| Reverse iteration for vagga_number | SN mula only | `.iter().rev()` instead of `.iter()` |
| Reverse iteration for sutta_number | SN mula only | `.iter().rev()` instead of `.iter()` |
| Extra SN match arm (no vagga) | SN mula only | Additional `(Some, Some, None, Some)` arm |

**All other 12 files have byte-for-byte identical `derive_cst_code()` implementations.**

The SN mula variation exists because Saṃyutta Nikāya mūla has samyuttas where
`group_levels` may contain stale entries from previous samyuttas, requiring
reverse iteration to pick the most recent level. Additionally, some samyuttas
(like Bhikkhunīsaṃyuttaṃ) don't have vaggas, requiring the extra match arm.

## Note on derive_cst_fields

The `derive_cst_fields()` function in `helpers.rs` already has a parallel
`use_rev` mechanism for `cst_vagga` and `cst_sutta` extraction. It uses:

```rust
let use_rev = nikaya_structure.nikaya == "samyutta"
    && fragment.cst_file.ends_with(".mul.xml");
```

This same condition can be reused for `derive_cst_code()`.

---

## Unification Plan

### Stage 1: Add shared derive_cst_code() to helpers.rs

**Task 1.1**: Add `pub fn derive_cst_code()` to `src/parsers/helpers.rs`

The shared function needs a `use_rev: bool` parameter (or derives it internally
from `nikaya_structure` and `cst_file`) to control the `.rev()` behavior.

A clean approach: derive `use_rev` internally using the same logic as
`derive_cst_fields()`:

```rust
pub fn derive_cst_code(
    fragment: &XmlFragment,
    nikaya_structure: &NikayaStructure,
    cst_sutta_title: Option<&str>,
) -> Option<String> {
    // SN mula uses reverse iteration for vagga/sutta because
    // group_levels may contain stale entries from previous samyuttas
    let use_rev = nikaya_structure.nikaya == "samyutta"
        && fragment.cst_file.ends_with(".mul.xml");

    // Section 1: Sutta ID early return (identical for all)
    // ...

    // Section 5: vagga_number - use .rev() when use_rev is true
    let vagga_number = if use_rev {
        fragment.group_levels.iter().rev()
            .find_map(|level| { /* vagga logic */ })
    } else {
        fragment.group_levels.iter()
            .find_map(|level| { /* same vagga logic */ })
    };

    // Section 6: sutta_number - use .rev() when use_rev is true
    let sutta_number = if use_rev {
        fragment.group_levels.iter().rev()
            .find_map(|level| { /* sutta logic */ })
    } else {
        fragment.group_levels.iter()
            .find_map(|level| { /* same sutta logic */ })
    };

    // Section 7: Build code - include the extra SN no-vagga arm for all
    // (it's harmless for non-SN parsers since samyutta_number will be None)
    match nikaya_structure.nikaya.as_str() {
        // ...
        "samyutta" => {
            match (book_id, samyutta_number, vagga_number, sutta_number) {
                (Some(book), Some(samyutta), Some(vagga), Some(sutta)) => { ... }
                (Some(book), Some(samyutta), Some(vagga), None) => { ... }
                (Some(book), Some(samyutta), None, Some(sutta)) => { ... }
                _ => None,
            }
        }
        // ...
    }
}
```

The extra `(Some, Some, None, Some)` SN arm is safe to include universally
because it only triggers when `nikaya_structure.nikaya == "samyutta"` and
`vagga_number` is `None` -- conditions that will only occur for SN.

### Stage 2: Replace local derive_cst_code() in each parser

**Task 2.1**: Update each of the 13 parser files:

1. Remove the local `fn derive_cst_code()` function definition
2. Add `derive_cst_code as derive_cst_code_shared` (or just `derive_cst_code`)
   to the imports from `helpers`
3. Update the call site to pass `derive_cst_code_shared` (or just use the
   imported name)

This should be done parser-by-parser with test runs between each change:

- Start with the 11 identical parsers (lowest risk)
- Then SN mula (the one with variations)
- Run `cargo test` after each parser change

### Stage 3: Remove the function parameter from derive_cst_fields()

**Task 3.1**: Once all parsers call the shared `derive_cst_code()`, update
`derive_cst_fields()` in `helpers.rs`:

1. Remove the generic `F` parameter and the `derive_cst_code_fn` parameter
2. Call `derive_cst_code()` directly inside `derive_cst_fields()`
3. Simplify the signature:

```rust
pub fn derive_cst_fields(
    fragment: &XmlFragment,
    nikaya_structure: &NikayaStructure,
) -> (String, Option<String>, Option<String>, Option<String>, Option<String>)
```

**Task 3.2**: Update all 13 parser call sites that currently pass
`derive_cst_code` as a parameter to use the new simpler signature:

```rust
// Before:
let (cst_file, cst_code, cst_vagga, cst_sutta, cst_paranum) =
    derive_cst_fields_shared(fragment, nikaya_structure, derive_cst_code);

// After:
let (cst_file, cst_code, cst_vagga, cst_sutta, cst_paranum) =
    derive_cst_fields_shared(fragment, nikaya_structure);
```

**Task 3.3**: Remove the `derive_cst_code` import alias from each parser file
(they no longer need to import or reference `derive_cst_code` since it's called
internally by `derive_cst_fields()`).

### Stage 4: Final cleanup and verification

**Task 4.1**: Run full test suite: `cargo test`

**Task 4.2**: Verify no remaining local `derive_cst_code` functions exist:
```
grep -rn "fn derive_cst_code" src/parsers/
```
Should only show the one in `helpers.rs`.

**Task 4.3**: Verify the function parameter is fully removed:
```
grep -rn "derive_cst_code_fn" src/
```
Should return no results.

## Risk Assessment

**Low risk**: 11 of 13 parsers have byte-identical `derive_cst_code()`
implementations. The SN mula variation is well-understood (reverse iteration +
extra match arm) and can be handled with a simple boolean flag using the same
`use_rev` pattern already proven in `derive_cst_fields()`.

**Testing strategy**: The existing test suite covers all nikayas. Run `cargo
test` after each stage to catch regressions immediately.
