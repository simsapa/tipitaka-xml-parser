# Tasks: Insert New XML Fragments

Generated from [prd-insert-fragment.md](./prd-insert-fragment.md)

## Relevant Files

**Core Types & Models:**
- `src/types.rs` - `FragmentKey`, `XmlFragment`, `CorrectionFragmentOverride`, `ParserOverrides` structs
- `src/fragments_models.rs` - Diesel ORM models (`NewXmlFragment`, `XmlFragmentRecord`, changesets)
- `src/fragments_schema.rs` - Auto-generated Diesel schema definitions

**Database & Migrations:**
- `migrations/fragments/2025-01-01-000000_create_fragments_tables/up.sql` - Existing migration
- `migrations/fragments/2025-01-02-000000_frag_idx_to_frag_idx_code/up.sql` - New migration (to create)
- `migrations/fragments/2025-01-02-000000_frag_idx_to_frag_idx_code/down.sql` - New migration rollback (to create)
- `src/fragment_exporter.rs` - DB export, `extract_correction_overrides()`, migration runner, `establish_connection_and_migrate()` helper

**Parsing Pipeline:**
- `src/xml_parser.rs` - Entry point `parse_into_fragments()`, calls parser then `apply_sc_overrides()`
- `src/parsers/helpers.rs` - `apply_fragment_adjustment()`, `apply_sc_overrides()`, `get_boundary_override()`
- `src/parsers/general.rs` - General parser with boundary chaining logic
- `src/parsers/samyutta_nikaya_mula.rs` - SN Mula parser
- `src/parsers/samyutta_nikaya_commentary.rs` - SN commentary parser

**Fragment Operations:**
- `src/fragment_operations.rs` - Move, shift, `find_target_fragment()` logic
- `src/fragment_reconstructor.rs` - XML reconstruction from DB fragments

**Web API & UI:**
- `src/web/routes.rs` - All API endpoints including reparse, regenerate
- `src/web/models.rs` - Request/response DTOs
- `src/web/state.rs` - Application state and DB connection
- `src/web/validation.rs` - Validation logic
- `src/static/index.html` - HTML template with move buttons, confirmation modal
- `src/static/scripts/app.js` - JS logic for fragment operations, display, API calls

**Regeneration:**
- `src/regenerate.rs` - `regenerate_fragments_db()`, `parse_tipitaka_xml_files()`
- `src/integration.rs` - `TipitakaImporter` high-level API

**Tests:**
- `tests/test_fragment_move_operations.rs` - Move operation tests
- `tests/test_checked_fragment_overrides.rs` - Override preservation tests
- `tests/test_single_file_reparse.rs` - Reparse tests
- `tests/test_regenerate_with_reference.rs` - Regeneration tests
- `tests/test_xml_fragment_position_tracking.rs` - Position tracking tests
- `tests/test_fragment_insertion.rs` - New: insertion tests (to create)
- `tests/test_inserted_fragment_regeneration.rs` - New: regeneration preservation tests (to create)

### Notes

- Build: `cargo build` or `cargo build --release`
- Test all: `cargo test`
- Test single file: `cargo test --test test_fragment_move_operations`
- Test single function: `cargo test test_move_to_prev`
- Diesel CLI: `diesel migration run --migration-dir migrations/fragments/`
- After modifying migrations, regenerate schema: `diesel print-schema --database-url /path/to/db.sqlite > src/fragments_schema.rs`

## Tasks

### 1. Database Schema Versioning and Migration Infrastructure

> **PRD §4.1, §7 "Schema Versioning":** The database needs a schema version tracking
> mechanism. On connection, check if migrations need to run and auto-apply them.
> Diesel's `embed_migrations!()` bundles migrations into the binary, and
> `run_pending_migrations()` applies unapplied ones. Currently, `run_migrations()`
> in `fragment_exporter.rs` already uses `MigrationHarness` — this task ensures it
> runs consistently on every DB connection, not just during export.

- [x] 1.0 Set up Diesel embedded migrations and auto-run on DB connection
  - [x] 1.1 Audit all locations where `SqliteConnection::establish()` is called (fragment_exporter.rs, web/state.rs, fragment_operations.rs, tests) and identify which ones already run migrations
  - [x] 1.2 Create a shared `establish_connection_and_migrate(db_path: &Path) -> Result<SqliteConnection>` helper function that establishes the connection and runs pending migrations in one call
  - [x] 1.3 Replace all direct `SqliteConnection::establish()` calls with the new helper, ensuring migrations auto-run on every DB connection
  - [x] 1.4 Verify the existing `embed_migrations!("migrations/fragments/")` macro is used and that `MigrationHarness::run_pending_migrations()` is the mechanism (not manual SQL)
  - [x] 1.5 Test that opening an existing DB with the old schema triggers auto-migration, and opening a fresh DB creates the schema from scratch

### 2. Database Migration: `frag_idx` to `frag_idx_code`

> **PRD §4.2, §7 "Database Migration":** SQLite doesn't support `ALTER COLUMN`, so
> the migration must create a new table with `frag_idx_code TEXT`, copy data with
> conversion (`20` → `"20.0"` using string concatenation), drop the old table, and
> rename. The `frag_idx` to `frag_idx_code` data conversion is done in the migration
> SQL itself.

- [x] 2.0 Create DB migration: rename `frag_idx` INTEGER to `frag_idx_code` TEXT with data conversion
  - [x] 2.1 Create new migration directory `migrations/fragments/2025-01-02-000000_frag_idx_to_frag_idx_code/`
  - [x] 2.2 Write `up.sql`: create `xml_fragments_new` table with `frag_idx_code TEXT NOT NULL` replacing `frag_idx INTEGER NOT NULL`; `INSERT INTO xml_fragments_new SELECT` with `CAST(frag_idx AS TEXT) || '.0'` for the conversion; `DROP TABLE xml_fragments`; `ALTER TABLE xml_fragments_new RENAME TO xml_fragments`
  - [x] 2.3 Write `down.sql`: reverse migration that converts `frag_idx_code` back to integer (strip `.0` suffix), handling that inserted fragments (sub-index > 0) would be lost on rollback
  - [x] 2.4 Run `diesel migration run` and regenerate `src/fragments_schema.rs` with `diesel print-schema` to reflect the new column type
  - [x] 2.5 Verify the migration works on an existing database with real data (check a few rows show `"0.0"`, `"1.0"`, etc.)

### 3. Update Rust Types, Models, and Schema

> **PRD §4.2 requirements 7–9:** `FragmentKey` changes from `frag_idx: usize` to
> `frag_idx_code: String`. `XmlFragment` likewise. The Diesel models
> (`XmlFragmentRecord`, `NewXmlFragment`, changesets) must match the new schema
> column type. `CorrectionFragmentOverrides` is `HashMap<FragmentKey, ...>` so the
> key change propagates automatically once `FragmentKey` is updated.

- [x] 3.0 Update Rust types, models, and schema for `frag_idx_code`
  - [x] 3.1 Update `FragmentKey` in `types.rs`: change `frag_idx: usize` to `frag_idx_code: String`
  - [x] 3.2 Update `XmlFragment` in `types.rs`: change `frag_idx: usize` to `frag_idx_code: String`
  - [x] 3.3 Update `XmlFragmentRecord` in `fragments_models.rs`: change `frag_idx: i32` to `frag_idx_code: String`
  - [x] 3.4 Update `NewXmlFragment` in `fragments_models.rs`: change `frag_idx: i32` to `frag_idx_code: &'a str`
  - [x] 3.5 Update all changeset structs (`UpdateFragmentIndex`, etc.) that reference `frag_idx` to use `frag_idx_code`
  - [x] 3.6 Ensure `fragments_schema.rs` reflects `frag_idx_code -> Text` (should already be done in task 2.4)
  - [x] 3.7 Run `cargo check` — fix all compilation errors from the type changes across the codebase (this will surface every location that needs updating)

### 4. Implement `frag_idx_code` Version-Style Sorting

> **PRD §4.2 requirement 12, §7 "Sorting":** `frag_idx_code` must sort numerically
> by splitting on `"."` — e.g., `"2.0" < "2.1" < "10.0"`, not lexicographic.
> This comparator is needed in Rust (sorting fragment vectors), SQL (`ORDER BY` in
> queries), and JavaScript (fragment list display, validation error sorting).

- [x] 4.0 Implement `frag_idx_code` version-style sorting utilities (Rust + JS)
  - [x] 4.1 Create a `parse_frag_idx_code(code: &str) -> (usize, usize)` helper in `types.rs` (or a new `frag_idx_code.rs` module) that splits `"21.3"` into `(21, 3)`
  - [x] 4.2 Implement `Ord`/`PartialOrd` for a `FragIdxCode` wrapper type (or a standalone comparison function `compare_frag_idx_code(a: &str, b: &str) -> Ordering`) for Rust-side sorting
  - [x] 4.3 Add a helper `next_sub_index(existing_codes: &[&str], major: usize) -> String` that finds the next available sub-index for a given major index (e.g., if `"21.0"` and `"21.1"` exist, returns `"21.2"`)
  - [x] 4.4 Add a helper `format_frag_idx_code(major: usize, minor: usize) -> String` for consistent formatting
  - [x] 4.5 For SQL ordering: update all `ORDER BY frag_idx` queries to use an expression that sorts numerically (e.g., `ORDER BY CAST(substr(frag_idx_code, 1, instr(frag_idx_code, '.')-1) AS INTEGER), CAST(substr(frag_idx_code, instr(frag_idx_code, '.')+1) AS INTEGER)`) — or apply Rust-side sorting after query
  - [x] 4.6 Add JavaScript `compareFragIdxCode(a, b)` function in `app.js` that splits on `"."` and compares numerically
  - [x] 4.7 Write unit tests for the Rust sorting/parsing helpers covering edge cases: `"0.0"`, `"9.0" vs "10.0"`, `"21.0" < "21.1" < "21.2" < "22.0"`

### 5. Update Parsers and Fragment Generation

> **PRD §4.2 requirement 10:** All parsers that produce fragments assign `frag_idx`
> sequentially as `fragments.len()`. These must now assign `frag_idx_code` in
> `"N.0"` format. The key locations are: `general.rs` (main parsing loop where
> fragments are pushed), `samyutta_nikaya_mula.rs`, `samyutta_nikaya_commentary.rs`,
> and `xml_parser.rs`. The `apply_fragment_adjustment()` function in
> `parsers/helpers.rs` looks up overrides by `FragmentKey(cst_file, frag_idx)` —
> this must use `frag_idx_code`.

- [ ] 5.0 Update all parsers and fragment generation to use `frag_idx_code`
  - [ ] 5.1 Update `parsers/general.rs`: when pushing fragments, set `frag_idx_code: format!("{}.0", fragments.len())` instead of `frag_idx: fragments.len()`
  - [ ] 5.2 Update `parsers/samyutta_nikaya_mula.rs`: same pattern — assign `frag_idx_code` as `"N.0"` when creating fragments
  - [ ] 5.3 Update `parsers/samyutta_nikaya_commentary.rs`: same pattern
  - [ ] 5.4 Update `apply_fragment_adjustment()` in `parsers/helpers.rs`: build `FragmentKey` with `frag_idx_code` string instead of `frag_idx` usize; update the `get_boundary_override()` helper likewise
  - [ ] 5.5 Update `apply_sc_overrides()` in `parsers/helpers.rs`: build `FragmentKey` with `frag_idx_code` from each fragment
  - [ ] 5.6 Update `xml_parser.rs` `parse_into_fragments()`: ensure fragment indexing uses the new code format
  - [ ] 5.7 Run `cargo check` to verify all parser code compiles

### 6. Update CorrectionFragmentOverrides Pipeline

> **PRD §4.6, §7 "Override Key Changes":** `CorrectionFragmentOverrides` is
> `HashMap<FragmentKey, CorrectionFragmentOverride>`. Since `FragmentKey` now uses
> `frag_idx_code: String`, the extraction in `extract_correction_overrides()` and
> `extract_all_correction_overrides()` must read `frag_idx_code` from the DB and
> build keys accordingly. The override application in `apply_fragment_adjustment()`
> and `apply_sc_overrides()` already uses `FragmentKey` lookups — these were updated
> in task 5, but the extraction side needs updating here.

- [ ] 6.0 Update `CorrectionFragmentOverrides` pipeline for `frag_idx_code`
  - [ ] 6.1 Update `extract_correction_overrides()` in `fragment_exporter.rs`: read `frag_idx_code` (String) instead of `frag_idx` (i32) from query results; build `FragmentKey` with the string code
  - [ ] 6.2 Update `extract_all_correction_overrides()` in `fragment_exporter.rs`: same changes for the all-files variant
  - [ ] 6.3 Update any frag_review status map returned alongside overrides (the `HashMap<usize, String>` keyed by frag_idx) to use `String` keys matching `frag_idx_code`
  - [ ] 6.4 Verify that the `CorrectionFragmentOverride` struct itself needs no changes (it stores override data, not the key)
  - [ ] 6.5 Run `cargo check` to verify the override pipeline compiles end-to-end

### 7. Update Fragment Operations

> **PRD §4.7 requirement 32:** The existing fragment operations — move
> (`move_fragment_content`), delete, create, boundary adjust — all use `frag_idx`
> for lookups, adjacent fragment finding, and index shifting. These must use
> `frag_idx_code`. The `find_target_fragment()` helper iterates adjacent fragments
> by incrementing/decrementing `frag_idx` — this must now sort by `frag_idx_code`
> and navigate by sorted position. Index shifting logic (incrementing frag_idx for
> fragments after a deletion/creation) must be adapted for the string format.

- [ ] 7.0 Update fragment operations (move, delete, create, boundary adjust) for `frag_idx_code`
  - [ ] 7.1 Update `find_target_fragment()` in `fragment_operations.rs`: query fragments sorted by `frag_idx_code` (using version-sort), find adjacent non-moved fragment by position in the sorted list rather than incrementing an integer index
  - [ ] 7.2 Update `move_fragment_content()`: use `frag_idx_code` for fragment lookups and the response
  - [ ] 7.3 Update fragment deletion logic: when deleting, no need to shift `frag_idx_code` values (the string codes remain stable, unlike integer indices that were decremented)
  - [ ] 7.4 Update fragment creation logic in `fragment_operations.rs` (the existing `create_fragment` if present): use `frag_idx_code` for the new fragment
  - [ ] 7.5 Update boundary adjustment logic: use `frag_idx_code` to identify the fragment and its neighbors
  - [ ] 7.6 Remove or update any `UpdateFragmentIndex` changeset usage that shifted integer indices — with `frag_idx_code`, adjacent fragments don't need reindexing on insert/delete
  - [ ] 7.7 Run `cargo check` to verify all operation code compiles

### 8. Update Web API Routes and Response Models

> **PRD §4.5, §4.2 requirement 10:** All API endpoints in `routes.rs` that accept
> or return `frag_idx` must switch to `frag_idx_code`. This includes fragment list
> endpoints, fragment detail, metadata update, boundary adjust, move, delete, create,
> reparse, and regenerate. The DTOs in `web/models.rs` (request/response structs)
> must also be updated. The validation endpoints that report errors by `frag_idx`
> need updating too.

- [ ] 8.0 Update web API routes and response models for `frag_idx_code`
  - [ ] 8.1 Update all request DTOs in `web/models.rs` that contain `frag_idx` (e.g., `MoveFragmentRequest`, any create/delete request structs) to use `frag_idx_code: String`
  - [ ] 8.2 Update all response DTOs and JSON serialization that return `frag_idx` to return `frag_idx_code`
  - [ ] 8.3 Update `routes.rs` fragment list endpoint: ensure fragments are returned sorted by `frag_idx_code` using version-sort
  - [ ] 8.4 Update `routes.rs` fragment detail endpoint: use `frag_idx_code` in lookups
  - [ ] 8.5 Update `routes.rs` move endpoint (`/api/fragments/move`): accept and use `frag_idx_code`
  - [ ] 8.6 Update `routes.rs` boundary adjust endpoint: use `frag_idx_code` for fragment identification
  - [ ] 8.7 Update `routes.rs` delete and existing create endpoints: use `frag_idx_code`
  - [ ] 8.8 Update `routes.rs` reparse endpoint: ensure override extraction and result reporting use `frag_idx_code`
  - [ ] 8.9 Update validation endpoints (`web/validation.rs`): report errors with `frag_idx_code` instead of `frag_idx`
  - [ ] 8.10 Run `cargo check` to verify all route code compiles

### 9. Update JavaScript/HTML UI

> **PRD §4.2 requirement 11:** There are ~14 references to `frag_idx` in `app.js`
> — used for display tags, `dataset.fragIdx`, API request bodies, sorting, error
> reporting, and fragment navigation. All must change to `frag_idx_code`. The
> `index.html` template may also reference `frag_idx` in data attributes or display.
> Sorting in JS must use the version-style comparator from task 4.6.

- [ ] 9.0 Update JavaScript/HTML UI to use `frag_idx_code`
  - [ ] 9.1 Update `app.js`: replace all `fragment.frag_idx` references with `fragment.frag_idx_code` (display tags, dataset attributes, API request bodies)
  - [ ] 9.2 Update `app.js`: replace `item.dataset.fragIdx` with `item.dataset.fragIdxCode` and all corresponding lookups
  - [ ] 9.3 Update `app.js` `moveFragmentTo()`: send `frag_idx_code` in the request body instead of `frag_idx`
  - [ ] 9.4 Update `app.js` sorting: replace `a.frag_idx - b.frag_idx` with the `compareFragIdxCode()` function from task 4.6
  - [ ] 9.5 Update `app.js` validation error display: use `frag_idx_code` in error location strings and `openFragmentFromValidation()` calls
  - [ ] 9.6 Update `index.html` if any inline references or data attributes use `frag_idx`
  - [ ] 9.7 Test the web UI manually: verify fragment list displays correctly, sorting works, move operations work, validation errors show correct codes

### 10. Fix Existing Tests

> **PRD §8:** All existing tests must pass after the migration. The 18+ integration
> tests in `tests/` reference `frag_idx` in assertions, fragment construction, and
> override setup. Each test file needs updating to use `frag_idx_code` strings.
> This is a mechanical but broad change — every test that constructs `XmlFragment`,
> `FragmentKey`, or asserts on `frag_idx` values needs updating.

- [ ] 10.0 Fix all existing tests to pass with `frag_idx_code`
  - [ ] 10.1 Update `test_fragment_move_operations.rs`: change all `frag_idx` references to `frag_idx_code` strings (e.g., `frag_idx: 0` → `frag_idx_code: "0.0".to_string()`)
  - [ ] 10.2 Update `test_checked_fragment_overrides.rs`: update override key construction and assertions
  - [ ] 10.3 Update `test_single_file_reparse.rs`: update fragment assertions and override setup
  - [ ] 10.4 Update `test_regenerate_with_reference.rs`: update fragment assertions
  - [ ] 10.5 Update `test_xml_fragment_position_tracking.rs`: update fragment index assertions
  - [ ] 10.6 Update `test_fragment_validation.rs`: update any frag_idx references
  - [ ] 10.7 Update all remaining SN test files (`test_sn_*.rs`, `test_sutta_boundary_splitting.rs`, `test_s0303m_sc_code_propagation.rs`, `test_sc_code_arangodb_range_lookup.rs`): change frag_idx references to frag_idx_code
  - [ ] 10.8 Update `src/test_tsv_validation.rs` (unit tests): change frag_idx references
  - [ ] 10.9 Run `cargo test` — all existing tests must pass

### 11. Implement Insert Fragment Backend

> **PRD §4.4, §4.5, §4.7 requirement 30:** The insert operation creates a new
> fragment with `frag_idx_code` derived from the insertion position (e.g., `"21.1"`
> between `"21.0"` and `"22.0"`). It copies metadata from the adjacent fragment,
> sets `frag_review = "checked"`, and uses "zero-width" boundaries — the same
> position as the boundary between the two neighbors (NOT literal zeros). The API
> endpoint is `POST /api/fragments/insert` accepting `frag_idx_code`, `cst_file`,
> and `direction`. Skip moved fragments when finding the insertion point.

- [ ] 11.0 Implement insert fragment backend (API endpoint + operation logic)
  - [ ] 11.1 Add `insert_fragment()` function in `fragment_operations.rs` that:
    - Loads the current fragment by `frag_idx_code` and `cst_file`
    - Finds the neighbor fragment in the given direction (skipping moved fragments, reusing `find_target_fragment()` logic)
    - Computes the `frag_idx_code` for the new fragment using `next_sub_index()` from task 4.3
    - Sets zero-width boundaries: `start_line/char = end_line/char` of the preceding fragment (for "insert after") or `start_line/char` of the following fragment (for "insert before")
    - Copies metadata from the adjacent fragment (cst_code, sc_code, cst_vagga, cst_sutta, cst_paranum, sc_sutta, nikaya, group_levels, frag_type)
    - Sets `content_xml = ""`, `frag_review = "checked"`
    - Inserts the new record into the database
    - Returns the new fragment record
  - [ ] 11.2 Add request/response DTOs in `web/models.rs`: `InsertFragmentRequest { frag_idx_code, cst_file, direction }` and response with the new fragment
  - [ ] 11.3 Add `POST /api/fragments/insert` route in `routes.rs` that calls the operation and returns the new fragment plus updated fragment list
  - [ ] 11.4 Run `cargo check` to verify the new endpoint compiles

### 12. Implement Insert Fragment UI

> **PRD §4.3, §6:** Add "Add new before" (A&uarr;) and "Add new after" (A&darr;) buttons
> after the existing move buttons. Use `showConfirmModal()` for confirmation.
> Disable when no fragment selected or current fragment is "moved". After insertion,
> refresh the fragment list and auto-select the new fragment.

- [ ] 12.0 Implement insert fragment UI (buttons, confirmation, refresh)
  - [ ] 12.1 Add the two buttons in `index.html`: "A&uarr;" (`id="add-new-before"`) after `id="move-to-prev"`, and "A&darr;" (`id="add-new-after"`) after `id="move-to-next"`, with similar styling to the move buttons
  - [ ] 12.2 Add `insertFragment(direction)` function in `app.js` that sends `POST /api/fragments/insert` with `frag_idx_code`, `cst_file`, and direction
  - [ ] 12.3 Add click handlers for both buttons using `showConfirmModal()`: "Insert a new empty fragment BEFORE/AFTER the current fragment?"
  - [ ] 12.4 On successful insertion: refresh the fragment list for the current file, auto-select the newly inserted fragment
  - [ ] 12.5 Add disable logic: both buttons disabled when no fragment is selected or when `currentFragment.frag_review === "moved"`
  - [ ] 12.6 Test manually in the web UI: insert before/after, verify the new fragment appears in correct position, verify metadata is copied, verify boundaries are zero-width

### 13. Adapt Regeneration Pipeline for Inserted Fragments

> **PRD §4.6 requirements 25–29, §7 "Regeneration Logic":** Inserted fragments
> (sub-index > 0) must survive regeneration. The existing pipeline has three
> integration points:
>
> 1. `extract_correction_overrides()` — must extract inserted fragments with their
>    full content_xml and boundaries.
> 2. `apply_fragment_adjustment()` — for a generated fragment at `"N.0"` preceding
>    an inserted `"N.1"`, apply an end_line/end_char override to truncate `"N.0"`
>    at the inserted fragment's start boundary.
> 3. Post-parsing injection — after boundary adjustments, inject the inserted
>    fragments into the fragment list at their correct sorted positions. The
>    existing chaining mechanism (next fragment starts where previous ends) handles
>    the subsequent fragment's start position.
>
> This adapts the existing pipeline rather than adding a new stage.

- [ ] 13.0 Adapt regeneration pipeline to preserve and re-inject inserted fragments
  - [ ] 13.1 Extend `CorrectionFragmentOverride` (or add a parallel `InsertedFragmentOverride` struct) to store full fragment data for inserted fragments: content_xml, start_line, start_char, end_line, end_char, plus all metadata fields
  - [ ] 13.2 Update `extract_correction_overrides()` to detect inserted fragments (sub-index > 0 in `frag_idx_code`) and extract them with full content and boundary data, storing them in the overrides map or a separate collection in `ParserOverrides`
  - [ ] 13.3 Update `extract_all_correction_overrides()` with the same logic for full regeneration
  - [ ] 13.4 Update `apply_fragment_adjustment()` in `parsers/helpers.rs`: when finalizing a generated fragment at `"N.0"`, check if there are inserted fragments `"N.1"`, `"N.2"`, etc. in the overrides — if so, apply the first inserted fragment's start boundary as the end_line/end_char override for `"N.0"`
  - [ ] 13.5 Add a post-parsing injection step in `xml_parser.rs` `parse_into_fragments()` (after `apply_sc_overrides()`): iterate through the fragment list, and at each `"N.0"` that has associated inserted fragments, splice them into the list at the correct positions
  - [ ] 13.6 Ensure the injected fragments have their metadata restored (frag_review, sc_code, cst_code, etc.) — either during injection or via `apply_sc_overrides()` processing them
  - [ ] 13.7 Update `fragment_exporter.rs` export logic to handle the mixed generated + inserted fragment list (the export writes all fragments sequentially, so it should work if the list is correctly ordered)
  - [ ] 13.8 Test with reparse: insert a fragment, reparse the file, verify the inserted fragment is preserved with correct boundaries and content
  - [ ] 13.9 Test with full regeneration: insert a fragment, regenerate the entire DB, verify the inserted fragment is preserved

### 14. Integration Tests for Fragment Insertion

> **PRD §8, §4.7:** Tests must verify: (a) correct insertion with proper
> `frag_idx_code` assignment, (b) zero-width boundaries at the correct position,
> (c) boundary adjustment moves content correctly, (d) XML reconstruction produces
> the original file after splitting, (e) inserted fragments survive regeneration
> and reparse with correct boundaries. The existing XML reconstruction verification
> (`ReconstructionVerificationFailed` error) guards against content gaps/overlaps.

- [ ] 14.0 Add integration tests for fragment insertion and regeneration preservation
  - [ ] 14.1 Create `tests/test_fragment_insertion.rs` with test: insert a fragment between two generated fragments, verify `frag_idx_code` is correctly assigned (e.g., `"21.1"` between `"21.0"` and `"22.0"`)
  - [ ] 14.2 Add test: verify the inserted fragment has zero-width boundaries matching the boundary between its neighbors
  - [ ] 14.3 Add test: insert multiple fragments at the same major index (e.g., `"21.1"`, `"21.2"`), verify correct sub-index assignment
  - [ ] 14.4 Add test: insert near a moved fragment, verify it skips the moved fragment and finds the correct non-moved neighbor
  - [ ] 14.5 Add test: after insertion, adjust boundaries to give the inserted fragment content, then verify XML reconstruction (concatenation of all fragments' content_xml reproduces the original XML)
  - [ ] 14.6 Create `tests/test_inserted_fragment_regeneration.rs` with test: insert a fragment, give it content via boundary adjustment, then reparse the file — verify the inserted fragment is preserved with its content and boundaries
  - [ ] 14.7 Add test: insert a fragment, regenerate the full DB with reference — verify the inserted fragment survives with correct data
  - [ ] 14.8 Add test: insert a fragment with zero-width (no content yet), regenerate — verify it is preserved as zero-width
  - [ ] 14.9 Run `cargo test` — all new and existing tests must pass
