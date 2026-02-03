# Tasks: ArangoDB Integration and Database Validation

## Relevant Files

- `src/web/arangodb.rs` - New module for ArangoDB connection management, status checking, and Pali title fetching
- `src/web/validation.rs` - New module for validation check definitions, registry, and handlers
- `src/web/mod.rs` - Update to include new arangodb and validation modules
- `src/web/routes.rs` - Add new API endpoint handlers for ArangoDB and validation
- `src/web/models.rs` - Add DTOs for validation results, auto-fix requests, and ArangoDB responses
- `src/static/index.html` - Add status indicator, "Database Validation" menu item, and validation modal HTML
- `src/static/scripts/app.js` - Add ArangoDB status polling, title caching, and validation modal logic
- `src/static/styles/main.css` - Add styles for status indicator and validation modal layout

### Notes

- The project uses `arangors = "0.6.0"` crate which is already in Cargo.toml
- The `sc_sutta` column already exists in `xml_fragments` table - no database migration needed
- Follow existing modal patterns from Settings and Regenerate modals in `index.html`
- Use `cargo test` to run tests; use `cargo build` to verify compilation after each task

## Tasks

- [x] 1.0 Create ArangoDB connection module
  - [x] 1.1 Create `src/web/arangodb.rs` file with module documentation
  - [x] 1.2 Implement `connect_to_arangodb()` function that establishes connection using hardcoded credentials (localhost:8529, root/test, suttacentral)
  - [x] 1.3 Implement `check_connection_status()` function that returns a boolean indicating if ArangoDB is reachable
  - [x] 1.4 Implement `get_pali_titles()` function that queries the `names` collection with `FOR x IN names FILTER x.is_root == true RETURN x` and returns a HashMap<String, String> mapping uid to name
  - [x] 1.5 Add the `arangodb` module to `src/web/mod.rs`
  - [x] 1.6 Verify compilation with `cargo build`

- [x] 2.0 Add ArangoDB API endpoints
  - [x] 2.1 Add `ArangoStatusResponse` struct to `src/web/models.rs` with fields: `connected: bool`, `error: Option<String>`
  - [x] 2.2 Add `PaliTitlesResponse` type alias (HashMap<String, String>) to `src/web/models.rs`
  - [x] 2.3 Implement `GET /api/arangodb/status` handler in `src/web/routes.rs` that calls `check_connection_status()` and returns JSON response
  - [x] 2.4 Implement `GET /api/arangodb/pali-titles` handler in `src/web/routes.rs` that calls `get_pali_titles()` and returns the uid→name mapping
  - [x] 2.5 Register new routes in `get_routes()` function
  - [x] 2.6 Verify endpoints work with `cargo run` and curl/browser testing

- [x] 3.0 Implement frontend ArangoDB status indicator
  - [x] 3.1 Add status indicator HTML element next to Menu button in `index.html` (small circle span with id `arango-status`)
  - [x] 3.2 Add CSS styles for status indicator in `main.css` (green/red colors, 12-16px circle, tooltip styling)
  - [x] 3.3 Add `window.paliTitlesCache = null` global variable in `app.js`
  - [x] 3.4 Add `window.arangoConnected = false` global variable to track connection state
  - [x] 3.5 Implement `checkArangoStatus()` async function that calls `/api/arangodb/status`, updates indicator color and tooltip
  - [x] 3.6 Implement `fetchPaliTitles()` async function that calls `/api/arangodb/pali-titles` and populates `window.paliTitlesCache`
  - [x] 3.7 Add logic to `checkArangoStatus()`: if connection restored (was false, now true) and cache is empty, call `fetchPaliTitles()`
  - [x] 3.8 Call `checkArangoStatus()` on init and set up 5-second interval with `setInterval()`
  - [x] 3.9 Test status indicator shows green when ArangoDB running, red with tooltip when not

- [x] 4.0 Create validation module with check registry
  - [x] 4.1 Create `src/web/validation.rs` file with module documentation
  - [x] 4.2 Define `ValidationError` struct with fields: `cst_file`, `frag_idx`, `fragment_id`, `message`
  - [x] 4.3 Define `AutoFix` struct with fields: `fragment_id`, `sc_code`, `suggested_value`
  - [x] 4.4 Define `ValidationCheckResult` struct with fields: `name`, `description`, `auto_fixable`, `errors: Vec<ValidationError>`, `auto_fixes: Vec<AutoFix>`
  - [x] 4.5 Implement `check_missing_sc_code()` function that queries fragments where `frag_type = 'Sutta'` AND `frag_review != 'moved'` AND (`sc_code IS NULL` OR `sc_code = ''`)
  - [x] 4.6 Implement `check_missing_sc_sutta()` function that queries fragments where `sc_code IS NOT NULL` AND `sc_code != ''` AND (`sc_sutta IS NULL` OR `sc_sutta = ''`), and populates auto_fixes using provided titles HashMap
  - [x] 4.7 Implement `run_all_validations()` function that runs all checks and returns HashMap<String, ValidationCheckResult>
  - [x] 4.8 Add the `validation` module to `src/web/mod.rs`
  - [x] 4.9 Verify compilation with `cargo build`

- [x] 5.0 Add validation API endpoints
  - [x] 5.1 Add validation DTOs to `src/web/models.rs`: `ValidationRunResponse`, `AutoFixRequest`, `AutoFixResponse`
  - [x] 5.2 Implement `POST /api/validation/run` handler that calls `run_all_validations()` with Pali titles from ArangoDB (if connected)
  - [x] 5.3 Implement `POST /api/validation/auto-fix/missing-sc-sutta` handler that accepts array of AutoFix items and updates `sc_sutta` field for each fragment
  - [x] 5.4 Register new validation routes in `get_routes()` function
  - [x] 5.5 Verify endpoints work with `cargo run` and curl testing

- [x] 6.0 Implement validation modal UI
  - [x] 6.1 Add "Database Validation" menu item to the Menu dropdown in `index.html`
  - [x] 6.2 Create validation modal HTML structure with: header, "Run Validation" button, split layout (left: check types list, right: results panel)
  - [x] 6.3 Add check type list items with badge placeholders for error counts
  - [x] 6.4 Add results panel with scrollable list container and "Auto-Fix All" button (initially hidden)
  - [x] 6.5 Add auto-fix confirmation modal HTML with scrollable changes list and "Apply Changes"/"Cancel" buttons
  - [x] 6.6 Add CSS styles for validation modal layout (split columns, scrollable lists, badges) in `main.css`

- [x] 7.0 Implement validation modal JavaScript logic
  - [x] 7.1 Add `validationResults = null` state variable and `selectedCheckType = null` variable
  - [x] 7.2 Implement `openValidationModal()` function that resets modal state and shows modal
  - [x] 7.3 Implement `closeValidationModal()` function
  - [x] 7.4 Implement `runValidation()` async function: show loading spinner on button, call `/api/validation/run`, store results, update check type badges, select first check type
  - [x] 7.5 Implement `selectCheckType(checkId)` function that highlights selected check, displays errors in results panel, shows/hides "Auto-Fix All" button based on auto_fixes array
  - [x] 7.6 Implement `renderValidationResults(checkId)` function that creates result items with cst_file, frag_idx, message, and "Open" button
  - [x] 7.7 Implement `openFragmentFromValidation(cstFile, fragIdx)` function that closes modal, selects file, finds fragment by frag_idx, and selects it
  - [x] 7.8 Implement `showAutoFixConfirmation()` function that displays scrollable list of changes (cst_file | frag_idx | sc_code → suggested_value)
  - [x] 7.9 Implement `applyAutoFixes()` async function that submits auto_fixes to `/api/validation/auto-fix/missing-sc-sutta`, then re-runs validation
  - [x] 7.10 Add event listeners for: menu item click, Run Validation button, check type selection, Open buttons, Auto-Fix All button, confirmation dialog buttons
  - [x] 7.11 Wire up modal close buttons and background click handlers

- [x] 8.0 Integration testing and refinement
  - [x] 8.1 Test ArangoDB status indicator with ArangoDB running and stopped
  - [x] 8.2 Test Pali titles cache population on startup and on connection restore
  - [x] 8.3 Test validation modal opens from menu, Run Validation shows spinner, results display correctly
  - [x] 8.4 Test clicking "Open" button navigates to correct fragment
  - [x] 8.5 Test auto-fix flow: confirmation dialog shows changes, Apply Changes updates database, results refresh
  - [x] 8.6 Test edge cases: no errors found, ArangoDB disconnected during validation, empty auto_fixes array
  - [x] 8.7 Verify all functionality works together in a complete workflow
  - [x] 8.8 Run `cargo build --release` to ensure release build succeeds
