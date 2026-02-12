# Unifying the Anguttara Nikaya Parsers

Status: **DONE**

## Overview

The three Anguttara Nikaya parsers are nearly identical (~1,280-1,296 lines each, ~3,860 lines total). They should be consolidated into the existing `general.rs` parser, eliminating ~2,560 lines of duplicated code.

### Files Under Analysis

| File | Lines | Struct |
|------|-------|--------|
| `src/parsers/anguttara_nikaya_mula.rs` | 1,296 | `AnguttaraNikayaMula` |
| `src/parsers/anguttara_nikaya_atthakatha.rs` | 1,282 | `AnguttaraNikayaAtthakatha` |
| `src/parsers/anguttara_nikaya_tika.rs` | 1,282 | `AnguttaraNikayaTika` |
| `src/parsers/general.rs` (target) | 1,298 | `GeneralParser` |

## Documented Variations

### Atthakatha vs Tika: No Differences

The `anguttara_nikaya_atthakatha.rs` and `anguttara_nikaya_tika.rs` files are **byte-identical** in their `parse_into_fragments` function body, state variables, event handlers, post-processing, and tests. They differ only in:

- Module doc comment (line 1)
- Struct name (`AnguttaraNikayaAtthakatha` vs `AnguttaraNikayaTika`)
- `impl_xml_parser!` macro invocation

These two can be removed with zero logic changes.

### Mula vs Atthakatha/Tika: One Logic Difference

The mula parser differs from atthakatha/tika in exactly **one behavioral aspect**, applied in **two code locations**:

#### The `should_close` Guard

When deciding whether to close a fragment at a structural boundary, the mula parser adds a safety check for correction overrides:

**Mula** (lines 193-204, and again at lines 297-304):
```rust
if matches!(frag_type, FragmentType::Sutta) {
    let should_close = if frag_start_pos >= event_start_pos {
        true  // Force close when overrides pushed start past boundary
    } else {
        let tentative_content = xml_content[frag_start_pos..event_start_pos].to_string();
        tentative_content.contains("rend=\"subhead\"") ||
        tentative_content.contains("rend=\"chapter\"") ||
        tentative_content.contains("rend=\"bodytext\"")
    };
    if should_close { /* close fragment */ }
}
```

**Atthakatha/Tika** (lines 193-199, and again at lines 287-291):
```rust
if matches!(frag_type, FragmentType::Sutta) {
    let tentative_content = xml_content[frag_start_pos..event_start_pos].to_string();
    let has_sutta_content = tentative_content.contains("rend=\"subhead\"") ||
                           tentative_content.contains("rend=\"chapter\"") ||
                           tentative_content.contains("rend=\"bodytext\"");
    if has_sutta_content { /* close fragment */ }
}
```

**Key difference**: Mula has an extra guard: `if frag_start_pos >= event_start_pos { true }` which forces a fragment close when correction overrides have pushed the fragment start to or past the current boundary position. This prevents creating a zero/negative-length content slice and maintains `frag_idx` alignment with correction overrides.

This guard appears in **two places** in the mula parser:
1. The div boundary closure (structural div with ID) — ~line 193
2. The AN vagga chapter boundary closure (`is_an_vagga_chapter`) — ~line 297

**Note**: The atthakatha/tika parsers lack this guard. If `frag_start_pos >= event_start_pos` occurred in those parsers, the tentative_content slice would be empty (or panic with inverted indices), meaning it would never contain the rend attributes, and the fragment would not close. The mula guard is strictly more correct.

### Mula vs General Parser: Same Difference

The `general.rs` parser uses the same logic as atthakatha/tika (no `should_close` guard). It is already validated for DN and MN.

### All Other Aspects: Identical

The following are identical across all three AN parsers **and** `general.rs`:

- Import statements
- State variable declarations and initialization
- XML element handling (Event::Start, Event::Empty, Event::Text, Event::End, Event::Eof)
- XmlFragment construction (all 16 fields)
- Post-processing (derive_cst_fields, populate_sc_fields)
- Test code (all 14 test functions)
- `impl_xml_parser!` macro usage pattern

## Generalization Plan

### Precedent

DN and MN parsers have already been unified into `general.rs`. The Anguttara parsers follow the same pattern and can be absorbed into `general.rs` with a small conditional addition.

### Stage 1: Add the `should_close` Guard to `general.rs`

**Goal**: Make `general.rs` handle the mula override edge case that currently only the AN mula parser handles.

**Tasks**:

1. **Add the `should_close` guard to `general.rs`** in both fragment-close locations (div boundary and AN vagga chapter boundary). Replace:
   ```rust
   let tentative_content = xml_content[frag_start_pos..event_start_pos].to_string();
   let has_sutta_content = tentative_content.contains("rend=\"subhead\"") || ...;
   if has_sutta_content { ... }
   ```
   With the mula's version:
   ```rust
   let should_close = if frag_start_pos >= event_start_pos {
       true
   } else {
       let tentative_content = xml_content[frag_start_pos..event_start_pos].to_string();
       tentative_content.contains("rend=\"subhead\"") || ...
   };
   if should_close { ... }
   ```

2. **Run existing tests** for DN and MN to confirm no regression: `cargo test`

3. **Update `general.rs` doc comment** to list Anguttara Nikaya as validated.

### Stage 2: Validate `general.rs` Against AN Test Data

**Goal**: Confirm that `general.rs` produces identical output to the three AN-specific parsers.

**Tasks**:

1. **Write comparison tests** that parse the same AN XML files with both the specific parser and `general.rs`, then assert fragment-level equality (count, content, positions, CST fields).

2. **Test all three text types**:
   - AN mula files (exercises the `should_close` guard)
   - AN atthakatha files
   - AN tika files

3. **Fix any discrepancies** found during comparison testing.

### Stage 3: Remove AN-Specific Parsers

**Goal**: Delete the three AN parser files and route all AN parsing through `general.rs`.

**Tasks**:

1. **Update `src/parsers/mod.rs`**: Remove the three AN module declarations and re-exports.

2. **Update parser dispatch logic**: Wherever `AnguttaraNikayaMula`, `AnguttaraNikayaAtthakatha`, or `AnguttaraNikayaTika` are instantiated/selected, route to `GeneralParser` instead.

3. **Delete the three files**:
   - `src/parsers/anguttara_nikaya_mula.rs`
   - `src/parsers/anguttara_nikaya_atthakatha.rs`
   - `src/parsers/anguttara_nikaya_tika.rs`

4. **Move any AN-specific tests** from the deleted files into the `general.rs` test module or a dedicated integration test (the tests are already identical, so this is mainly ensuring coverage isn't lost).

5. **Run full test suite**: `cargo test` to confirm everything passes.

### Stage 4: Consider Samyutta Unification (Future)

The `samyutta_nikaya_*.rs` parsers likely follow the same duplication pattern and could be the next candidates for unification into `general.rs`. This is out of scope for this task but noted for planning.

## Risk Assessment

- **Low risk**: The atthakatha/tika parsers are already identical to `general.rs` in logic. Removing them is a pure deduplication.
- **Low risk**: The mula `should_close` guard is strictly more correct than the current `general.rs` behavior (it handles an edge case that the general parser silently ignores). Adding it to `general.rs` should not change behavior for DN/MN files that don't trigger the edge case.
- **Mitigation**: Stage 2 comparison tests ensure identical output before any deletions.
