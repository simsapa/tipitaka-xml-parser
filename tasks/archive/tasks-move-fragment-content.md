# Tasks for Move Fragment Content Feature

## Relevant Files

- `src/lib.rs` - Added public module declaration for fragment_operations
- `src/fragment_operations.rs` - NEW: Core helper functions for moving fragment content (Direction enum, find_target_fragment, move_fragment_content)
- `src/fragments_models.rs` - Added ClearMovedFragmentMetadata changeset and Debug derive to XmlFragmentRecord
- `src/web/models.rs` - Added MoveFragmentRequest and MoveFragmentResponse models for API endpoint
- `src/web/routes.rs` - Added POST /api/fragments/move endpoint handler with Direction enum parsing and error handling
- `src/main.rs` - Updated copy_reviewed_fragments_from_reference function to preserve "moved" fragments during regeneration
- `src/static/scripts/app.js` - Implemented moveFragmentTo async function and connected move-to-prev/next button handlers, added data-frag-review attribute tracking
- `src/static/styles/main.css` - Added CSS styling for moved fragments (darker gray background in light/dark modes)
- `tests/test_fragment_move_operations.rs` - NEW: Comprehensive integration tests for fragment move operations (8 tests covering basic moves, skip-over logic, boundary errors, metadata clearing, and boundary updates)

### Notes

- Run tests with: `cargo test` or `cargo test --test test_fragment_move_operations`
- Build with: `cargo build` or `cargo build --release`
- The feature follows existing patterns from boundary adjustment (src/web/routes.rs:252-428) and fragment deletion (src/web/routes.rs:430-517)
- Diesel changeset models in src/fragments_models.rs can be used or new ones can be created as needed

## Tasks

- [x] 1.0 Create core fragment move operation helper function
  - [x] 1.1 Create new file `src/fragment_operations.rs` with module skeleton
  - [x] 1.2 Add `pub mod fragment_operations;` to `src/lib.rs`
  - [x] 1.3 Define `Direction` enum with variants `Prev` and `Next` and serde serialization
  - [x] 1.4 Implement `find_target_fragment` helper to locate next non-moved fragment in specified direction
  - [x] 1.5 Implement main `move_fragment_content` function signature: `pub fn move_fragment_content(conn: &mut SqliteConnection, cst_file: &str, frag_idx: i32, direction: Direction) -> Result<(XmlFragmentRecord, XmlFragmentRecord)>`
  - [x] 1.6 Add transaction wrapper and load current fragment by cst_file and frag_idx
  - [x] 1.7 Implement boundary error checking (first/last fragment validation)
  - [x] 1.8 Implement skip-over logic to find target fragment (call find_target_fragment)
  - [x] 1.9 Implement content transfer logic for moving to previous (append current to target, update boundaries)
  - [x] 1.10 Implement content transfer logic for moving to next (prepend current to target, update boundaries)
  - [x] 1.11 Empty current fragment content_xml and clear metadata fields (cst_code, sc_code, cst_vagga, cst_sutta, cst_paranum, sc_sutta, group_levels)
  - [x] 1.12 Set current fragment frag_review to "moved"
  - [x] 1.13 Execute Diesel update operations for both fragments
  - [x] 1.14 Return tuple of (current_fragment, target_fragment) after updates
  - [x] 1.15 Build and verify compilation succeeds: `cargo build`

- [x] 2.0 Add API models for move fragment request/response
  - [x] 2.1 Open `src/web/models.rs` and add `MoveFragmentRequest` struct with fields: frag_idx (i32), xml_file (String), direction (String)
  - [x] 2.2 Add `#[derive(Serialize, Deserialize, Debug)]` attributes to MoveFragmentRequest
  - [x] 2.3 Add `MoveFragmentResponse` struct with fields: current_fragment (FragmentListItem), target_fragment (FragmentListItem)
  - [x] 2.4 Add `#[derive(Serialize, Deserialize, Debug)]` attributes to MoveFragmentResponse
  - [x] 2.5 Build and verify compilation succeeds: `cargo build`

- [x] 3.0 Implement POST /api/fragments/move endpoint in web routes
  - [x] 3.1 Open `src/web/routes.rs` and import MoveFragmentRequest, MoveFragmentResponse from web::models
  - [x] 3.2 Import Direction enum and move_fragment_content from crate::fragment_operations
  - [x] 3.3 Create function signature: `fn move_fragment(request: Json<MoveFragmentRequest>, db_state: &State<DbState>) -> Result<Json<MoveFragmentResponse>, String>`
  - [x] 3.4 Add `#[post("/api/fragments/move", data = "<request>")]` attribute
  - [x] 3.5 Parse direction string to Direction enum with error handling
  - [x] 3.6 Get database connection from db_state
  - [x] 3.7 Call move_fragment_content helper function with parsed parameters
  - [x] 3.8 Map XmlFragmentRecord results to FragmentListItem DTOs for response
  - [x] 3.9 Return Json<MoveFragmentResponse> with current and target fragment data
  - [x] 3.10 Add error handling with descriptive messages for boundary violations and database errors
  - [x] 3.11 Register the new route in get_routes() function (add move_fragment to routes! macro)
  - [x] 3.12 Build and verify compilation succeeds: `cargo build`

- [x] 4.0 Update regeneration logic to preserve moved fragments
  - [x] 4.1 Open `src/main.rs` and locate copy_reviewed_fragments_from_reference function (line 225)
  - [x] 4.2 Find the filter chain that loads reviewed fragments (around line 265-270)
  - [x] 4.3 Update the filter to include fragments with frag_review = "moved" in addition to "checked" (modify the filter logic to accept both values)
  - [x] 4.4 Add inline comment explaining that "moved" fragments are preserved like "checked" fragments
  - [x] 4.5 Build and verify compilation succeeds: `cargo build`
  - [ ] 4.6 Manually test regeneration flow to ensure moved fragments are copied correctly

- [x] 5.0 Add unit tests for fragment move operations
  - [x] 5.1 Create new file `tests/test_fragment_move_operations.rs`
  - [x] 5.2 Add module-level doc comment explaining the test suite purpose
  - [x] 5.3 Import required dependencies: diesel, tempfile, fragment models, and move_fragment_content function
  - [x] 5.4 Create helper function `setup_test_db() -> (TempDir, SqliteConnection)` that creates a temp database with test fragments
  - [x] 5.5 Implement test `test_move_to_prev_basic` - verify content is appended to previous fragment and current becomes empty with "moved" status
  - [x] 5.6 Implement test `test_move_to_next_basic` - verify content is prepended to next fragment and current becomes empty with "moved" status
  - [x] 5.7 Implement test `test_skip_moved_fragments_to_prev` - create chain with already-moved fragment, verify skip-over logic works
  - [x] 5.8 Implement test `test_skip_moved_fragments_to_next` - create chain with already-moved fragment, verify skip-over logic works
  - [x] 5.9 Implement test `test_move_to_prev_boundary_error` - verify error when moving first fragment to prev
  - [x] 5.10 Implement test `test_move_to_next_boundary_error` - verify error when moving last fragment to next
  - [x] 5.11 Implement test `test_metadata_cleared_on_move` - verify all metadata fields are emptied (cst_code, sc_code, etc.)
  - [x] 5.12 Implement test `test_boundaries_updated_correctly` - verify start_line, start_char, end_line, end_char are updated properly
  - [x] 5.13 Run all tests and fix any failures: `cargo test --test test_fragment_move_operations`

- [x] 6.0 Update frontend JavaScript to connect move buttons to API
  - [x] 6.1 Open `src/static/scripts/app.js` and locate the move button event listeners (lines 684-700)
  - [x] 6.2 Create async function `moveFragmentTo(direction)` that takes 'prev' or 'next' as parameter
  - [x] 6.3 Add validation to check if state.selectedFragmentId exists
  - [x] 6.4 Get current fragment detail to retrieve cst_file (call getCurrentFragmentDetail)
  - [x] 6.5 Construct request body with frag_idx, xml_file (cst_file), and direction
  - [x] 6.6 Make POST request to `/api/fragments/move` with JSON body
  - [x] 6.7 Handle response: extract current_fragment and target_fragment from result
  - [x] 6.8 Update DOM for current fragment using updateFragmentItemInList with current_fragment data
  - [x] 6.9 Update DOM for target fragment using updateFragmentItemInList with target_fragment data
  - [x] 6.10 Refresh the current fragment detail view to show updated boundaries and content
  - [x] 6.11 Add error handling with user-friendly alert messages
  - [x] 6.12 Update the move-to-prev button onclick handler to call moveFragmentTo('prev') (replace FIXME comment on line 688)
  - [x] 6.13 Update the move-to-next button onclick handler to call moveFragmentTo('next') (replace FIXME comment on line 697)
  - [ ] 6.14 Test in browser: verify move buttons work and UI updates correctly

- [x] 7.0 Add CSS styling for moved fragments visual distinction
  - [x] 7.1 Open `src/static/styles/main.css`
  - [x] 7.2 Locate the fragment list item styling section (around line 84-128)
  - [x] 7.3 Add new CSS rule for moved fragments: `.panel-item[data-frag-review="moved"]` with darker gray background (e.g., `background-color: #d0d0d0;`)
  - [x] 7.4 Add dark mode variant: `[data-theme="dark"] .panel-item[data-frag-review="moved"]` with appropriate darker background
  - [x] 7.5 Open `src/static/scripts/app.js` and locate fetchAndPopulateFragmentList function (line 109)
  - [x] 7.6 Update fragment item creation to add `data-frag-review` attribute: `item.dataset.fragReview = fragment.frag_review || '';` (add after line 134)
  - [x] 7.7 Update updateFragmentItemInList function (line 161) to also set the data-frag-review attribute when updating
  - [ ] 7.8 Test in browser: verify moved fragments display with darker gray background
