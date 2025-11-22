# Tasks: Web UI for Fragment Review and Correction

## Relevant Files

- `Cargo.toml` - Added Rocket web framework dependencies (rocket, rocket_dyn_templates, tokio, toml)
- `src/main.rs` - Added `WebUi` CLI command handler with port flag
- `src/lib.rs` - Exported web module
- `src/web/mod.rs` - Web module with server initialization and static file serving
- `src/web/routes.rs` - Rocket route handlers for API endpoints (files, fragments, settings)
- `src/web/models.rs` - Web-specific DTOs including AppSettings for config
- `src/web/state.rs` - Rocket state management for database path
- `src/web/settings.rs` - TOML config file management for application settings
- `src/static/index.html` - Main HTML page with Bulma CSS layout, two-panel structure, and modals
- `src/static/styles/main.css` - Custom CSS for panel resizing, scrollbars, modals, and UI tweaks
- `src/static/scripts/app.js` - Frontend JavaScript with API calls, state persistence, and UI interactions
- `src/fragments_models.rs` - Diesel models for database operations
- `src/fragments_schema.rs` - Database schema definitions
- `.gitignore` - Added web-ui-config.toml to ignore list

### Notes

- Use `cargo build` to compile the project
- Use `cargo test` to run all tests
- Use `cargo run -- web-ui <db_path>` to start the web server after implementation
- Frontend uses vanilla JavaScript with Fetch API for simplicity

## Tasks

- [x] 1.0 Stage 1: Basic Structure and Layout with Mock Data
  - [x] 1.1 Add Rocket web framework dependencies to `Cargo.toml` (rocket, rocket_dyn_templates if needed)
  - [x] 1.2 Create `src/web/` module structure with `mod.rs`, `routes.rs`, `models.rs`, `state.rs`
  - [x] 1.3 Add `WebUi` CLI command in `src/main.rs` with port flag (default 8000)
  - [x] 1.4 Implement basic Rocket server setup in `src/web/mod.rs` that serves static files
  - [x] 1.5 Create `src/static/` directory structure with subdirectories for `styles/` and `scripts/`
  - [x] 1.6 Create `src/static/index.html` with Bulma CSS CDN link and two-panel layout structure
  - [x] 1.7 Implement left panel structure: file list (top), fragment list (middle), metadata form (bottom)
  - [x] 1.8 Implement right panel structure: three text areas for previous/current/next fragments
  - [x] 1.9 Add boundary control button rows between text areas (4 buttons each)
  - [x] 1.10 Add delete fragment buttons above/below text areas
  - [x] 1.11 Create `src/static/scripts/app.js` with mock data structure (2-3 files, 5-10 fragments each)
  - [x] 1.12 Implement JavaScript to populate file list from mock data with click handlers
  - [x] 1.13 Implement JavaScript to populate fragment list when file is selected
  - [x] 1.14 Implement JavaScript to update metadata fields and text areas when fragment is selected
  - [x] 1.15 Create `src/static/styles/main.css` for custom styling and draggable panel separator
  - [x] 1.16 Implement draggable panel separator functionality in JavaScript
  - [x] 1.17 Test that all UI elements are functional with mock data
  - [x] 1.18 Verify compilation with `cargo build` and server starts on correct port

- [x] 2.0 Stage 2: Database Integration with Real Fragment Data
  - [ ] 2.1 Update `WebUi` CLI command to accept required `<db_path>` argument
  - [x] 2.2 Create database connection pool in `src/web/state.rs` using Diesel
  - [x] 2.3 Add `DbState` struct to Rocket managed state for sharing connection pool
  - [x] 2.4 Create web-specific DTOs in `src/web/models.rs` (FileListItem, FragmentListItem, FragmentDetail)
  - [x] 2.5 Implement `GET /api/files` endpoint in `src/web/routes.rs` to return distinct `cst_file` values
  - [x] 2.6 Implement `GET /api/files/:filename/fragments` endpoint to return fragments for a file ordered by `frag_idx`
  - [x] 2.7 Implement `GET /api/fragments/:id` endpoint to return fragment details with adjacent fragments
  - [x] 2.8 Add logic to handle edge cases (first/last fragment in file) in fragment detail endpoint
  - [x] 2.9 Update `app.js` to call `GET /api/files` and populate file list with real data
  - [x] 2.10 Update `app.js` to call `GET /api/files/:filename/fragments` when file is selected
  - [x] 2.11 Add visual indicators (Bulma tags with colors) for `frag_review` status in fragment list
  - [x] 2.12 Update `app.js` to call `GET /api/fragments/:id` when fragment is selected
  - [x] 2.13 Populate metadata fields with actual database values from selected fragment
  - [x] 2.14 Populate text areas with `content_xml` for previous/current/next fragments
  - [x] 2.15 Implement disable/hide logic for previous controls when fragment is first in file
  - [x] 2.16 Implement disable/hide logic for next controls when fragment is last in file
  - [x] 2.17 Test with actual fragments database to verify all data displays correctly
  - [x] 2.18 Verify edge cases work correctly (first/last fragments)

- [x] 3.0 Stage 3: Full Persistence and Boundary Adjustment Functionality
  - [x] 3.1 Add Diesel update models to `src/fragments_models.rs` for fragment metadata updates
  - [x] 3.2 Implement `PATCH /api/fragments/:id` endpoint for updating metadata fields
  - [x] 3.3 Add frontend blur event handlers on metadata input fields to auto-save changes
  - [x] 3.4 Convert `frag_review` field to dropdown with options: unchecked, in-progress, checked, needs-review
  - [x] 3.14 Implement modal confirmation for empty fragment deletion
  - [x] 3.15 Implement fragment deletion logic on confirmation (update adjacent fragment boundaries)
  - [x] 3.18 Add "Delete Previous Fragment" button handler with confirmation modal
  - [x] 3.19 Add "Delete Next Fragment" button handler with confirmation modal
  - [x] 3.20 Implement frontend refresh logic after boundary adjustments (reload fragment data)
  - [x] 3.21 Implement frontend refresh logic after deletions (refresh list and re-select)
  - [x] 3.22 Add error handling and user-friendly error messages for failed operations
  - [x] 3.23 Test boundary adjustments with various scenarios (line/char movements)
  - [x] 3.24 Test fragment deletion workflow with confirmations
  - [x] 3.25 Verify data integrity after operations (no gaps/overlaps in fragments)
  - [x] 3.26 Test complete workflow: review, edit, adjust boundaries, delete fragments

- [x] 4.0 Settings and UI State Persistence
  - [x] 4.1 Add menu button to right panel with Settings and Regenerate options
  - [x] 4.2 Create Settings modal with inputs for db_path, port, and xml_paths
  - [x] 4.3 Create Regenerate modal with "Work in Progress" message
  - [x] 4.4 Add toml dependency to Cargo.toml for config file support
  - [x] 4.5 Create `src/web/settings.rs` module for TOML config management
  - [x] 4.6 Implement `load_settings()` and `save_settings()` functions
  - [x] 4.7 Add AppSettings model to `src/web/models.rs`
  - [x] 4.8 Implement `GET /api/settings` endpoint
  - [x] 4.9 Implement `POST /api/settings` endpoint
  - [x] 4.10 Add localStorage persistence for selected file and fragment
  - [x] 4.11 Implement state restoration on page reload
  - [x] 4.12 Add web-ui-config.toml to .gitignore
