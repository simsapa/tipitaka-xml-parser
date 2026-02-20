# PRD: Insert New XML Fragments

## 1. Introduction/Overview

The Tipitaka XML parser splits XML files into fragments that map to sutta boundaries. However, some suttas have irregular boundaries that the automated parser cannot detect correctly. This feature allows users to **manually insert new fragments** before or after a selected fragment in the web UI, enabling fine-grained control over sutta boundary splitting.

Additionally, the `frag_idx` integer field will be migrated to a string-based `frag_idx_code` field (e.g., `"20.0"`, `"20.1"`) to support inserted fragments without conflicting index renumbering.

## 2. Goals

1. Allow users to insert a new empty fragment before or after the currently selected fragment in the web UI.
2. Migrate `frag_idx` (integer) to `frag_idx_code` (string) across the database schema, Rust models, and all code references.
3. Add a schema version to the database and auto-run migrations with data conversion on connection.
4. Preserve inserted fragments during database regeneration by adapting the existing `CorrectionFragmentOverrides` pipeline.
5. Ensure correct XML content reconstruction from fragments after insertion and boundary adjustment.

## 3. User Stories

- **As a reviewer**, I want to insert a new fragment between two existing fragments so that I can define a sutta boundary that the parser missed.
- **As a reviewer**, I want inserted fragments to survive database regeneration so that my manual corrections are not lost.
- **As a reviewer**, I want to use the existing boundary adjustment buttons after inserting a fragment so that I can move content from the neighbor into the new fragment.

## 4. Functional Requirements

### 4.1 Database Schema Versioning

1. Add a `schema_version` table (or pragma-based mechanism) to track the current database schema version.
2. On database connection, check the current schema version and determine if migrations need to run.
3. If the schema is at a version before the `frag_idx_code` migration, automatically run the migration and convert existing data (e.g., `frag_idx` integer `20` becomes `frag_idx_code` string `"20.0"`).
4. Use Diesel's built-in migration runner (`diesel_migrations::run_pending_migrations` or equivalent) to manage schema evolution.

### 4.2 Database Migration: `frag_idx` to `frag_idx_code`

5. Create a new database migration that renames/replaces the `frag_idx INTEGER` column with `frag_idx_code TEXT` in the `xml_fragments` table.
6. Existing integer values must be converted to string format: `frag_idx` value `20` becomes `frag_idx_code` value `"20.0"`.
7. Update the Diesel model (`fragments_models.rs`) and schema to reflect the new column type.
8. Update `FragmentKey` in `types.rs` to use `String` for `frag_idx_code` instead of `usize` for `frag_idx`.
9. Update `CorrectionFragmentOverrides` (keyed by `FragmentKey`) to use the new string-based key.
10. Update all Rust code that references `frag_idx` to use `frag_idx_code` — this includes:
    - `fragment_parser.rs` (fragment creation, sequential assignment)
    - `fragment_exporter.rs` (export and override extraction)
    - `fragment_operations.rs` (move, delete, create, boundary adjust)
    - `routes.rs` (all API endpoints)
    - `parsers/helpers.rs` (`apply_sc_overrides`, `apply_fragment_adjustment`)
    - All parser files that produce fragments
11. Update all JavaScript/HTML references in `app.js` and `index.html` from `frag_idx` to `frag_idx_code`.
12. Ordering: fragments within a file must be sorted by `frag_idx_code` using version-style numeric comparison (e.g., `"2.0" < "2.1" < "10.0"`), not lexicographic sorting.

### 4.3 Web UI: Insert Buttons

13. Add an **"Add new before" (A&uarr;)** button after the existing `id="move-to-prev"` button in `index.html`.
14. Add an **"Add new after" (A&darr;)** button after the existing `id="move-to-next"` button in `index.html`.
15. Both buttons should use a similar styling to the move buttons (danger-styled or a distinct color to indicate a significant action).
16. Clicking either button must show a confirmation modal (reuse `showConfirmModal`) before executing the insert.
17. The buttons should be disabled when:
    - No fragment is currently selected.
    - The currently selected fragment has `frag_review = "moved"` (skip moved fragments).

### 4.4 Insert Operation Logic

18. **"Add new before"**: Insert a new fragment before the currently selected fragment.
19. **"Add new after"**: Insert a new fragment after the currently selected fragment.
20. Skip moved/collapsed fragments when determining the insertion point (same logic as `find_target_fragment` for move operations).
21. The new fragment is created with:
    - `frag_idx_code`: derived from the insertion position. If inserting between `"21.0"` and `"22.0"`, the new fragment gets `"21.1"`. If inserting after an already-inserted `"21.1"`, it gets `"21.2"`.
    - `content_xml`: empty string `""`.
    - `frag_review`: `"checked"`.
    - `frag_type`: copied from the adjacent fragment (default to `Sutta`).
    - `cst_file`, `nikaya`, `group_levels`: copied from the adjacent fragment.
    - `cst_code`, `sc_code`, `cst_vagga`, `cst_sutta`, `cst_paranum`, `sc_sutta`: copied from the adjacent fragment as starting values (user can edit).
    - `start_line`, `start_char`, `end_line`, `end_char`: "zero-width" — set to the same boundary values as the end position of the preceding fragment (or start position of the following fragment), so they don't advance the line/char position. These are NOT literal zeros; they represent a valid position in the file with zero span.
    - `content_html`: empty or null.
22. After insertion, the fragment list in the UI must refresh to show the new fragment in its correct sorted position.

### 4.5 API Endpoint

23. Add a new API endpoint: `POST /api/fragments/insert` accepting:
    - `frag_idx_code` (string): the currently selected fragment's code.
    - `cst_file` (string): the XML file name.
    - `direction` (string): `"before"` or `"after"`.
24. The endpoint returns the newly created fragment record and the updated fragment list for the file.

### 4.6 Regeneration and Override Preservation

The existing `CorrectionFragmentOverrides` pipeline already handles boundary adjustments and metadata restoration for checked/moved fragments. Inserted fragments should be integrated into this same pipeline rather than introducing a completely new regeneration stage.

**How the existing pipeline works (for context):**

- `extract_correction_overrides()` queries reviewed fragments from the reference DB and builds a `HashMap<FragmentKey, CorrectionFragmentOverride>`.
- During parsing, `apply_fragment_adjustment()` is called for each generated fragment. It looks up overrides by `FragmentKey(cst_file, frag_idx)` and adjusts end boundaries or collapses moved fragments. The adjusted end position of one fragment becomes the start position of the next, maintaining continuous boundaries.
- After parsing, `apply_sc_overrides()` restores metadata (sc_code, cst_code, frag_review, etc.) from overrides.

**Adapting the pipeline for inserted fragments:**

25. During `extract_correction_overrides()`, inserted fragments (those with `frag_idx_code` containing a non-zero sub-index, e.g., `"21.1"`) must be extracted alongside checked and moved fragments, preserving their full content and boundary data.
26. Inserted fragments contain XML data and affect line boundaries, just like other `CorrectionFragmentOverrides`. They must be treated as overrides that split a generated fragment's content at a specific boundary point.
27. In the parsing/post-processing pipeline, after the parser produces the base fragment set (with codes `"0.0"`, `"1.0"`, ..., `"N.0"`), the override application logic must:
    - Detect inserted fragment overrides (sub-index > 0) for the current file.
    - For a generated fragment at `"N.0"` that has an inserted fragment `"N.1"` after it: adjust `"N.0"`'s end boundary to the inserted fragment's start boundary (this can be handled by `apply_fragment_adjustment()` using the existing end_line/end_char override mechanism).
    - Inject the inserted fragment at the correct position in the fragment list with its stored content and boundaries.
    - Ensure the next generated fragment `"(N+1).0"` starts at the inserted fragment's end boundary (this is already handled by the existing chaining mechanism where each fragment's start = previous fragment's end).
28. The `apply_sc_overrides()` function must also process inserted fragments to restore their metadata (sc_code, cst_code, frag_review, etc.).
29. During single-file reparse (`reparse_file`), the same preservation logic applies: inserted fragments are carried over from the current DB state via the same override pipeline.

### 4.7 Content Boundary Integrity

30. "Zero-width" for a newly inserted fragment means its start and end boundaries are at the same position (the boundary between the two adjacent fragments), NOT literal zero values. For example, if inserted between a fragment ending at line 45, char 20 and one starting at line 45, char 20, the inserted fragment gets `start_line=45, start_char=20, end_line=45, end_char=20`.
31. When a user adjusts boundaries of an inserted fragment (giving it content from a neighbor), the system must ensure:
    - No line/char range overlap between adjacent fragments.
    - No gaps in coverage — all XML content must be accounted for across the fragment sequence.
    - The concatenation of all fragment `content_xml` values (in `frag_idx_code` order) reproduces the original XML file content.
32. The existing boundary adjustment logic (`adjust-boundary` endpoint) must work correctly with the new `frag_idx_code` field.

## 5. Non-Goals (Out of Scope)

- Automatic detection of where to split fragments — the user decides manually.
- A dedicated "split point" UI dialog — users use existing boundary adjustment buttons.
- Inserting fragments that add new XML content not present in the original file.
- Undo/redo functionality for insertions.
- Batch insertion of multiple fragments at once.

## 6. Design Considerations

- The "Add new before/after" buttons should visually sit alongside the existing "Move to prev/next" buttons in the left panel's middle row.
- Use `A` with up/down arrow (A&uarr; / A&darr;) as button labels, consistent with the existing `M` with arrows for move buttons.
- The confirmation modal text should clearly state what will happen: "Insert a new empty fragment BEFORE/AFTER the current fragment?"
- After insertion, the UI should auto-select the newly inserted fragment so the user can immediately adjust its metadata and boundaries.

## 7. Technical Considerations

### Schema Versioning

- Use Diesel's embedded migrations (`diesel_migrations::embed_migrations!()`) to bundle migrations into the binary.
- On startup or DB connection, call `run_pending_migrations()` to auto-apply any unapplied migrations.
- The `frag_idx` to `frag_idx_code` data conversion should be part of the migration SQL (using `CAST` and string concatenation to produce `"N.0"` values).

### Database Migration

- SQLite does not support `ALTER COLUMN`, so the migration will likely need to create a new table, copy data with transformation, drop old table, and rename.
- All indexes and foreign key relationships involving `frag_idx` must be updated.

### Sorting `frag_idx_code`

- Implement a comparison function that splits the code on `"."` and compares each part numerically: `"2.0" < "2.1" < "10.0"` (not lexicographic).
- This comparator must be used in:
  - SQL `ORDER BY` clauses (may need a custom sort or separate numeric columns for major/minor).
  - Rust code when sorting fragment vectors.
  - JavaScript when displaying the fragment list.

### Override Key Changes

- `FragmentKey` changes from `{ cst_file: String, frag_idx: usize }` to `{ cst_file: String, frag_idx_code: String }`.
- All `HashMap<FragmentKey, ...>` lookups must be updated.

### Regeneration Logic — Integration with Existing Pipeline

The existing regeneration pipeline has three key integration points for inserted fragments:

1. **`extract_correction_overrides()`**: Already queries reviewed fragments. Needs to include inserted fragments (sub-index > 0) with their full content_xml and boundaries, stored in an extended override structure or a parallel data structure alongside `CorrectionFragmentOverrides`.

2. **`apply_fragment_adjustment()`**: Currently adjusts boundaries per-fragment during parsing. For a generated fragment at `"N.0"` that precedes an inserted fragment `"N.1"`, this function should apply the `end_line`/`end_char` override to truncate `"N.0"` at the point where the inserted content begins. This uses the existing override mechanism — the inserted fragment's start boundary becomes the generated fragment's end boundary override.

3. **Post-parsing injection**: After `apply_fragment_adjustment()` handles boundary adjustments during parsing, a post-processing step (which can be integrated into or called alongside `apply_sc_overrides()` in `xml_parser.rs:parse_into_fragments()`) must:
   - Identify inserted fragment overrides for the current file.
   - Insert the stored fragments at the correct positions in the fragment list.
   - The chaining mechanism already ensures the next generated fragment starts at the correct position.

This approach minimizes changes to the existing pipeline — `apply_fragment_adjustment()` handles the boundary splitting (as it already does for checked overrides), and the insertion is a targeted addition to the post-processing phase.

## 8. Success Metrics

- Users can insert a new fragment and adjust its boundaries to correctly split an XML file at an irregular sutta boundary.
- Inserted fragments with `"checked"` status survive full database regeneration and single-file reparse.
- The concatenation of all fragments' `content_xml` for a file reproduces the original XML content exactly.
- All existing tests continue to pass after the `frag_idx` to `frag_idx_code` migration.
- Database schema migrations run automatically on connection with no manual intervention.

## 9. Resolved Questions

1. **Sort performance**: Storing `frag_idx_code` as a single string is sufficient. No need for separate major/minor integer columns.
2. **Maximum sub-index**: No limit enforced. Users can insert as many sub-fragments as needed.
3. **Validation**: No new validation check needed — the existing XML content reconstruction (concatenation of all fragments must reproduce the original file) already guards against gaps and overlaps.
