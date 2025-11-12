# PRD: Web UI for Fragment Review and Correction

## Introduction/Overview

This feature adds a web-based user interface for manually reviewing and correcting the XML fragments stored in the `xml_fragments` database table. The UI allows users to inspect fragment data, adjust fragment boundaries (line/character positions), edit fragment metadata, and mark fragments as reviewed. This addresses the need for manual quality control and correction of automatically generated fragment data before it is used in production workflows.

The feature will be implemented in three stages:
1. **Stage 1**: Basic structure with Rust modules, web assets, Bulma CSS layout, and mock data
2. **Stage 2**: Database integration to display real fragment data
3. **Stage 3**: Full persistence and boundary adjustment functionality

## Goals

1. Enable manual review and correction of XML fragment data through an intuitive web interface
2. Provide visual tools for adjusting fragment boundaries without directly editing database values
3. Track review status of fragments using the `frag_review` column with multiple states
4. Support a complete correction workflow that feeds into database regeneration
5. Create a maintainable, single-user web application using Rocket and Bulma CSS

## User Stories

1. **As a data reviewer**, I want to browse XML files and their fragments so that I can systematically review the parsing results.

2. **As a data reviewer**, I want to see the content of adjacent fragments (previous, current, next) so that I can understand the context and identify boundary issues.

3. **As a data reviewer**, I want to adjust fragment boundaries using simple controls so that I can fix incorrect line/character splits without manual calculation.

4. **As a data reviewer**, I want to edit fragment metadata (cst_code, cst_vagga, etc.) so that I can correct parsing errors in classification.

5. **As a data reviewer**, I want to mark fragments with review states (unchecked, in-progress, checked, needs-review) so that I can track my progress and flag problematic fragments.

6. **As a data reviewer**, I want to delete fragments and redistribute their content when parsing created spurious fragments so that I can clean up the database.

7. **As a developer**, I want the UI to work with a local database file so that I can review and correct data without affecting production systems.

## Functional Requirements

### Stage 1: Basic Structure and Layout

1. The system must provide a new CLI command `web-ui` that starts the Rocket web server.
2. The web server must run on port 8000 by default, with an optional `--port` flag to override.
3. The system must serve static web assets (HTML, CSS, JavaScript) using Bulma CSS framework.
4. The UI must implement a two-panel layout matching `ui_wireframe.txt`:
   - Left panel: File list (top), fragment list (middle), metadata input fields (bottom)
   - Right panel: Three text areas for previous/current/next fragment content with boundary controls between them
5. The panel separator must be draggable to resize the left and right panels.
6. The system must display mock fragment data (2-3 XML files with small snippets, not full files).
7. File list must be clickable to select a file and display its fragments.
8. Fragment list must show `frag_idx` and `frag_type` for each fragment.
9. Fragment list must be clickable to select a fragment and display its details.
10. The left panel must display input fields for editable fragment properties: `frag_type`, `frag_review`, `cst_code`, `cst_vagga`, `cst_sutta`, `cst_paranum`, `sc_code`, `sc_sutta`.
11. The right panel must display three text areas showing `content_xml` for previous, current, and next fragments.
12. Between text areas, the UI must display control button rows:
    - Between previous and current: 4 buttons to adjust the boundary
    - Between current and next: 4 buttons to adjust the boundary
13. Above the previous fragment text area, the UI must display a "Delete Previous Fragment" button.
14. Below the next fragment text area, the UI must display a "Delete Next Fragment" button.
15. All interactive elements (list selection, panel dragging, buttons) must be functional with mock data.

### Stage 2: Database Integration

16. The `web-ui` CLI command must accept a required argument for the database file path: `web-ui <db_path>`.
17. The system must connect to the SQLite database at the provided path using Diesel ORM.
18. The file list must populate with distinct `cst_file` values from the `xml_fragments` table.
19. When a file is selected, the fragment list must display all fragments for that file, ordered by `frag_idx`.
20. The fragment list must show visual indicators (icons/badges) for the `frag_review` status of each fragment.
21. When a fragment is selected, the metadata input fields must populate with that fragment's database values.
22. The right panel must display the actual `content_xml` for the selected fragment and its adjacent fragments (previous/next by `frag_idx`).
23. If the selected fragment is the first in the file, the "previous" area and delete button must be disabled/hidden.
24. If the selected fragment is the last in the file, the "next" area and delete button must be disabled/hidden.

### Stage 3: Full Persistence and Boundary Adjustment

25. When metadata input fields are edited and lose focus (blur event), the system must save the changes to the database.
26. The `frag_review` field must support dropdown selection with values: `unchecked`, `in-progress`, `checked`, `needs-review`.
27. Boundary control buttons must adjust fragment line/character positions with zero-sum behavior:
    - "Line Up" moves content from current to previous (decrements current `start_line`)
    - "Line Down" moves content from previous to current (increments current `start_line`)
    - "Char Left" moves content from current to previous (decrements current `start_char`)
    - "Char Right" moves content from previous to current (increments current `start_char`)
28. The second button row operates similarly for the current/next boundary.
29. Boundary adjustments must immediately update the affected fragments in the database.
30. Boundary adjustments must recalculate `end_line`/`end_char` of the shrinking fragment and `start_line`/`start_char` of the growing fragment.
31. The system must reload and re-display fragment content after boundary adjustments.
32. If a boundary adjustment would result in an empty fragment (content completely moved), the system must:
    - Display a confirmation modal: "This action will make Fragment X empty. Delete it and redistribute content?"
    - On confirmation: Delete the empty fragment and update the adjacent fragment's boundary
    - On cancellation: Revert the action
33. The "Delete Previous Fragment" button must:
    - Display a confirmation modal: "Delete Fragment X and move its content to the current fragment?"
    - On confirmation: Delete the previous fragment, extend current fragment's `start_line`/`start_char` to include previous content, update `frag_idx` values
34. The "Delete Next Fragment" button must operate similarly for the next fragment.
35. After any deletion, the system must refresh the fragment list and re-select the current fragment.
36. The system must persist no state between server restarts (fresh start each time).

## Non-Goals (Out of Scope)

1. Multi-user access or concurrent editing support (single-user only)
2. Authentication or authorization mechanisms
3. Undo/redo functionality for edits
4. Export functionality for corrected data (use existing CLI tools)
5. Real-time validation of XML structure or fragment content
6. Automatic regeneration of fragments database (manual process using existing tools)
7. Search or filter functionality for fragments
8. Keyboard shortcuts for navigation
9. Responsive mobile design (desktop-only)
10. Dark mode or theme customization
11. Saving UI state (panel sizes, selected file/fragment) between sessions

## Design Considerations

### UI Framework
- **Bulma CSS**: Use Bulma's panel, columns, buttons, form controls, and modal components
- **Layout**: Two-column layout using Bulma columns with a draggable separator (custom JS)
- **Panel Structure**: Left panel uses nested panels for file list, fragment list, and form sections
- **Right Panel**: Three stacked text areas with button groups between them

### Visual Indicators
- **Review Status Badges**: Use Bulma tags with colors:
  - NULL or empty (meaning unchecked): no special style
  - `in-progress`: yellow
  - `checked`: green
  - `needs-review`: red
- **Selected Item**: Highlight with Bulma `is-active` class
- **Disabled Elements**: Use Bulma `is-disabled` for boundary controls at file edges

### Boundary Control Buttons
- **First Row** (previous ↔ current): `[Merge Prev] [Line ↑] [Line ↓] [Char ←] [Char →]`
- **Second Row** (current ↔ next): `[Line ↑] [Line ↓] [Char ←] [Char →] [Merge Next]`
- Use Bulma button groups for clean alignment

### Modals
- Use Bulma modal component for confirmation dialogs
- Include clear messaging about the action and affected fragment indices

## Technical Considerations

### Backend Architecture
- **Web Framework**: Rocket (latest stable version)
- **Database**: Diesel ORM with SQLite (reuse existing schema from `fragments_schema.rs`)
- **Project Structure**:
  ```
  src/
    web/
      mod.rs           # Module declaration
      routes.rs        # API endpoint handlers
      models.rs        # Web-specific models/DTOs
      state.rs         # Rocket state management
    static/
      index.html       # Main HTML page
      styles/
        main.css       # Custom styles (supplementing Bulma)
      scripts/
        app.js         # Frontend JavaScript logic
  ```

### API Endpoints (RESTful)
- `GET /api/files` - List distinct cst_file values
- `GET /api/files/:filename/fragments` - List fragments for a file
- `GET /api/fragments/:id` - Get fragment details with adjacent fragments
- `PATCH /api/fragments/:id` - Update fragment metadata
- `POST /api/fragments/:id/adjust-boundary` - Adjust boundary with adjacent fragment
- `DELETE /api/fragments/:id` - Delete fragment and redistribute content

### Frontend
- **Vanilla JavaScript** or lightweight framework (Alpine.js suggested for reactivity)
- **Fetch API** for backend communication
- **Panel resizing**: Use JavaScript mouse events on separator element
- **State management**: Simple client-side object tracking selected file/fragment

### Database Operations
- All writes must use transactions to ensure consistency
- Boundary adjustments affect two fragments atomically
- Fragment deletion requires recalculating `frag_idx` for subsequent fragments in the same file

### Error Handling
- Display user-friendly error messages for failed operations
- Log detailed errors to server console
- Handle edge cases (missing adjacent fragments, invalid boundary adjustments)

## Success Metrics

1. **Functionality**: All three stages implemented and working with test database
2. **Usability**: Reviewer can process and mark 20+ fragments as reviewed within 10 minutes
3. **Data Integrity**: Boundary adjustments maintain fragment contiguity (no gaps/overlaps)
4. **Correctness**: Manual edits persist correctly and can be used in subsequent regeneration workflows
5. **Stability**: Web server runs without crashes during typical review session (1-2 hours)

## Open Questions

1. Should the boundary control buttons show the actual line/char numbers that will result from the adjustment?
2. Do we need a "Save All" or "Discard Changes" button, or is auto-save on blur sufficient?
3. Should there be a visual diff highlighting what content moves when boundaries are adjusted?
4. Should the system validate that `content_xml` remains valid XML after boundary adjustments?
5. Is there a maximum reasonable size for fragment content that should trigger a warning in the UI?
6. Should fragment deletion preserve any historical record, or is permanent deletion acceptable?
7. Do we need to handle the case where the database is modified externally while the web UI is running?
8. Should the UI display `start_line`, `start_char`, `end_line`, `end_char` as read-only reference information?

## Implementation Stages - Detailed Breakdown

### Stage 1 Deliverables
- [ ] Rocket web server with basic route serving `index.html`
- [ ] `src/web/` module structure
- [ ] `src/static/` with Bulma CSS integrated
- [ ] Two-panel layout with draggable separator
- [ ] Mock data structure (2-3 files, 5-10 fragments each)
- [ ] File and fragment list rendering from mock data
- [ ] Fragment selection updates metadata form and content areas
- [ ] All buttons present and styled (non-functional for persistence)

### Stage 2 Deliverables
- [ ] CLI argument parsing for database path
- [ ] Diesel connection pool in Rocket state
- [ ] API endpoints for reading files and fragments
- [ ] Frontend calls API to populate UI with real data
- [ ] Review status indicators display correctly
- [ ] Edge case handling (first/last fragment in file)

### Stage 3 Deliverables
- [ ] PATCH endpoint for metadata updates
- [ ] POST endpoint for boundary adjustments with transaction handling
- [ ] DELETE endpoint for fragment deletion with confirmation
- [ ] Modal component for confirmations
- [ ] Full boundary adjustment logic (zero-sum, recalculation)
- [ ] Empty fragment detection and handling
- [ ] Frontend updates after modifications (refresh data)

## Related Files

- Database schema: `src/fragments_schema.rs`
- Existing models: `src/fragments_models.rs`
- UI wireframe: `ui_wireframe.txt`
- Existing CLI: `src/main.rs`
