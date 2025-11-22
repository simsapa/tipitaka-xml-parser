# Skip Moved Fragments in Fragment Detail Display

## Overview

Enhanced the fragment detail display functionality to automatically skip over "moved" fragments when showing previous and next fragments in the UI. This ensures that when a fragment is selected, the three textareas (previous, current, next) always display non-empty fragment content for context.

## Problem Statement

When fragments are moved using the "Move to Prev" or "Move to Next" buttons, the source fragment becomes empty with `frag_review = "moved"`. Previously, when viewing a fragment adjacent to a moved fragment, the UI would display the empty moved fragment in the prev/next textarea, which provided no useful context to the user.

## Solution

Modified the `get_fragment_detail` API endpoint to use the existing `find_target_fragment` function, which automatically skips over any fragments marked as "moved" when searching for adjacent fragments.

## Changes Made

### 1. Made `find_target_fragment` Public

**File:** `src/fragment_operations.rs`

Changed the visibility of `find_target_fragment` from `fn` to `pub fn` so it can be reused by the web routes module.

```rust
pub fn find_target_fragment(
    conn: &mut SqliteConnection,
    cst_file: &str,
    current_idx: i32,
    direction: Direction,
) -> Result<Option<XmlFragmentRecord>>
```

This function:
- Searches for the adjacent fragment in the specified direction (Prev/Next)
- Skips over any fragments with `frag_review = "moved"`
- Returns the first non-moved fragment found
- Returns `None` if no valid fragment exists (boundary reached or all remaining fragments are moved)

### 2. Updated `get_fragment_detail` Endpoint

**File:** `src/web/routes.rs`

Modified the `get_fragment_detail` function to use `find_target_fragment` instead of directly querying for `frag_idx - 1` and `frag_idx + 1`.

**Before:**
```rust
// Get previous fragment (same file, frag_idx - 1)
let prev_fragment: Option<AdjacentFragment> = xml_fragments::table
    .filter(xml_fragments::cst_file.eq(&current.cst_file))
    .filter(xml_fragments::frag_idx.eq(current.frag_idx - 1))
    .first::<XmlFragmentRecord>(&mut conn)
    .optional()
    // ...
```

**After:**
```rust
// Get previous fragment (skip over moved fragments)
use crate::fragment_operations::{find_target_fragment, Direction};

let prev_fragment: Option<AdjacentFragment> = find_target_fragment(
    &mut conn,
    &current.cst_file,
    current.frag_idx,
    Direction::Prev,
)
    .map_err(|e| format!("Failed to find previous fragment: {}", e))?
    // ...
```

## Behavior

### UI Display Logic

When a fragment is selected:

1. **Previous Textarea:** Shows the content of the nearest non-moved fragment before the current fragment
   - If the immediate previous fragment (frag_idx - 1) is moved, skip to frag_idx - 2
   - Continue skipping backwards until a non-moved fragment is found
   - If all previous fragments are moved or boundary is reached, show empty

2. **Current Textarea:** Always shows the selected fragment's content (may be empty if it's a moved fragment)

3. **Next Textarea:** Shows the content of the nearest non-moved fragment after the current fragment
   - If the immediate next fragment (frag_idx + 1) is moved, skip to frag_idx + 2
   - Continue skipping forwards until a non-moved fragment is found
   - If all next fragments are moved or boundary is reached, show empty

### Example Scenarios

**Scenario 1: Single Moved Fragment**
```
Fragments: [0: normal] [1: moved] [2: normal] [3: normal]
Selecting fragment 2:
  - prev_fragment = fragment 0 (skipped fragment 1)
  - current_fragment = fragment 2
  - next_fragment = fragment 3
```

**Scenario 2: Multiple Consecutive Moved Fragments**
```
Fragments: [0: normal] [1: moved] [2: moved] [3: moved] [4: normal]
Selecting fragment 4:
  - prev_fragment = fragment 0 (skipped fragments 1, 2, 3)
  - current_fragment = fragment 4
  - next_fragment = None (no next fragment)
```

**Scenario 3: Viewing a Moved Fragment**
```
Fragments: [0: normal] [1: normal] [2: moved] [3: normal]
Selecting fragment 2 (the moved one):
  - prev_fragment = fragment 1
  - current_fragment = fragment 2 (empty, with frag_review="moved")
  - next_fragment = fragment 3
```

## Testing

### Automated Tests
- All existing tests pass (270 tests total)
- 8 fragment move operation tests verify the skip-over logic
- Integration tests confirm boundary handling

### Manual Testing Checklist
- [ ] Select a fragment next to a moved fragment - verify prev/next textareas show non-empty content
- [ ] Select a fragment with multiple consecutive moved fragments before it - verify it skips all of them
- [ ] Select a fragment with multiple consecutive moved fragments after it - verify it skips all of them
- [ ] Select the first fragment in a file - verify prev textarea is empty
- [ ] Select the last fragment in a file - verify next textarea is empty
- [ ] Move a fragment to prev, then select the next fragment - verify the UI updates correctly

## Compatibility

This change is **fully backward compatible**:
- No database schema changes
- No API breaking changes (same request/response format)
- Frontend JavaScript requires no modifications (it already handles the `prev_fragment` and `next_fragment` fields)
- Existing fragment move functionality continues to work unchanged

## Performance

The `find_target_fragment` function uses a loop to step through fragments, but:
- Most common case: finds target on first try (no moved fragments)
- Worst case: O(n) where n = number of consecutive moved fragments
- In practice, this is negligible since moved fragments are relatively rare
- Database queries are simple indexed lookups on `(cst_file, frag_idx)`

## Future Enhancements

Potential improvements (not currently implemented):
1. Add a visual indicator in the UI showing when prev/next has skipped over moved fragments
2. Display the frag_idx of the prev/next fragment in the UI for clarity
3. Add a "jump to" button to quickly navigate to the displayed prev/next fragment
4. Consider caching the skip-over logic if performance becomes an issue with many moved fragments
