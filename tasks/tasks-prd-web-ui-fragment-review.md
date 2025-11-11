# Tasks: Web UI for Fragment Review and Correction

## Relevant Files

- `Cargo.toml` - Add Rocket web framework dependencies
- `src/main.rs` - Add new `web-ui` CLI command handler
- `src/lib.rs` - Export web module
- `src/web/mod.rs` - Web module declaration and initialization
- `src/web/routes.rs` - Rocket API endpoint handlers
- `src/web/models.rs` - Web-specific DTOs and response models
- `src/web/state.rs` - Rocket state management for database connection
- `src/static/index.html` - Main HTML page with Bulma CSS layout
- `src/static/styles/main.css` - Custom CSS for panel resizing and tweaks
- `src/static/scripts/app.js` - Frontend JavaScript for UI interactions
- `src/fragments_models.rs` - May need additional models for updating fragments
- `src/fragments_schema.rs` - Already defined, will be reused

### Notes

- Use `cargo build` to compile the project
- Use `cargo test` to run all tests
- Use `cargo run -- web-ui <db_path>` to start the web server after implementation
- Frontend uses vanilla JavaScript with Fetch API for simplicity

## Tasks

- [ ] 1.0 Stage 1: Basic Structure and Layout with Mock Data
  - [ ] 1.1 Add Rocket web framework dependencies to `Cargo.toml` (rocket, rocket_dyn_templates if needed)
  - [ ] 1.2 Create `src/web/` module structure with `mod.rs`, `routes.rs`, `models.rs`, `state.rs`
  - [ ] 1.3 Add `WebUi` CLI command in `src/main.rs` with port flag (default 8000)
  - [ ] 1.4 Implement basic Rocket server setup in `src/web/mod.rs` that serves static files
  - [ ] 1.5 Create `src/static/` directory structure with subdirectories for `styles/` and `scripts/`
  - [ ] 1.6 Create `src/static/index.html` with Bulma CSS CDN link and two-panel layout structure
  - [ ] 1.7 Implement left panel structure: file list (top), fragment list (middle), metadata form (bottom)
  - [ ] 1.8 Implement right panel structure: three text areas for previous/current/next fragments
  - [ ] 1.9 Add boundary control button rows between text areas (4 buttons each)
  - [ ] 1.10 Add delete fragment buttons above/below text areas
  - [ ] 1.11 Create `src/static/scripts/app.js` with mock data structure (2-3 files, 5-10 fragments each)
  - [ ] 1.12 Implement JavaScript to populate file list from mock data with click handlers
  - [ ] 1.13 Implement JavaScript to populate fragment list when file is selected
  - [ ] 1.14 Implement JavaScript to update metadata fields and text areas when fragment is selected
  - [ ] 1.15 Create `src/static/styles/main.css` for custom styling and draggable panel separator
  - [ ] 1.16 Implement draggable panel separator functionality in JavaScript
  - [ ] 1.17 Test that all UI elements are functional with mock data
  - [ ] 1.18 Verify compilation with `cargo build` and server starts on correct port

- [ ] 2.0 Stage 2: Database Integration with Real Fragment Data
  - [ ] 2.1 Update `WebUi` CLI command to accept required `<db_path>` argument
  - [ ] 2.2 Create database connection pool in `src/web/state.rs` using Diesel
  - [ ] 2.3 Add `DbState` struct to Rocket managed state for sharing connection pool
  - [ ] 2.4 Create web-specific DTOs in `src/web/models.rs` (FileListItem, FragmentListItem, FragmentDetail)
  - [ ] 2.5 Implement `GET /api/files` endpoint in `src/web/routes.rs` to return distinct `cst_file` values
  - [ ] 2.6 Implement `GET /api/files/:filename/fragments` endpoint to return fragments for a file ordered by `frag_idx`
  - [ ] 2.7 Implement `GET /api/fragments/:id` endpoint to return fragment details with adjacent fragments
  - [ ] 2.8 Add logic to handle edge cases (first/last fragment in file) in fragment detail endpoint
  - [ ] 2.9 Update `app.js` to call `GET /api/files` and populate file list with real data
  - [ ] 2.10 Update `app.js` to call `GET /api/files/:filename/fragments` when file is selected
  - [ ] 2.11 Add visual indicators (Bulma tags with colors) for `frag_review` status in fragment list
  - [ ] 2.12 Update `app.js` to call `GET /api/fragments/:id` when fragment is selected
  - [ ] 2.13 Populate metadata fields with actual database values from selected fragment
  - [ ] 2.14 Populate text areas with `content_xml` for previous/current/next fragments
  - [ ] 2.15 Implement disable/hide logic for previous controls when fragment is first in file
  - [ ] 2.16 Implement disable/hide logic for next controls when fragment is last in file
  - [ ] 2.17 Test with actual fragments database to verify all data displays correctly
  - [ ] 2.18 Verify edge cases work correctly (first/last fragments)

- [ ] 3.0 Stage 3: Full Persistence and Boundary Adjustment Functionality
  - [ ] 3.1 Add Diesel update models to `src/fragments_models.rs` for fragment metadata updates
  - [ ] 3.2 Implement `PATCH /api/fragments/:id` endpoint for updating metadata fields
  - [ ] 3.3 Add frontend blur event handlers on metadata input fields to auto-save changes
  - [ ] 3.4 Convert `frag_review` field to dropdown with options: unchecked, in-progress, checked, needs-review
  - [ ] 3.5 Create boundary adjustment request/response models in `src/web/models.rs`
  - [ ] 3.6 Implement `POST /api/fragments/:id/adjust-boundary` endpoint with transaction handling
  - [ ] 3.7 Add logic for "Line Up" button: moves content from current to previous (decrements `start_line`)
  - [ ] 3.8 Add logic for "Line Down" button: moves content from previous to current (increments `start_line`)
  - [ ] 3.9 Add logic for "Char Left" button: moves content from current to previous (decrements `start_char`)
  - [ ] 3.10 Add logic for "Char Right" button: moves content from previous to current (increments `start_char`)
  - [ ] 3.11 Implement boundary recalculation for affected fragments (zero-sum behavior)
  - [ ] 3.12 Add empty fragment detection logic in boundary adjustment
  - [ ] 3.13 Create Bulma modal component in `index.html` for confirmation dialogs
  - [ ] 3.14 Implement modal confirmation for empty fragment deletion
  - [ ] 3.15 Implement fragment deletion logic on confirmation (update adjacent fragment boundaries)
  - [ ] 3.16 Implement `DELETE /api/fragments/:id` endpoint with transaction handling
  - [ ] 3.17 Add logic to recalculate `frag_idx` for subsequent fragments after deletion
  - [ ] 3.18 Add "Delete Previous Fragment" button handler with confirmation modal
  - [ ] 3.19 Add "Delete Next Fragment" button handler with confirmation modal
  - [ ] 3.20 Implement frontend refresh logic after boundary adjustments (reload fragment data)
  - [ ] 3.21 Implement frontend refresh logic after deletions (refresh list and re-select)
  - [ ] 3.22 Add error handling and user-friendly error messages for failed operations
  - [ ] 3.23 Test boundary adjustments with various scenarios (line/char movements)
  - [ ] 3.24 Test fragment deletion workflow with confirmations
  - [ ] 3.25 Verify data integrity after operations (no gaps/overlaps in fragments)
  - [ ] 3.26 Test complete workflow: review, edit, adjust boundaries, delete fragments
