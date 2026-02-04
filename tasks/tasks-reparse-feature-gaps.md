# Tasks: Single File Reparse Feature - Gap Fixes

This task list addresses gaps identified during PRD review of the single-file reparse with checked fragment overrides feature.

## Relevant Files

- `src/parsers/general.rs` - General parser using `apply_fragment_adjustment`
- `src/parsers/digha_nikaya_mula.rs` - DN mula parser
- `src/parsers/digha_nikaya_atthakatha.rs` - DN atthakatha parser
- `src/parsers/digha_nikaya_tika.rs` - DN tika parser
- `src/parsers/majjhima_nikaya_mula.rs` - MN mula parser
- `src/parsers/majjhima_nikaya_atthakatha.rs` - MN atthakatha parser
- `src/parsers/majjhima_nikaya_tika.rs` - MN tika parser
- `src/parsers/samyutta_nikaya_mula.rs` - SN mula parser
- `src/parsers/samyutta_nikaya_atthakatha.rs` - SN atthakatha parser
- `src/parsers/samyutta_nikaya_tika.rs` - SN tika parser
- `src/parsers/anguttara_nikaya_mula.rs` - AN mula parser
- `src/parsers/anguttara_nikaya_atthakatha.rs` - AN atthakatha parser
- `src/parsers/anguttara_nikaya_tika.rs` - AN tika parser
- `src/parsers/helpers.rs` - Helper functions including `get_boundary_override`, `apply_fragment_adjustment`
- `src/static/scripts/app.js` - Frontend JavaScript
- `tests/test_checked_fragment_overrides.rs` - Integration tests

---

## Gap 1: Boundary Overrides from Checked Fragments Not Applied

**Severity**: High

**Problem**: All 13 parsers call `apply_fragment_adjustment(overrides.adjustments.as_ref())` which only checks the legacy `FragmentAdjustments` TSV data. The `checked_overrides` from the database are ignored for boundary adjustments.

**PRD Requirement** (FR2.3, FR2.6):
> "CheckedFragmentOverrides take precedence over FragmentAdjustments"
> "Boundary overrides (`end_line`, `end_char`) affect only the current fragment"

### Implementation Overview

The `get_boundary_override()` function already exists in `helpers.rs` and correctly implements the priority logic (checked overrides first, then adjustments). However, it's not being used.

**Current code pattern in parsers:**
```rust
let (end_line, end_char, end_pos) = apply_fragment_adjustment(
    &lines,
    current_line,
    current_char,
    cst_file,
    fragments.len(),
    overrides.adjustments.as_ref(),
);
```

**Required change:**
The `apply_fragment_adjustment` function should be updated to accept both `checked_overrides` and `adjustments`, and internally call `get_boundary_override()` to determine if there's an override.

Alternatively, create a new function `apply_boundary_with_overrides()` that wraps this logic.

### Tasks

- [ ] 1.0 Update boundary override application in parsers
  - [ ] 1.1 Update `apply_fragment_adjustment()` signature in `src/parsers/helpers.rs` to accept `checked_overrides: Option<&CheckedFragmentOverrides>` as an additional parameter
  - [ ] 1.2 Modify `apply_fragment_adjustment()` implementation to call `get_boundary_override(cst_file, frag_idx, checked_overrides, adjustments)` internally
  - [ ] 1.3 Update all call sites in `src/parsers/general.rs` to pass `overrides.checked_overrides.as_ref()` as the new parameter
  - [ ] 1.4 Update all call sites in `src/parsers/digha_nikaya_mula.rs`
  - [ ] 1.5 Update all call sites in `src/parsers/digha_nikaya_atthakatha.rs`
  - [ ] 1.6 Update all call sites in `src/parsers/digha_nikaya_tika.rs`
  - [ ] 1.7 Update all call sites in `src/parsers/majjhima_nikaya_mula.rs`
  - [ ] 1.8 Update all call sites in `src/parsers/majjhima_nikaya_atthakatha.rs`
  - [ ] 1.9 Update all call sites in `src/parsers/majjhima_nikaya_tika.rs`
  - [ ] 1.10 Update all call sites in `src/parsers/samyutta_nikaya_mula.rs`
  - [ ] 1.11 Update all call sites in `src/parsers/samyutta_nikaya_atthakatha.rs`
  - [ ] 1.12 Update all call sites in `src/parsers/samyutta_nikaya_tika.rs`
  - [ ] 1.13 Update all call sites in `src/parsers/anguttara_nikaya_mula.rs`
  - [ ] 1.14 Update all call sites in `src/parsers/anguttara_nikaya_atthakatha.rs`
  - [ ] 1.15 Update all call sites in `src/parsers/anguttara_nikaya_tika.rs`
  - [ ] 1.16 Run `cargo build` to verify compilation
  - [ ] 1.17 Run `cargo test` to verify all existing tests pass

### Recommended Tests

| Test Name | Description | Location |
|-----------|-------------|----------|
| `test_boundary_override_from_checked_fragment` | Verify that boundary overrides (end_line, end_char) from checked fragments are applied during parsing | `tests/test_checked_fragment_overrides.rs` |
| `test_boundary_override_precedence` | Verify checked boundary overrides take precedence over TSV adjustments when both exist for same fragment | `tests/test_checked_fragment_overrides.rs` |
| `test_boundary_override_content_extraction` | Verify fragment content is correctly extracted based on overridden boundaries | `tests/test_checked_fragment_overrides.rs` |

### Tasks

- [ ] 2.0 Add tests for boundary override from checked fragment
  - [ ] 2.1 Add test `test_boundary_override_from_checked_fragment` in `tests/test_checked_fragment_overrides.rs`:
    - Create simple XML with multiple fragments
    - Parse and export to DB
    - Update a fragment with different `end_line`/`end_char` values and mark as "checked"
    - Extract overrides using `extract_checked_overrides()`
    - Reparse with the checked overrides
    - Verify fragment boundaries match the checked override values
    - Verify `content_xml` length changed appropriately
  - [ ] 2.2 Add test `test_boundary_override_precedence` in `tests/test_checked_fragment_overrides.rs`:
    - Create a `FragmentAdjustments` entry for a specific (cst_file, frag_idx)
    - Create a `CheckedFragmentOverride` for the same (cst_file, frag_idx) with different boundary values
    - Parse with both overrides
    - Verify the checked override values are used, not the TSV adjustment values
  - [ ] 2.3 Add test `test_boundary_override_content_extraction` in `tests/test_checked_fragment_overrides.rs`:
    - Create XML where fragment boundaries matter for content
    - Set a checked override that moves the boundary
    - Verify the extracted `content_xml` reflects the new boundary position
  - [ ] 2.4 Run `cargo test --test test_checked_fragment_overrides` to verify all new tests pass

---

## Gap 2: Conditional TSV Population Not Used

**Severity**: Medium

**Problem**: The `populate_sc_fields_from_tsv_conditional()` function exists but parsers use the non-conditional `populate_sc_fields_from_tsv()`. This means SC fields from TSV mapping could potentially overwrite values set by checked overrides.

**PRD Requirement** (FR2.4):
> "`populate_sc_fields_from_tsv()` → maps `cst_code` to `sc_code` (now conditional, skips fragments with SC already set)"

**Current Mitigation**: The issue is partially mitigated because `apply_sc_overrides()` runs AFTER the parser completes (in `xml_parser.rs`), so it overwrites any TSV-populated values. However, this is not the intended design flow per the PRD.

### Implementation Overview

The PRD specifies this execution order:
1. Parsing loop → creates fragments
2. `derive_cst_fields()` → populates `cst_code`
3. **Apply SC overrides from checked fragments**
4. `populate_sc_fields_from_tsv_conditional()` → skips fragments with SC already set

Currently, step 3 happens after the parser returns (in `xml_parser.rs`), and step 4 uses the non-conditional function inside the parser. The fix is to either:

**Option A**: Move TSV population out of parsers into `xml_parser.rs` (after `apply_sc_overrides`)
**Option B**: Replace `populate_sc_fields_from_tsv` with `populate_sc_fields_from_tsv_conditional` in all parsers

Option B is simpler and maintains the current architecture.

### Recommended Tests

| Test Name | Description | Location |
|-----------|-------------|----------|
| `test_conditional_tsv_skips_existing_sc_code` | Verify `populate_sc_fields_from_tsv_conditional()` does not overwrite existing `sc_code` values | `src/parsers/helpers.rs` (unit test) |
| `test_conditional_tsv_populates_null_sc_code` | Verify `populate_sc_fields_from_tsv_conditional()` populates `sc_code` when it's None | `src/parsers/helpers.rs` (unit test) |
| `test_sc_override_not_overwritten_by_tsv` | Integration test verifying SC overrides from checked fragments survive TSV population | `tests/test_checked_fragment_overrides.rs` |

### Tasks

- [ ] 3.0 Use conditional TSV population in parsers
  - [ ] 3.1 In `src/parsers/general.rs`, change import from `populate_sc_fields_from_tsv` to `populate_sc_fields_from_tsv_conditional`
  - [ ] 3.2 In `src/parsers/general.rs`, change the function call from `populate_sc_fields_from_tsv(&mut fragments)?` to `populate_sc_fields_from_tsv_conditional(&mut fragments)?`
  - [ ] 3.3 Repeat for `src/parsers/digha_nikaya_mula.rs`
  - [ ] 3.4 Repeat for `src/parsers/digha_nikaya_atthakatha.rs`
  - [ ] 3.5 Repeat for `src/parsers/digha_nikaya_tika.rs`
  - [ ] 3.6 Repeat for `src/parsers/majjhima_nikaya_mula.rs`
  - [ ] 3.7 Repeat for `src/parsers/majjhima_nikaya_atthakatha.rs`
  - [ ] 3.8 Repeat for `src/parsers/majjhima_nikaya_tika.rs`
  - [ ] 3.9 Repeat for `src/parsers/samyutta_nikaya_mula.rs`
  - [ ] 3.10 Repeat for `src/parsers/samyutta_nikaya_atthakatha.rs`
  - [ ] 3.11 Repeat for `src/parsers/samyutta_nikaya_tika.rs`
  - [ ] 3.12 Repeat for `src/parsers/anguttara_nikaya_mula.rs`
  - [ ] 3.13 Repeat for `src/parsers/anguttara_nikaya_atthakatha.rs`
  - [ ] 3.14 Repeat for `src/parsers/anguttara_nikaya_tika.rs`
  - [ ] 3.15 Run `cargo build` to verify compilation
  - [ ] 3.16 Run `cargo test` to verify all existing tests pass

- [ ] 4.0 Add tests for conditional TSV population
  - [ ] 4.1 Add unit test `test_conditional_tsv_skips_existing_sc_code` in `src/parsers/helpers.rs`:
    - Create fragments where some have `sc_code = Some("existing")` and others have `sc_code = None`
    - Call `populate_sc_fields_from_tsv_conditional()`
    - Verify fragments with existing `sc_code` are unchanged
  - [ ] 4.2 Add unit test `test_conditional_tsv_populates_null_sc_code` in `src/parsers/helpers.rs`:
    - Create fragments with `sc_code = None` but valid `cst_code` that maps to SC codes
    - Call `populate_sc_fields_from_tsv_conditional()`
    - Verify `sc_code` is populated from TSV mapping
  - [ ] 4.3 Add integration test `test_sc_override_not_overwritten_by_tsv` in `tests/test_checked_fragment_overrides.rs`:
    - Parse a file with checked SC override
    - Verify the override value persists through the full parsing pipeline
    - Verify TSV mapping did not overwrite the override
  - [ ] 4.4 Run `cargo test` to verify all new tests pass

---

## Gap 3: Reparse Buttons Not Disabled During Operation

**Severity**: Low

**Problem**: The reparse buttons in the file list are not disabled while a reparse or regeneration operation is in progress. Users could potentially trigger multiple simultaneous operations.

**PRD Requirement** (FR1.1):
> "Button must be disabled while any regeneration or reparse operation is in progress"

### Implementation Overview

Add a global flag `isOperationInProgress` that is set to `true` when any reparse/regeneration starts and `false` when it completes. The `fetchAndPopulateFileList()` function should check this flag when rendering buttons, and the buttons should be disabled when the flag is true.

Additionally, disable buttons immediately when an operation starts and re-enable when it completes.

### Recommended Tests

Since this is a UI feature, testing is primarily manual. Document these manual test cases:

| Test Case | Steps | Expected Result |
|-----------|-------|-----------------|
| Buttons disabled during reparse | 1. Click reparse button on a file<br>2. Observe other reparse buttons | All reparse buttons should be disabled (grayed out) |
| Buttons disabled during regeneration | 1. Open Regenerate modal<br>2. Click "Regenerate Using Current DB"<br>3. Observe reparse buttons | All reparse buttons should be disabled |
| Buttons re-enabled after success | 1. Start a reparse operation<br>2. Wait for completion<br>3. Close modal | All reparse buttons should be enabled again |
| Buttons re-enabled after error | 1. Start reparse with invalid file<br>2. Wait for error<br>3. Close modal | All reparse buttons should be enabled again |
| Cannot double-click reparse | 1. Click reparse button<br>2. Quickly try to click another reparse button | Second click should be blocked (button disabled) |

### Tasks

- [ ] 5.0 Disable reparse buttons during operations
  - [ ] 5.1 Add global variable `let isOperationInProgress = false;` in `src/static/scripts/app.js`
  - [ ] 5.2 In `reparseFile()`, set `isOperationInProgress = true` at start and `isOperationInProgress = false` in finally block (use try/finally pattern)
  - [ ] 5.3 In `startRegeneration()`, set `isOperationInProgress = true` at start and `isOperationInProgress = false` in finally block
  - [ ] 5.4 Add function `updateReparseButtonsState()` that:
    - Selects all `.reparse-btn` elements
    - Sets `disabled = isOperationInProgress` on each
    - Optionally adds/removes a CSS class for visual feedback
  - [ ] 5.5 Call `updateReparseButtonsState()` whenever `isOperationInProgress` changes
  - [ ] 5.6 In `fetchAndPopulateFileList()`, set `reparseBtn.disabled = isOperationInProgress` when creating buttons
  - [ ] 5.7 Also disable the regenerate modal buttons when `isOperationInProgress` is true
  - [ ] 5.8 Manual test: execute all test cases from the table above

---

## Summary

| Gap | Severity | Implementation Tasks | Test Tasks | Est. Effort |
|-----|----------|---------------------|------------|-------------|
| 1. Boundary overrides not applied | High | 1.0 (17 sub-tasks) | 2.0 (4 sub-tasks) | Medium |
| 2. Conditional TSV population | Medium | 3.0 (16 sub-tasks) | 4.0 (4 sub-tasks) | Low |
| 3. Buttons not disabled | Low | 5.0 (8 sub-tasks) | Manual testing | Low |

**Recommended order**: Gap 1 → Gap 2 → Gap 3 (address highest severity first)

**Total tasks**: 5 task groups, 49 sub-tasks
