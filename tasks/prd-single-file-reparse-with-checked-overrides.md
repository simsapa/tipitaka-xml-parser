# PRD: Single File Reparse with Checked Fragment Overrides

## 1. Introduction/Overview

This feature adds the ability to reparse a single XML file while leveraging user-corrected ("checked") fragments from the reference database as parsing overrides. Currently, the only way to re-parse XML files is to regenerate the entire database, which is time-consuming.

The feature addresses two problems:
1. **Slow iteration cycle**: Users must regenerate the entire database to see the effect of corrections on a single file
2. **Lost context during parsing**: When the parser encounters problematic areas (e.g., unrecognized samyutta transitions), it loses tracking of the current sutta group, resulting in null `sc_code` values for subsequent fragments until it naturally recovers

By using checked fragments as parsing overrides (superseding `FragmentAdjustments`), the parser can "pick up" the correct context from user corrections and continue deriving accurate fragment data. This approach also allows fragment boundary corrections to be stored in the database rather than an external TSV file.

## 2. Goals

- **G1**: Allow users to reparse a single XML file without regenerating the entire database
- **G2**: Use checked fragments from the reference database as overrides during parsing to help the parser through problematic areas
- **G3**: Preserve the existing regeneration workflow without breaking changes
- **G4**: Ensure idempotency: reparsing the same file twice with no manual changes should produce identical results
- **G5**: Enable eventual deprecation of `FragmentAdjustments` by including boundary data in `CheckedFragmentOverrides`

## 3. User Stories

### US1: Single File Reparse
As a user reviewing XML fragments, I want to reparse a single XML file so that I can see the effect of my corrections without waiting for a full database regeneration.

### US2: Checked Fragment Context Recovery
As a user who has manually corrected a fragment with the proper `sc_code` and `sc_sutta`, I want the parser to use my correction to correctly identify subsequent fragments in the same sutta group.

### US3: Iterative Correction Workflow
As a user, I want to mark a fragment as "checked", reparse the file, and verify that previously-null fragments now have correct `sc_code` values based on my correction.

### US4: Boundary Corrections in Database
As a user, I want my boundary corrections (end_line, end_char) to be stored in the database as checked fragments rather than in a separate TSV file, so all corrections are managed in one place.

## 4. Functional Requirements

### 4.1 UI: Reparse Button

**FR1.1**: Add a "Reparse" button to each file item in the XML Files list panel
- Button should display only a reload icon (no text)
- Button should have a tooltip: "Reparse this file using current DB as reference"
- Button should be styled consistently with other action buttons in the UI
- Button must be disabled while any regeneration or reparse operation is in progress

**FR1.2**: Clicking the reparse button must show a confirmation dialog
- Use the same modal style as "Regenerate Using Current DB as Reference"
- Dialog text: "Reparse {filename}? This will update fragments using the current database as reference."
- Buttons: "Cancel" and "Reparse"

**FR1.3**: The reparse action must:
1. Extract checked fragment overrides from current database for target file (BEFORE any DB writes)
2. Extract frag_review status for all fragments in target file (for restoration after parsing)
3. Load FragmentAdjustments from embedded TSV (fallback)
4. Parse only the selected XML file using overrides
5. Export parsed fragments to database (`export_fragments_to_db()` handles DELETE + INSERT for that file)
6. Restore frag_review status for fragments that were previously checked
7. Display final output results in the regeneration modal (same behavior as "Regenerate Using Current DB as Reference" - no real-time updates required)
8. Auto-refresh the XML files list and fragments list upon completion

**Note**: This simplified flow eliminates the need for copying the database. Since we extract overrides BEFORE any DB writes, the data is still intact. The `insert_fragments()` function already deletes existing fragments by `cst_file` before inserting new ones (`fragment_exporter.rs:95-100`).

### 4.2 Checked Fragment Overrides

**FR2.1**: Create a new type `CheckedFragmentOverrides` to supersede `FragmentAdjustments`
- Key: `(cst_file, frag_idx)`
- Values must include:
  - `sc_code: Option<String>` - SC reference code
  - `sc_sutta: Option<String>` - SC sutta name
  - `end_line: Option<usize>` - Fragment end line (1-indexed)
  - `end_char: Option<usize>` - Fragment end character (0-indexed)

**FR2.2**: Load checked fragment overrides from reference database before parsing
- Query fragments where `frag_review = 'checked'` for the file(s) being parsed
- Extract `sc_code`, `sc_sutta`, `end_line`, and `end_char` fields
- Create a HashMap keyed by `(cst_file, frag_idx)`

**FR2.3**: Override priority during parsing
- **CheckedFragmentOverrides take precedence over FragmentAdjustments**
- When both exist for the same `(cst_file, frag_idx)`:
  1. First check `CheckedFragmentOverrides` - if found, use it
  2. Only fall back to `FragmentAdjustments` if no checked override exists
- This priority order allows gradual migration from `FragmentAdjustments` to database-stored corrections

**FR2.4**: SC overrides are applied in post-processing, NOT during parsing

SC fields are populated AFTER the parsing loop, not during. The execution order is:
1. Parsing loop → creates fragments with `group_levels`
2. `derive_cst_fields()` → populates `cst_code` from `group_levels`
3. **NEW: Apply SC overrides from checked fragments** (after derive, before TSV)
4. `populate_sc_fields_from_tsv()` → maps `cst_code` to `sc_code` (now conditional, skips fragments with SC already set)

**When applying SC overrides:**
1. Apply SC override directly to the overridden fragment
2. Parse `sc_code` to extract group numbers (e.g., `sn5.1` → samyutta=5, sutta=1)
3. Propagate context to subsequent fragments that have null `sc_code`
4. Continue until hitting a fragment with non-null `sc_code` (natural recovery point)

**Boundary overrides** are still applied during fragment finalization in the parsing loop.

**FR2.5**: Group tracking updates must work for all nikaya types:

| Nikaya | Group Type | Override Effect |
|--------|-----------|-----------------|
| Digha (DN) | Book/Sutta | Extract sutta number from `sc_code` (e.g., `dn1` → sutta 1) |
| Majjhima (MN) | Book/Vagga/Sutta | Extract vagga and sutta from `sc_code` (e.g., `mn1` → sutta 1) |
| Samyutta (SN) | Book/Samyutta/Vagga/Sutta | Extract samyutta from `sc_code` (e.g., `sn5.1` → samyutta 5, sutta 1) |
| Anguttara (AN) | Book/Pannasaka/Vagga/Sutta | Extract nipata from `sc_code` (e.g., `an3.1` → nipata 3, sutta 1) |
| Khuddaka (KN) | Varies by text | Text-specific handling as needed |

**FR2.6**: The override affects both the current fragment AND updates parser state for subsequent fragments
- Boundary overrides (`end_line`, `end_char`) affect only the current fragment
- SC field overrides (`sc_code`, `sc_sutta`) affect parser's group tracking for subsequent fragments

### 4.3 Full Database Regeneration

**FR3.1**: CheckedFragmentOverrides must also be used during full database regeneration with reference
- When "Regenerate Using Current DB as Reference" is triggered:
  1. Copy current database to reference database
  2. Load CheckedFragmentOverrides from reference database for ALL files
  3. Parse all XML files using overrides
  4. Export fragments to new database
  5. Copy reviewed fragments from reference database

**FR3.2**: This ensures consistent behavior between single-file reparse and full regeneration

### 4.4 Interaction with `copy_reviewed_fragments_from_reference()`

**FR4.1**: The execution order for single-file reparse:
1. Extract CheckedFragmentOverrides from current database for target file
2. Extract frag_review status map (`frag_idx` → `frag_review`) for target file
3. Load FragmentAdjustments from embedded TSV (fallback only)
4. Parse XML file using boundary overrides
5. In post-processing: `derive_cst_fields()` → apply SC overrides → `populate_sc_fields_from_tsv_conditional()`
6. Export parsed fragments to database (DELETE + INSERT for that file)
7. Restore frag_review status for previously checked fragments

**FR4.2**: frag_review status restoration:
- Before parsing, extract `frag_idx → frag_review` mapping for all checked fragments
- After `export_fragments_to_db()` inserts new fragments, update their `frag_review` field
- This preserves the review status without needing a reference database copy

**FR4.3**: How checked overrides help subsequent fragments:
- The checked fragment's `sc_code` (e.g., `sn5.1`) is used to derive context
- This context (samyutta=5) is propagated to subsequent null fragments
- The checked fragment itself retains its override values AND its frag_review status
- Subsequent fragments get derived `sc_code` values (e.g., `sn5.2`, `sn5.3`) based on their `cst_code`

### 4.5 Backend API

**FR5.1**: Add new endpoint `POST /api/reparse-file`
- Request body: `{ "cst_file": "s0301m.mul.xml" }`
- Response: Same structure as regeneration response

**FR5.2**: The endpoint must:
1. Validate that the file exists in the current database
2. Extract `CheckedFragmentOverrides` from current database for this file
3. Extract `frag_review` status map for this file (for restoration after parsing)
4. Load `FragmentAdjustments` from embedded TSV (fallback)
5. Parse the single XML file with `ParserOverrides` (boundary overrides during parsing)
6. In post-processing: apply SC overrides, then conditional TSV population
7. Export fragments to current database (DELETE + INSERT)
8. Restore `frag_review` status for previously checked fragments
9. Return success/error status with output messages

### 4.6 Deprecation Path for FragmentAdjustments

**FR6.1**: Add migration notes for existing `adjust-fragments.tsv` entries:
- Each entry in `adjust-fragments.tsv` should eventually be converted to a checked fragment in the database
- Once all entries are migrated, `FragmentAdjustments` loading can be removed

**FR6.2**: During the transition period:
- Both systems coexist
- CheckedFragmentOverrides always take precedence
- FragmentAdjustments serve as fallback for fragments not yet migrated to database

**FR6.3**: After this feature is completed:
- Migrate all existing `adjust-fragments.tsv` entries to checked fragments in the database
- Remove `FragmentAdjustments` loading code and the `adjust-fragments.tsv` file
- Update documentation to reflect the new workflow

## 5. Non-Goals (Out of Scope)

- **NG1**: Bulk reparse of multiple selected files (single file only)
- **NG2**: Automatic migration tool from `adjust-fragments.tsv` to database
- **NG3**: UI for managing or viewing checked fragment overrides
- **NG4**: Immediate removal of `FragmentAdjustments` (deprecation is gradual)

## 6. Design Considerations

### Key Finding: Simplified Approach is Feasible

**Current full regeneration flow:**
1. Copy current DB → reference DB (expensive)
2. Parse all XML files → new DB
3. Copy reviewed fragments from reference → new DB

**Simplified single-file flow (no DB copy needed):**
1. Extract CheckedFragmentOverrides from current DB for target file
2. Extract frag_review status (to restore after parsing)
3. Parse single XML file using overrides
4. `export_fragments_to_db()` handles DELETE + INSERT for that file
5. Restore frag_review status for previously checked fragments

**Why this works:**
- `insert_fragments()` already deletes existing fragments by `cst_file` before inserting (`fragment_exporter.rs:95-100`)
- We extract overrides BEFORE any DB writes, so data is still intact
- Single-file scope means other files are untouched
- No concurrent writes during operation (modal blocks UI)

### UI Mockup

```
XML Files
─────────────────────────────────
Saṃyutta Nikāya
  s0301m.mul.xml (1234)  [↻]   ← Reparse button (icon only)
  s0302m.mul.xml (567)   [↻]
  s0301a.att.xml (890)   [↻]
```

- Button: Small, icon-only, positioned at the right of each file row
- Icon: Reload/refresh icon (e.g., `fa-sync` or similar)
- Hover: Tooltip appears with explanatory text

### Confirmation Modal

Reuse the existing regeneration modal structure:
- Same styling and layout
- Title: "Reparse File"
- Final output results displayed in the output area (same as full regeneration)

## 7. Technical Considerations

### 7.1 Data Structures

```rust
/// Checked fragment override data extracted from current database
/// Supersedes FragmentAdjustments with additional SC field support
#[derive(Debug, Clone)]
pub struct CheckedFragmentOverride {
    // Boundary overrides (same as FragmentAdjustments)
    pub end_line: Option<usize>,   // 1-indexed
    pub end_char: Option<usize>,   // 0-indexed

    // SC field overrides (new functionality)
    pub sc_code: Option<String>,
    pub sc_sutta: Option<String>,
}

pub type CheckedFragmentOverrides = HashMap<FragmentKey, CheckedFragmentOverride>;

/// Combined override configuration for parsing
/// Simplifies function signatures by bundling all override types
#[derive(Debug, Clone, Default)]
pub struct ParserOverrides {
    pub adjustments: Option<FragmentAdjustments>,
    pub checked_overrides: Option<CheckedFragmentOverrides>,
}

/// Parsed components from an SC code for context propagation
#[derive(Debug, Clone, Default)]
pub struct ScCodeComponents {
    pub prefix: String,          // e.g., "sn", "an", "dn", "mn"
    pub samyutta: Option<i32>,   // SN: samyutta number (e.g., sn5.1 → 5)
    pub nipata: Option<i32>,     // AN: book number (e.g., an3.1 → 3)
    pub sutta: Option<i32>,      // Sutta number (last number in code)
}
```

### 7.2 Parser Integration

**Key Insight**: SC fields are populated in post-processing, NOT during the parsing loop.

#### 7.2.1 Current Flow (before this feature)

```rust
// 1. Parsing loop creates fragments with group_levels
let fragments = parse_xml_content(...)?;

// 2. Post-processing: derive CST fields
for fragment in &mut fragments {
    let (cst_file, cst_code, ...) = derive_cst_fields(fragment, nikaya_structure);
    fragment.cst_code = cst_code;
    // ...
}

// 3. Populate SC fields from TSV mapping
if populate_sc_fields {
    populate_sc_fields_from_tsv(&mut fragments)?;
}
```

#### 7.2.2 New Flow (with SC overrides)

```rust
// 1. Parsing loop creates fragments (unchanged, but uses boundary overrides)
let fragments = parse_xml_content(..., &parser_overrides)?;

// 2. Post-processing: derive CST fields (unchanged)
for fragment in &mut fragments {
    let (cst_file, cst_code, ...) = derive_cst_fields(fragment, nikaya_structure);
    fragment.cst_code = cst_code;
    // ...
}

// 3. NEW: Apply SC overrides from checked fragments
if let Some(ref overrides) = checked_overrides {
    apply_sc_overrides(&mut fragments, overrides, &nikaya_structure.nikaya);
}

// 4. Populate SC fields CONDITIONALLY (skip fragments with SC already set)
if populate_sc_fields {
    populate_sc_fields_from_tsv_conditional(&mut fragments)?;
}
```

#### 7.2.3 SC Code Parsing by Nikaya

| Nikaya | Example Code | Extracted Components |
|--------|--------------|---------------------|
| DN | `dn1` | sutta=1 |
| MN | `mn41` | sutta=41 |
| SN | `sn5.1` | samyutta=5, sutta=1 |
| AN | `an3.1` | nipata=3, sutta=1 |

#### 7.2.4 Context Propagation Algorithm

```
apply_sc_overrides(fragments, overrides, nikaya):
    for each override in overrides:
        fragment = find fragment by frag_idx
        fragment.sc_code = override.sc_code
        fragment.sc_sutta = override.sc_sutta

        # Parse override to extract context
        context = parse_sc_code(override.sc_code, nikaya)

        # Propagate to subsequent fragments with null sc_code
        for subsequent in fragments[frag_idx+1..]:
            if subsequent.sc_code is not null:
                break  # Natural recovery point

            # Derive sc_code from cst_code using propagated context
            if subsequent.cst_code is not null:
                subsequent.sc_code = derive_sc_code(subsequent.cst_code, context)
```

### 7.3 Helper Functions

#### 7.3.1 Boundary Override Helper (for parsing loop)

```rust
/// Get boundary overrides for a fragment (checked takes precedence over adjustments)
/// Returns (end_line, end_char) override if available
pub fn get_fragment_overrides(
    cst_file: &str,
    frag_idx: usize,
    checked_overrides: Option<&CheckedFragmentOverrides>,
    adjustments: Option<&FragmentAdjustments>,
) -> (Option<(usize, usize)>, Option<(String, Option<String>)>)
// Returns: (boundary_override, sc_override)
```

#### 7.3.2 SC Code Parser

```rust
/// Parse SC code into components based on nikaya
pub fn parse_sc_code(sc_code: &str, nikaya: &str) -> Option<ScCodeComponents>
// Examples:
// - parse_sc_code("sn5.1", "samyutta") → ScCodeComponents { prefix: "sn", samyutta: Some(5), sutta: Some(1), .. }
// - parse_sc_code("an3.1", "anguttara") → ScCodeComponents { prefix: "an", nipata: Some(3), sutta: Some(1), .. }
// - parse_sc_code("dn1", "digha") → ScCodeComponents { prefix: "dn", sutta: Some(1), .. }
// - parse_sc_code("mn41", "majjhima") → ScCodeComponents { prefix: "mn", sutta: Some(41), .. }
```

#### 7.3.3 SC Override Application

```rust
/// Apply SC overrides and propagate context to subsequent fragments
pub fn apply_sc_overrides(
    fragments: &mut Vec<XmlFragment>,
    checked_overrides: &CheckedFragmentOverrides,
    nikaya: &str,
)
```

#### 7.3.4 Conditional TSV Population

```rust
/// Populate SC fields from TSV only for fragments with null sc_code
/// Skips fragments that already have sc_code set (from overrides)
pub fn populate_sc_fields_from_tsv_conditional(
    fragments: &mut Vec<XmlFragment>,
) -> Result<()>
```

#### 7.3.5 Database Extraction Functions (in fragment_exporter.rs)

```rust
/// Extract checked overrides from current DB
/// Returns: (overrides for parsing, frag_idx → frag_review for restoration)
pub fn extract_checked_overrides(
    db_path: &Path,
    cst_file: &str,
) -> Result<(CheckedFragmentOverrides, HashMap<usize, String>)>

/// Restore frag_review status after parsing
/// Returns: number of fragments updated
pub fn restore_frag_review_status(
    db_path: &Path,
    cst_file: &str,
    review_status: &HashMap<usize, String>,
) -> Result<usize>
```

**SQL query for extraction:**
```sql
SELECT frag_idx, end_line, end_char, sc_code, sc_sutta, frag_review
FROM xml_fragments
WHERE cst_file = ? AND frag_review NOT IN (NULL, '', 'unchecked')
```

### 7.4 Files to Modify

| File | Changes |
|------|---------|
| `src/types.rs` | Add `CheckedFragmentOverride`, `CheckedFragmentOverrides`, `ParserOverrides`, `ScCodeComponents` |
| `src/fragment_exporter.rs` | Add `extract_checked_overrides()`, `restore_frag_review_status()` |
| `src/parsers/helpers.rs` | Add `get_fragment_overrides()`, `parse_sc_code()`, `apply_sc_overrides()`, `populate_sc_fields_from_tsv_conditional()` |
| `src/parsers/samyutta_nikaya_mula.rs` | Update post-processing to apply SC overrides |
| `src/parsers/general.rs` | Update to use `ParserOverrides` |
| `src/xml_parser.rs` | Update dispatcher signatures |
| `src/integration.rs` | Update `TipitakaImporter` to accept/pass `ParserOverrides` |
| `src/main.rs` | Load checked overrides from reference DB for full regeneration, update CLI |
| `src/web/routes.rs` | Add `/api/reparse-file` endpoint |
| `src/static/index.html` | Add reparse button to file list |
| `src/static/scripts/app.js` | Add reparse handler and API call |

### 7.5 Existing Code to Reuse

- `src/main.rs:267-273`: Query pattern for fetching reviewed fragments
- `src/parsers/helpers.rs:228-258`: `apply_fragment_adjustment()` - pattern for boundary override
- `src/parsers/helpers.rs:269-285`: `populate_sc_fields_from_tsv()` - modify to be conditional
- `src/fragment_exporter.rs:95-100`: DELETE by cst_file (already handles single-file scope)
- `src/web/routes.rs:776-1056`: `regenerate()` endpoint - reference for API structure

### 7.6 Test Case: s0301m.mul.xml

The specific test case described:
- `frag_idx 161`: Correctly parsed as `cst_code = sn1.4.3.5`, `sc_code = sn4.25`
- `frag_idx 162`: Samyutta transition to `cst_code = sn1.5.0.1`, but NOT recognized as `sc_code = sn5.1`
- `frag_idx 162-171`: `sc_code` is null
- `frag_idx 172`: Parser recovers with `cst_code = sn1.6.1.1`, `sc_code = sn6.1`

After manually setting `frag_idx 162` to `sc_code = sn5.1`, `sc_sutta = Āḷavikāsutta` and marking as "checked":
- Reparse should use the checked data to update samyutta tracking
- `frag_idx 163-171` should now correctly derive `sc_code = sn5.x`

### 7.7 Verification Steps

1. **Build:** `cargo build` - verify no compilation errors
2. **Unit tests:** `cargo test` - verify all tests pass
3. **Manual test - single file reparse:**
   - Start web server, open UI
   - Parse s0301m.mul.xml, verify fragments 162-171 have null sc_code
   - Edit fragment 162: set sc_code=sn5.1, sc_sutta=Āḷavikāsutta, frag_review=checked
   - Click reparse button for s0301m.mul.xml
   - Verify fragments 163-171 now have correct sc_code=sn5.x values
4. **Idempotency test:**
   - Reparse same file twice with no manual changes
   - Export to TSV before and after, diff should show zero changes
5. **Full regeneration test:**
   - Regenerate entire database with reference
   - Verify checked fragments are properly used during parsing

## 8. Success Metrics

- **SM1**: Manual testing confirms reparse button appears and functions correctly
- **SM2**: Test case s0301m.mul.xml produces correct `sc_code` values for fragments 162-171 after reparse
- **SM3**: Reparsing twice with no manual changes produces identical fragment data
- **SM4**: Full regeneration twice produces identical databases
- **SM5**: Existing `adjust-fragments.tsv` entries continue to work during transition period

## 9. Test Plan

### Test 1: Checked Override Fixes Null Fragments

**Setup**:
1. Parse s0301m.mul.xml to database
2. Verify fragments 162-171 have null `sc_code`
3. Manually set fragment 162: `sc_code = sn5.1`, `sc_sutta = Āḷavikāsutta`, `frag_review = 'checked'`

**Action**: Trigger single-file reparse for s0301m.mul.xml

**Expected Result**:
- Fragment 162: retains `sc_code = sn5.1` (from checked override, then copied from reference)
- Fragments 163-171: have correct `sc_code = sn5.x` values (derived by parser using updated context)
- Fragment 172+: continue with correct values

### Test 2: Idempotent Single-File Reparse

**Setup**:
1. Database with checked fragments for s0301m.mul.xml
2. Perform first reparse

**Action**: Perform second reparse immediately (no manual changes)

**Expected Result**:
- Export fragment data to TSV before and after second reparse
- TSV diff shows zero changes

### Test 3: Idempotent Full Regeneration

**Setup**:
1. Complete database with all checked fragments
2. Perform first full regeneration with reference

**Action**: Perform second full regeneration immediately (no manual changes)

**Expected Result**:
- Export relevant fields to TSV before and after second regeneration
- TSV diff shows zero changes

### Test 4: CheckedFragmentOverrides Precedence

**Setup**:
1. Add entry to `adjust-fragments.tsv` for a specific `(cst_file, frag_idx)`
2. Also add checked fragment in database for same `(cst_file, frag_idx)` with different values

**Action**: Parse the file

**Expected Result**:
- The checked fragment override values are used, not the TSV adjustment values
- Confirms precedence order is correct

### Test 5: Boundary Override from Checked Fragment

**Setup**:
1. Create checked fragment with `end_line` and `end_char` values that differ from auto-detected
2. Mark as `frag_review = 'checked'`

**Action**: Reparse the file

**Expected Result**:
- Fragment boundaries match the checked override values
- Content is correctly extracted based on overridden boundaries

### Test Data Location

- Test fixtures: `tests/data/`
- Expected outputs: `tests/data/expected/`
- Test input data (reference databases, correction data): `tests/data/fixtures/`

## 10. Open Questions

All questions have been resolved:

- ~~**OQ1**: Should the reparse button be disabled while a regeneration is in progress?~~ **Yes** - Added to FR1.1
- ~~**OQ2**: Should there be visual feedback (e.g., spinner) on the specific file row during reparse?~~ **No** - Modal stays open, user waits
- ~~**OQ3**: Should the file list auto-refresh after reparse completes, or require manual refresh?~~ **Auto-refresh** - Added to FR1.3
- ~~**OQ4**: For nikaya parsers other than Samyutta (DN, MN, AN), are there similar problematic transitions that would benefit from checked overrides?~~ Addressed in FR2.5
- ~~**OQ5**: Should there be a UI indicator showing which files have checked fragments that will be used as overrides?~~ **No** - Not needed
- ~~**OQ6**: What is the timeline for fully deprecating `adjust-fragments.tsv`?~~ **After this feature is completed** - Added to FR6.3
