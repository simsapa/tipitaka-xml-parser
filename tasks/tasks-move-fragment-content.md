# Tasks for Move Fragment Content Feature

## Relevant Files

- `src/lib.rs` - Add new public module for fragment operations helper function
- `src/fragment_operations.rs` - NEW: Core helper function for moving fragment content between adjacent fragments
- `src/web/models.rs` - Add request/response models for move fragment API endpoint
- `src/web/routes.rs` - Add POST /api/fragments/move endpoint handler
- `src/main.rs` - Update copy_reviewed_fragments_from_reference function (line 225-330) to include "moved" status
- `src/static/scripts/app.js` - Implement moveFragmentTo function and connect to existing UI buttons (lines 684-700)
- `src/static/styles/main.css` - Add CSS styling for moved fragments visual distinction
- `tests/test_fragment_move_operations.rs` - NEW: Integration tests for fragment move operations

### Notes

- Run tests with: `cargo test` or `cargo test --test test_fragment_move_operations`
- Build with: `cargo build` or `cargo build --release`
- The feature follows existing patterns from boundary adjustment (src/web/routes.rs:252-428) and fragment deletion (src/web/routes.rs:430-517)
- Diesel changeset models in src/fragments_models.rs can be used or new ones can be created as needed

## Tasks

- [ ] 1.0 Create core fragment move operation helper function
  - [ ] 1.1 Create new file `src/fragment_operations.rs` with module skeleton
  - [ ] 1.2 Add `pub mod fragment_operations;` to `src/lib.rs`
  - [ ] 1.3 Define `Direction` enum with variants `Prev` and `Next` and serde serialization
  - [ ] 1.4 Implement `find_target_fragment` helper to locate next non-moved fragment in specified direction
  - [ ] 1.5 Implement main `move_fragment_content` function signature: `pub fn move_fragment_content(conn: &mut SqliteConnection, cst_file: &str, frag_idx: i32, direction: Direction) -> Result<(XmlFragmentRecord, XmlFragmentRecord)>`
  - [ ] 1.6 Add transaction wrapper and load current fragment by cst_file and frag_idx
  - [ ] 1.7 Implement boundary error checking (first/last fragment validation)
  - [ ] 1.8 Implement skip-over logic to find target fragment (call find_target_fragment)
  - [ ] 1.9 Implement content transfer logic for moving to previous (append current to target, update boundaries)
  - [ ] 1.10 Implement content transfer logic for moving to next (prepend current to target, update boundaries)
  - [ ] 1.11 Empty current fragment content_xml and clear metadata fields (cst_code, sc_code, cst_vagga, cst_sutta, cst_paranum, sc_sutta, group_levels)
  - [ ] 1.12 Set current fragment frag_review to "moved"
  - [ ] 1.13 Execute Diesel update operations for both fragments
  - [ ] 1.14 Return tuple of (current_fragment, target_fragment) after updates
  - [ ] 1.15 Build and verify compilation succeeds: `cargo build`

- [ ] 2.0 Add API models for move fragment request/response
  - [ ] 2.1 Open `src/web/models.rs` and add `MoveFragmentRequest` struct with fields: frag_idx (i32), xml_file (String), direction (String)
  - [ ] 2.2 Add `#[derive(Serialize, Deserialize, Debug)]` attributes to MoveFragmentRequest
  - [ ] 2.3 Add `MoveFragmentResponse` struct with fields: current_fragment (FragmentListItem), target_fragment (FragmentListItem)
  - [ ] 2.4 Add `#[derive(Serialize, Deserialize, Debug)]` attributes to MoveFragmentResponse
  - [ ] 2.5 Build and verify compilation succeeds: `cargo build`

- [ ] 3.0 Implement POST /api/fragments/move endpoint in web routes
  - [ ] 3.1 Open `src/web/routes.rs` and import MoveFragmentRequest, MoveFragmentResponse from web::models
  - [ ] 3.2 Import Direction enum and move_fragment_content from crate::fragment_operations
  - [ ] 3.3 Create function signature: `fn move_fragment(request: Json<MoveFragmentRequest>, db_state: &State<DbState>) -> Result<Json<MoveFragmentResponse>, String>`
  - [ ] 3.4 Add `#[post("/api/fragments/move", data = "<request>")]` attribute
  - [ ] 3.5 Parse direction string to Direction enum with error handling
  - [ ] 3.6 Get database connection from db_state
  - [ ] 3.7 Call move_fragment_content helper function with parsed parameters
  - [ ] 3.8 Map XmlFragmentRecord results to FragmentListItem DTOs for response
  - [ ] 3.9 Return Json<MoveFragmentResponse> with current and target fragment data
  - [ ] 3.10 Add error handling with descriptive messages for boundary violations and database errors
  - [ ] 3.11 Register the new route in get_routes() function (add move_fragment to routes! macro)
  - [ ] 3.12 Build and verify compilation succeeds: `cargo build`

- [ ] 4.0 Update regeneration logic to preserve moved fragments
  - [ ] 4.1 Open `src/main.rs` and locate copy_reviewed_fragments_from_reference function (line 225)
  - [ ] 4.2 Find the filter chain that loads reviewed fragments (around line 265-270)
  - [ ] 4.3 Update the filter to include fragments with frag_review = "moved" in addition to "checked" (modify the filter logic to accept both values)
  - [ ] 4.4 Add inline comment explaining that "moved" fragments are preserved like "checked" fragments
  - [ ] 4.5 Build and verify compilation succeeds: `cargo build`
  - [ ] 4.6 Manually test regeneration flow to ensure moved fragments are copied correctly

- [ ] 5.0 Add unit tests for fragment move operations
  - [ ] 5.1 Create new file `tests/test_fragment_move_operations.rs`
  - [ ] 5.2 Add module-level doc comment explaining the test suite purpose
  - [ ] 5.3 Import required dependencies: diesel, tempfile, fragment models, and move_fragment_content function
  - [ ] 5.4 Create helper function `setup_test_db() -> (TempDir, SqliteConnection)` that creates a temp database with test fragments
  - [ ] 5.5 Implement test `test_move_to_prev_basic` - verify content is appended to previous fragment and current becomes empty with "moved" status
  - [ ] 5.6 Implement test `test_move_to_next_basic` - verify content is prepended to next fragment and current becomes empty with "moved" status
  - [ ] 5.7 Implement test `test_skip_moved_fragments_to_prev` - create chain with already-moved fragment, verify skip-over logic works
  - [ ] 5.8 Implement test `test_skip_moved_fragments_to_next` - create chain with already-moved fragment, verify skip-over logic works
  - [ ] 5.9 Implement test `test_move_to_prev_boundary_error` - verify error when moving first fragment to prev
  - [ ] 5.10 Implement test `test_move_to_next_boundary_error` - verify error when moving last fragment to next
  - [ ] 5.11 Implement test `test_metadata_cleared_on_move` - verify all metadata fields are emptied (cst_code, sc_code, etc.)
  - [ ] 5.12 Implement test `test_boundaries_updated_correctly` - verify start_line, start_char, end_line, end_char are updated properly
  - [ ] 5.13 Run all tests and fix any failures: `cargo test --test test_fragment_move_operations`

- [ ] 6.0 Update frontend JavaScript to connect move buttons to API
  - [ ] 6.1 Open `src/static/scripts/app.js` and locate the move button event listeners (lines 684-700)
  - [ ] 6.2 Create async function `moveFragmentTo(direction)` that takes 'prev' or 'next' as parameter
  - [ ] 6.3 Add validation to check if state.selectedFragmentId exists
  - [ ] 6.4 Get current fragment detail to retrieve cst_file (call getCurrentFragmentDetail)
  - [ ] 6.5 Construct request body with frag_idx, xml_file (cst_file), and direction
  - [ ] 6.6 Make POST request to `/api/fragments/move` with JSON body
  - [ ] 6.7 Handle response: extract current_fragment and target_fragment from result
  - [ ] 6.8 Update DOM for current fragment using updateFragmentItemInList with current_fragment data
  - [ ] 6.9 Update DOM for target fragment using updateFragmentItemInList with target_fragment data
  - [ ] 6.10 Refresh the current fragment detail view to show updated boundaries and content
  - [ ] 6.11 Add error handling with user-friendly alert messages
  - [ ] 6.12 Update the move-to-prev button onclick handler to call moveFragmentTo('prev') (replace FIXME comment on line 688)
  - [ ] 6.13 Update the move-to-next button onclick handler to call moveFragmentTo('next') (replace FIXME comment on line 697)
  - [ ] 6.14 Test in browser: verify move buttons work and UI updates correctly

- [ ] 7.0 Add CSS styling for moved fragments visual distinction
  - [ ] 7.1 Open `src/static/styles/main.css`
  - [ ] 7.2 Locate the fragment list item styling section (around line 84-128)
  - [ ] 7.3 Add new CSS rule for moved fragments: `.panel-item[data-frag-review="moved"]` with darker gray background (e.g., `background-color: #d0d0d0;`)
  - [ ] 7.4 Add dark mode variant: `[data-theme="dark"] .panel-item[data-frag-review="moved"]` with appropriate darker background
  - [ ] 7.5 Open `src/static/scripts/app.js` and locate fetchAndPopulateFragmentList function (line 109)
  - [ ] 7.6 Update fragment item creation to add `data-frag-review` attribute: `item.dataset.fragReview = fragment.frag_review || '';` (add after line 134)
  - [ ] 7.7 Update updateFragmentItemInList function (line 161) to also set the data-frag-review attribute when updating
  - [ ] 7.8 Test in browser: verify moved fragments display with darker gray background
