## Relevant Files

- `src/types.rs` - Added `CheckedFragmentOverride`, `CheckedFragmentOverrides`, `ParserOverrides`, `ScCodeComponents` types
- `src/fragment_exporter.rs` - Added `extract_checked_overrides()`, `extract_all_checked_overrides()`, and `restore_frag_review_status()` functions
- `src/parsers/helpers.rs` - Added `parse_sc_code()`, `get_boundary_override()`, `apply_boundary_override()`, `apply_sc_overrides()`, `derive_sc_code_from_context()`, `format_sc_code()`, `populate_sc_fields_from_tsv_conditional()`
- `src/parsers/samyutta_nikaya_mula.rs` - Update post-processing to apply SC overrides
- `src/parsers/majjhima_nikaya_mula.rs` - Update post-processing to apply SC overrides
- `src/parsers/digha_nikaya_mula.rs` - Update post-processing to apply SC overrides
- `src/parsers/anguttara_nikaya_mula.rs` - Update post-processing to apply SC overrides
- `src/parsers/general.rs` - Update to use `ParserOverrides`
- `src/xml_parser.rs` - Update dispatcher signatures to pass `ParserOverrides`
- `src/xml_parser_trait.rs` - Update `XmlParser` trait to accept `ParserOverrides`
- `src/integration.rs` - Update `TipitakaImporter` to accept and pass `ParserOverrides`
- `src/main.rs` - Load checked overrides from reference DB, update CLI flow
- `src/web/routes.rs` - Add `/api/reparse-file` endpoint
- `src/static/index.html` - Add reparse button to file list items
- `src/static/scripts/app.js` - Add reparse button rendering, confirmation modal, and API call handler
- `tests/test_checked_fragment_overrides.rs` - Integration tests for checked overrides
- `tests/test_single_file_reparse.rs` - Integration tests for single-file reparse flow

### Notes

- Unit tests should be placed in `tests/` for integration tests or `#[cfg(test)] mod` for unit tests
- Use `cargo test` to run all tests, `cargo test --test test_name` for specific test files
- Use `cargo test test_function_name` for specific test functions
- Lines are 1-indexed, characters are 0-indexed (document clearly in comments)
- CheckedFragmentOverrides take precedence over FragmentAdjustments
- SC overrides are applied in post-processing (after `derive_cst_fields()`, before TSV population)
- Boundary overrides are applied during fragment finalization in the parsing loop

## Tasks

- [x] 1.0 Add new types for CheckedFragmentOverrides and ParserOverrides
  - [x] 1.1 Add `CheckedFragmentOverride` struct to `src/types.rs` with fields: `end_line: Option<usize>`, `end_char: Option<usize>`, `sc_code: Option<String>`, `sc_sutta: Option<String>`. Use `#[derive(Debug, Clone, Default)]`.
  - [x] 1.2 Add `CheckedFragmentOverrides` type alias as `HashMap<FragmentKey, CheckedFragmentOverride>` to `src/types.rs`
  - [x] 1.3 Add `ParserOverrides` struct to `src/types.rs` with fields: `adjustments: Option<FragmentAdjustments>`, `checked_overrides: Option<CheckedFragmentOverrides>`. Use `#[derive(Debug, Clone, Default)]`.
  - [x] 1.4 Add `ScCodeComponents` struct to `src/types.rs` with fields: `prefix: String`, `samyutta: Option<i32>`, `nipata: Option<i32>`, `sutta: Option<i32>`. Use `#[derive(Debug, Clone, Default)]`.
  - [x] 1.5 Add doc comments for all new types explaining their purpose and relationship to `FragmentAdjustments`
  - [x] 1.6 Run `cargo build` to verify compilation succeeds

- [x] 2.0 Implement database extraction functions for checked overrides
  - [x] 2.1 Add `extract_checked_overrides()` function to `src/fragment_exporter.rs` that queries fragments where `frag_review NOT IN (NULL, '', 'unchecked')` for a given `cst_file` and returns `Result<(CheckedFragmentOverrides, HashMap<usize, String>)>` (overrides + frag_review status map)
  - [x] 2.2 Implement the SQL query: `SELECT frag_idx, end_line, end_char, sc_code, sc_sutta, frag_review FROM xml_fragments WHERE cst_file = ? AND frag_review NOT IN (NULL, '', 'unchecked')`
  - [x] 2.3 Build the `CheckedFragmentOverrides` HashMap keyed by `(cst_file, frag_idx)`
  - [x] 2.4 Build the `HashMap<usize, String>` for `frag_idx → frag_review` status restoration
  - [x] 2.5 Add `restore_frag_review_status()` function to `src/fragment_exporter.rs` that updates `frag_review` field for fragments matching the status map. Return count of updated fragments.
  - [x] 2.6 Add unit tests in `src/fragment_exporter.rs` for both functions using temporary database
  - [x] 2.7 Run `cargo test` to verify tests pass

- [x] 3.0 Implement SC code parsing and override application logic
  - [x] 3.1 Add `parse_sc_code()` function to `src/parsers/helpers.rs` that parses SC codes like `sn5.1`, `an3.1`, `dn1`, `mn41` and returns `Option<ScCodeComponents>` with extracted prefix, samyutta/nipata, and sutta numbers
  - [x] 3.2 Add `get_boundary_override()` helper function that checks `CheckedFragmentOverrides` first, then falls back to `FragmentAdjustments`. Returns `Option<(usize, usize)>` for `(end_line, end_char)`.
  - [x] 3.3 Add `apply_sc_overrides()` function that: (1) applies SC overrides directly to overridden fragments, (2) parses `sc_code` to extract context, (3) propagates context to subsequent fragments with null `sc_code` until hitting a non-null fragment
  - [x] 3.4 Add `derive_sc_code_from_context()` helper that derives `sc_code` for a fragment using its `cst_code` and the propagated context (e.g., samyutta number)
  - [x] 3.5 Add `populate_sc_fields_from_tsv_conditional()` function that only populates SC fields for fragments where `sc_code` is `None` (skips fragments already set by overrides)
  - [x] 3.6 Add unit tests for `parse_sc_code()` covering DN, MN, SN, AN patterns
  - [x] 3.7 Add unit tests for `apply_sc_overrides()` verifying context propagation
  - [x] 3.8 Run `cargo test` to verify tests pass

- [x] 4.0 Integrate ParserOverrides into the parsing pipeline
  - [x] 4.1 Update `XmlParser` trait in `src/xml_parser_trait.rs` to accept `&ParserOverrides` parameter in the `parse()` method signature
  - [x] 4.2 Update `parse_into_fragments()` in `src/xml_parser.rs` to accept `&ParserOverrides` and pass it to parser implementations
  - [x] 4.3 Update `SamyuttaNikayaMula` parser to: (1) use boundary overrides from `ParserOverrides` during parsing, (2) call `apply_sc_overrides()` after `derive_cst_fields()`, (3) call `populate_sc_fields_from_tsv_conditional()` instead of unconditional version
  - [x] 4.4 Update `MajjhimaNikayaMula`, `DighaNikayaMula`, `AnguttaraNikayaMula` parsers similarly
  - [x] 4.5 Update `GeneralParser` in `src/parsers/general.rs` to use `ParserOverrides`
  - [x] 4.6 Update `TipitakaImporter` in `src/integration.rs`: replace `adjustments: Option<FragmentAdjustments>` with `overrides: ParserOverrides`, update `with_adjustments()` to `with_overrides()`, pass overrides through the pipeline
  - [x] 4.7 Update call sites in `src/main.rs` to construct and pass `ParserOverrides`
  - [x] 4.8 Run `cargo build` and `cargo test` to verify all changes compile and existing tests pass

- [x] 5.0 Implement the backend API endpoint for single-file reparse
  - [x] 5.1 Add `ReparseFileRequest` struct in `src/web/routes.rs` with field `cst_file: String`
  - [x] 5.2 Add `POST /api/reparse-file` endpoint handler function `reparse_file()` in `src/web/routes.rs`
  - [x] 5.3 Implement validation: check that the file exists in current database
  - [x] 5.4 Implement the reparse flow: (1) extract `CheckedFragmentOverrides` and `frag_review` status from current DB, (2) load `FragmentAdjustments` from embedded TSV, (3) construct `ParserOverrides`, (4) parse the single XML file, (5) export fragments to DB, (6) restore `frag_review` status
  - [x] 5.5 Return response with success/error status and output messages (same structure as regeneration response)
  - [x] 5.6 Register the new endpoint in the router
  - [x] 5.7 Run `cargo build` to verify compilation

- [x] 6.0 Implement the frontend UI for reparse button and modal
  - [x] 6.1 Add a reload icon button (e.g., `🔄` or Font Awesome `fa-sync`) to each file item in the XML Files dropdown in `src/static/index.html`. Position it to the right of the fragment count.
  - [x] 6.2 Add tooltip to the button: "Reparse this file using current DB as reference"
  - [x] 6.3 Add CSS styling for the reparse button to match existing action buttons
  - [x] 6.4 Add `reparseFile(cstFile)` function in `src/static/scripts/app.js` that: (1) shows confirmation dialog, (2) on confirm, calls `POST /api/reparse-file`, (3) shows output in regeneration modal, (4) auto-refreshes file list and fragments on completion
  - [x] 6.5 Add click event handler for reparse buttons that calls `reparseFile()` with the file name
  - [x] 6.6 Disable reparse buttons while any regeneration or reparse operation is in progress
  - [x] 6.7 Test the UI manually: start the web server, verify button appears, verify confirmation dialog, verify modal output display

- [x] 7.0 Update full database regeneration to use CheckedFragmentOverrides
  - [x] 7.1 Update `regenerate()` endpoint in `src/web/routes.rs` to extract `CheckedFragmentOverrides` from reference DB for ALL files when `use_reference_db = true`
  - [x] 7.2 Pass the loaded overrides to `TipitakaImporter` via `ParserOverrides`
  - [x] 7.3 Update `parse_tipitaka_xml()` in `src/main.rs` to load `CheckedFragmentOverrides` from reference DB when `--reference-fragments-db` is provided
  - [x] 7.4 Ensure consistent behavior between single-file reparse and full regeneration
  - [x] 7.5 Run `cargo build` and manual test to verify regeneration still works correctly

- [x] 8.0 Add integration tests for the reparse feature
  - [x] 8.1 Create `tests/test_checked_fragment_overrides.rs` with test for `CheckedFragmentOverrides` precedence over `FragmentAdjustments`
  - [x] 8.2 Add test for SC code parsing covering all nikaya types (DN, MN, SN, AN)
  - [x] 8.3 Add test for SC override propagation to subsequent null fragments
  - [x] 8.4 Add test for boundary override from checked fragment
  - [x] 8.5 Create `tests/test_single_file_reparse.rs` with test for idempotent single-file reparse (reparse twice, compare fragment data)
  - [x] 8.6 Add test using `s0301m.mul.xml` test case: verify fragments 162-171 get correct `sc_code` after checking fragment 162
  - [x] 8.7 Run `cargo test` to verify all integration tests pass
