# PRD: Move Fragment Content Feature

## Introduction/Overview

This feature allows users to correct fragment boundaries that were incorrectly split during XML parsing by moving the content of a fragment to an adjacent (previous or next) fragment. The current fragment becomes empty but remains in the database to maintain consistent fragment indexing across regeneration cycles.

**Problem:** During XML parsing, fragment boundaries may be incorrectly detected, resulting in content being split at the wrong location. Currently, there is no easy way to merge fragment content without losing the consistent fragment count.

**Goal:** Provide a user-friendly way to move fragment content between adjacent fragments while preserving the total number of fragments, ensuring that regeneration processes continue to work correctly.

## Goals

1. Enable users to move XML content from the current fragment to the previous or next fragment
2. Maintain the total number of fragments (no deletion) for consistent regeneration
3. Mark moved fragments with a `"moved"` review status for tracking and UI display
4. Skip over already-moved fragments when finding the target fragment
5. Update line and character boundaries correctly after content transfer
6. Ensure moved fragments are preserved during the regeneration process (copied from reference DB)

## User Stories

1. **As a fragment reviewer**, I want to move incorrectly split content to the previous fragment, so that the fragment boundary is corrected without changing the total fragment count.

2. **As a fragment reviewer**, I want moved fragments to be visually distinct (darker gray) in the UI, so I can easily identify which fragments have been consolidated.

3. **As a fragment reviewer**, I want the system to automatically skip over already-moved fragments when finding a target, so that content goes to the nearest non-empty fragment.

4. **As a developer**, I want moved fragments to be preserved during regeneration (copied from reference DB like "checked" fragments), so that manual corrections are not lost.

## Functional Requirements

### FR1: Move to Previous Fragment
The system must allow users to move the current fragment's content to the previous fragment by:
- Finding the target previous fragment (skipping any with `frag_review = "moved"`)
- **Appending** the current fragment's `content_xml` to the end of the target fragment's `content_xml`
- Extending the target fragment's `end_line` and `end_char` to match the current fragment's end boundaries
- Setting both the current fragment's `start_line`, `start_char`, `end_line`, and `end_char` to match the target's new end boundaries
- Emptying the current fragment's `content_xml` (set to empty string)
- Marking the current fragment's `frag_review` status as `"moved"`
- Emptying the current fragment's `cst_code`, `sc_code`, `cst_vagga`, `cst_sutta`, `cst_paranum`, `sc_sutta`, and `group_levels` attributes

### FR2: Move to Next Fragment
The system must allow users to move the current fragment's content to the next fragment by:
- Finding the target next fragment (skipping any with `frag_review = "moved"`)
- **Prepending** the current fragment's `content_xml` to the beginning of the target fragment's `content_xml`
- Extending the target fragment's `start_line` and `start_char` to match the current fragment's start boundaries
- Setting both the current fragment's `start_line`, `start_char`, `end_line`, and `end_char` to match the target's new start boundaries
- Emptying the current fragment's `content_xml` (set to empty string)
- Marking the current fragment's `frag_review` status as `"moved"`
- Emptying the current fragment's `cst_code`, `sc_code`, `cst_vagga`, `cst_sutta`, `cst_paranum`, `sc_sutta`, and `group_levels` attributes

### FR3: Skip Already-Moved Fragments
When searching for a target fragment, the system must:
- Check if the immediate previous/next fragment has `frag_review = "moved"`
- If yes, continue stepping in that direction (decrementing or incrementing `frag_idx`) until finding a fragment where `frag_review != "moved"`
- Use the first non-moved fragment as the target
- Return an error if all remaining fragments in that direction are marked as `"moved"`

### FR4: Boundary Error Handling
The system must return an error when:
- Attempting to move to previous when the current fragment is the first in the file (no previous fragment exists)
- Attempting to move to next when the current fragment is the last in the file (no next fragment exists)
- All remaining fragments in the target direction are marked as `"moved"` (no valid target exists)

### FR5: API Endpoint
The system must provide a REST API endpoint:
- **Path:** `POST /api/fragments/move`
- **Request Body:**
  ```json
  {
    "frag_idx": 5,
    "xml_file": "s0101m.mul.xml",
    "direction": "prev" | "next"
  }
  ```
- **Success Response:** Returns the current fragment and target fragment with updated values
  ```json
  {
    "current_fragment": { /* FragmentListItem with frag_idx, frag_review="moved", etc. */ },
    "target_fragment": { /* FragmentListItem with updated content */ }
  }
  ```
- **Error Response:** Returns error message for boundary violations or no valid target

### FR6: UI Integration
The system must:
- Connect the existing `id="move-to-prev"` and `id="move-to-next"` buttons to the backend API
- Update the DOM with the returned fragment data (using `frag_idx` to identify elements)
- Display moved fragments with a darker gray color in the fragment list (based on `frag_review = "moved"`)

### FR7: Helper Function for Testing
The system must provide a reusable Rust helper function:
- **Function signature:** `move_fragment_content(conn: &mut SqliteConnection, cst_file: &str, frag_idx: i32, direction: Direction) -> Result<(XmlFragmentRecord, XmlFragmentRecord)>`
- Returns a tuple of `(current_fragment, target_fragment)` after the move operation
- Can be called from both the web API handler and unit tests
- Performs all database operations within a transaction

### FR8: Regeneration Integration
The system must preserve moved fragments during regeneration:
- When regenerating with a reference database (via `--reference-fragments-db`), fragments with `frag_review = "moved"` must be copied from the reference DB to the new DB
- This should follow the same pattern as copying fragments with `frag_review = "checked"` (see `copy_reviewed_fragments_from_reference` in `src/main.rs:875-940`)
- The moved fragment replaces the newly generated fragment at the same `frag_idx` position

### FR9: Database Transaction Safety
All move operations must:
- Execute within a database transaction
- Rollback all changes if any step fails
- Return appropriate error messages for database failures

## Non-Goals (Out of Scope)

1. **Undo functionality** - Users cannot undo a move operation (they can manually re-adjust boundaries if needed)
2. **Filtering or hiding moved fragments** - Moved fragments remain visible in the UI with visual distinction only
3. **Merging content across different XML files** - Move operations only work within a single file's fragments
4. **Concurrency handling** - This is a single-user application; no multi-user conflict resolution needed
5. **Size limit validation** - No restriction on how large the target fragment's `content_xml` can become
6. **Automatic fragment cleanup** - Empty/moved fragments are never automatically deleted

## Design Considerations

### UI/UX
- Buttons `id="move-to-prev"` and `id="move-to-next"` already exist in the UI
- Conditional disabling and confirmation dialogs are already implemented
- Moved fragments should have a darker gray background color in the fragment list (apply via CSS class based on `frag_review` status)

### Database Schema
Relevant fields in `xml_fragments` table (from `src/fragments_schema.rs:13-35`):
- `id` (primary key)
- `cst_file` (identifies which XML file)
- `frag_idx` (fragment position, 0-indexed)
- `frag_review` (nullable text field for status like "checked", "moved", etc.)
- `content_xml` (the XML content)
- `start_line`, `start_char`, `end_line`, `end_char` (position boundaries)
- `cst_code`, `sc_code`, `cst_vagga`, `cst_sutta`, `cst_paranum`, `sc_sutta` (metadata to clear)
- `group_levels` (metadata to clear)

### API Response Models
Create new models in `src/web/models.rs`:
```rust
#[derive(Serialize, Deserialize, Debug)]
pub struct MoveFragmentRequest {
    pub frag_idx: i32,
    pub xml_file: String,
    pub direction: String, // "prev" or "next"
}

#[derive(Serialize, Deserialize, Debug)]
pub struct MoveFragmentResponse {
    pub current_fragment: FragmentListItem,
    pub target_fragment: FragmentListItem,
}
```

## Technical Considerations

### Implementation Location
- **Helper function:** Create in a new module or in an existing utility module (e.g., `src/fragment_operations.rs` or add to `src/lib.rs`)
- **API handler:** Add to `src/web/routes.rs`
- **Models:** Add to `src/web/models.rs`
- **Regeneration logic:** Modify `src/main.rs` in the `copy_reviewed_fragments_from_reference` function (around line 875-940)

### Dependencies
- Uses existing Diesel models: `XmlFragmentRecord`, `UpdateFragmentBoundary`, `UpdateFragmentMetadata`
- May need to create a new Diesel changeset for updating all fields at once (or use existing `UpdateFragmentFromReference` as reference)

### Content Transfer Logic
When moving to previous:
```
target.content_xml = target.content_xml + current.content_xml
target.end_line = current.end_line
target.end_char = current.end_char
current.start_line = target.end_line
current.start_char = target.end_char
current.end_line = target.end_line
current.end_char = target.end_char
current.content_xml = ""
```

When moving to next:
```
target.content_xml = current.content_xml + target.content_xml
target.start_line = current.start_line
target.start_char = current.start_char
current.start_line = target.start_line
current.start_char = target.start_char
current.end_line = target.start_line
current.end_char = target.start_char
current.content_xml = ""
```

### Similar Existing Code
Reference the boundary adjustment implementation in `src/web/routes.rs:252-428` for:
- Database transaction pattern
- Loading fragments by `cst_file` and `frag_idx`
- Using Diesel update operations

## Success Metrics

1. **Functional correctness:** Users can successfully move fragment content in both directions without errors
2. **Data integrity:** Fragment count remains consistent before and after move operations
3. **Regeneration preservation:** Moved fragments are correctly copied from reference DB during regeneration (100% of "moved" fragments preserved)
4. **UI responsiveness:** DOM updates correctly reflect the moved status and updated content
5. **Error handling:** Appropriate error messages displayed for boundary violations

## Open Questions

1. ~~Should there be a maximum number of "moved" fragments that can be skipped when finding a target?~~ **Answer:** No limit specified; keep stepping until a non-moved fragment is found or boundary is reached.

2. ~~What should happen to `content_html` field when moving content?~~ **Answer:** Not mentioned in requirements; assume it can remain as-is or be set to `None` for emptied fragments. Document this behavior in code comments.

3. ~~Should the UI show a confirmation before executing the move (beyond what's already implemented)?~~ **Answer:** Confirmation dialogs already exist; no additional confirmation needed.

4. ~~Should the system log move operations for audit purposes?~~ **Answer:** Not required; standard application logging is sufficient.
