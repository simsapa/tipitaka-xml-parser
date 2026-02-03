# PRD: ArangoDB Integration and Database Validation

## 1. Introduction/Overview

This feature integrates the web UI with a local SuttaCentral ArangoDB instance to enable fetching Pali sutta titles and adds a database validation system to identify and fix data quality issues.

**Problem Statement:**
- Fragment records in the SQLite database need SuttaCentral sutta titles (`sc_sutta`) to be populated based on their `sc_code` values
- There is currently no way to validate database integrity or identify records with missing required fields
- Manual correction of missing titles is tedious when the data is available from SuttaCentral's ArangoDB

**Solution:**
1. Add ArangoDB connection status indicator with periodic health checks
2. Cache Pali sutta titles from ArangoDB on startup for quick lookups
3. Provide a validation modal to run integrity checks and auto-fix certain issues

## 2. Goals

1. **ArangoDB Connection Monitoring**: Display real-time connection status to the local SuttaCentral ArangoDB instance
2. **Title Caching**: Cache all Pali sutta titles from ArangoDB on UI startup for filtering without repeated DB queries
3. **Database Validation**: Provide a validation modal accessible from Menu > Database Validation
4. **Auto-Fix Capability**: Enable one-click fixing of missing `sc_sutta` values using ArangoDB data

## 3. User Stories

1. **As a reviewer**, I want to see whether ArangoDB is connected so I know if title lookups will work.

2. **As a reviewer**, I want the UI to automatically cache sutta titles on startup so I can filter and search titles without waiting for database queries.

3. **As a reviewer**, I want to run validation checks to identify fragments with missing required data so I can prioritize corrections.

4. **As a reviewer**, I want to auto-fix missing `sc_sutta` values using ArangoDB data so I don't have to manually look them up and enter them one by one.

5. **As a reviewer**, I want to click on a validation error to navigate directly to that fragment so I can investigate and fix the issue.

## 4. Functional Requirements

### 4.1 ArangoDB Connection Status Indicator

1. **FR-1.1**: The system must display a status indicator next to the "Menu" button in the left panel header.

2. **FR-1.2**: The indicator must be **green** when successfully connected to ArangoDB at `localhost:8529`.

3. **FR-1.3**: The indicator must be **red** when the connection fails or ArangoDB is unavailable.

4. **FR-1.4**: When the indicator is red, hovering over it must display a tooltip explaining the connection issue (e.g., "Cannot connect to ArangoDB at localhost:8529").

5. **FR-1.5**: The system must check the ArangoDB connection status every **5 seconds**.

6. **FR-1.6**: The connection must use hardcoded credentials:
   - Host: `http://localhost:8529`
   - Username: `root`
   - Password: `test`
   - Database: `suttacentral`

### 4.2 Pali Title Caching

7. **FR-2.1**: On web UI startup, the system must make a background request to a Rocket API endpoint to fetch all Pali titles from ArangoDB.

8. **FR-2.2**: The Rocket handler must query the ArangoDB `names` collection with: `FOR x IN names FILTER x.is_root == true RETURN x`

9. **FR-2.3**: The response must be a JSON object mapping `uid` (sc_code) to `name` (title).

10. **FR-2.4**: The frontend must cache the titles in a global JavaScript variable (e.g., `window.paliTitlesCache`).

11. **FR-2.5**: If ArangoDB is unavailable on startup but becomes available later (detected via the 5-second health check), the system must automatically fetch and cache the titles at that time.

12. **FR-2.6**: No manual refresh mechanism is required; titles are only refreshed on page reload.

### 4.3 Validation Modal UI

13. **FR-3.1**: The system must add a new menu item "Database Validation" under the Menu dropdown.

14. **FR-3.2**: Clicking "Database Validation" must open a modal window for running validation checks.

15. **FR-3.3**: The modal must have a **split layout**:
    - **Left side**: List of validation check types with badges showing error counts
    - **Right side**: Results list for the currently selected check type

16. **FR-3.4**: The modal must include a "Run Validation" button that triggers all validation checks.

17. **FR-3.5**: Validation checks must only run when manually triggered via the button click.

18. **FR-3.6**: While validation is running, the system must display a loading spinner on the "Run Validation" button and disable the button until validation completes.

19. **FR-3.7**: Each result item in the right panel must display:
    - `cst_file` - the XML file name
    - `frag_idx` - the fragment index
    - A descriptive message explaining the issue
    - An "Open" button to navigate to that fragment in the UI

20. **FR-3.8**: Clicking the "Open" button must close the modal and navigate to the specified fragment (select the file and fragment in the UI).

21. **FR-3.9**: The validation modal must reset to its default state when closed (no persistence of selected check type).

### 4.4 Validation Checks

22. **FR-4.1**: The system must support a validation check registry that allows defining multiple check types.

23. **FR-4.2**: Each validation check definition must include:
    - `id`: Unique identifier for the check
    - `name`: Display name for the check
    - `description`: Explanation of what the check validates
    - `auto_fix_handler`: (Optional) Rocket API endpoint for auto-fixing errors

24. **FR-4.3**: The validation API must return a JSON response with results grouped by check ID. Each check result includes an `auto_fixes` array that is populated during validation when fixes are available:
    ```json
    {
      "missing_sc_code": {
        "name": "Missing SC Code",
        "description": "Sutta fragments without sc_code",
        "auto_fixable": false,
        "errors": [
          {
            "cst_file": "s0101m.mul.xml",
            "frag_idx": 5,
            "fragment_id": 123,
            "message": "Sutta fragment missing sc_code"
          }
        ],
        "auto_fixes": []
      },
      "missing_sc_sutta": {
        "name": "Missing SC Sutta Title",
        "description": "Fragments with sc_code but missing sc_sutta",
        "auto_fixable": true,
        "errors": [
          {
            "cst_file": "s0101m.mul.xml",
            "frag_idx": 8,
            "fragment_id": 156,
            "message": "Missing sc_sutta for sc_code: dn1"
          }
        ],
        "auto_fixes": [
          {
            "fragment_id": 156,
            "sc_code": "dn1",
            "suggested_value": "Brahmajālasutta"
          }
        ]
      }
    }
    ```

### 4.5 Validation Check: Missing SC Code for Sutta Fragments

25. **FR-5.1**: Implement validation check `missing_sc_code`:
    - Query: All fragments where `frag_type = 'Sutta'` AND `frag_review != 'moved'` AND (`sc_code IS NULL` OR `sc_code = ''`)
    - This check is NOT auto-fixable

26. **FR-5.2**: The error message must indicate: "Sutta fragment missing sc_code"

### 4.6 Validation Check: Missing SC Sutta Title (Auto-Fixable)

27. **FR-6.1**: Implement validation check `missing_sc_sutta`:
    - Query: All fragments where `sc_code IS NOT NULL` AND `sc_code != ''` AND (`sc_sutta IS NULL` OR `sc_sutta = ''`)
    - This check IS auto-fixable when ArangoDB is connected

28. **FR-6.2**: During validation, when ArangoDB is connected, the system must look up the title for each missing `sc_sutta` from the cached Pali titles and populate the `auto_fixes` array in the response.

29. **FR-6.3**: The validation results panel must show an "Auto-Fix All" button when:
    - This check type is selected
    - There are entries in the `auto_fixes` array

30. **FR-6.4**: Clicking "Auto-Fix All" must display a confirmation dialog showing a **scrollable list** of all changes to be made:
    - Format: `[cst_file] frag_idx: sc_code → suggested_value`

31. **FR-6.5**: Confirming the auto-fix must:
    - Submit the `auto_fixes` array to the auto-fix handler API
    - The handler must update the `sc_sutta` field for each fragment
    - Refresh the validation results after completion

## 5. Non-Goals (Out of Scope)

1. **Configurable ArangoDB credentials**: Connection settings are hardcoded for this iteration.
2. **Multiple language title caching**: Only Pali titles are cached.
3. **Manual title refresh button**: Titles only refresh on page reload.
4. **Bulk editing from validation results**: Users can only open individual fragments.
5. **Export validation results**: No CSV/JSON export functionality.
6. **Additional validation checks beyond the two specified**: Future checks will be added in separate PRDs.
7. **Validation result persistence**: Results are not saved between sessions.

## 6. Design Considerations

### 6.1 Status Indicator UI

- Position: Next to the "Menu" button in the left panel header
- Size: Small circle indicator (12-16px diameter)
- Colors: Green (`#48c774` / Bulma `is-success`) for connected, Red (`#f14668` / Bulma `is-danger`) for disconnected
- Tooltip: Use native `title` attribute or Bulma tooltip styling

### 6.2 Validation Modal Layout

```
+-----------------------------------------------+
|  Database Validation                     [X]  |
+-----------------------------------------------+
|  [Run Validation]                             |
+-----------------------------------------------+
| Check Types        |  Results                 |
| +---------------+  |  +--------------------+  |
| | Missing SC    |  |  | s0101m.mul.xml #5  |  |
| | Code (12)   < |  |  | Missing sc_code    |  |
| +---------------+  |  | [Open]             |  |
| | Missing SC    |  |  +--------------------+  |
| | Sutta (45)    |  |  | s0101m.mul.xml #8  |  |
| +---------------+  |  | Missing sc_sutta   |  |
|                    |  | [Open]             |  |
|                    |  +--------------------+  |
|                    |  [Auto-Fix All]         |
+-----------------------------------------------+
```

### 6.3 Auto-Fix Confirmation Dialog

- Show a scrollable list (max-height with overflow) of all changes
- Each line: `cst_file | frag_idx | sc_code → title`
- Buttons: "Apply Changes" (primary), "Cancel"

## 7. Technical Considerations

### 7.1 New Rust Modules

1. **`src/web/arangodb.rs`**: ArangoDB connection management
   - `connect_to_arangodb()` - Establish connection
   - `check_connection_status()` - Health check endpoint
   - `get_pali_titles()` - Fetch titles from `names` collection

2. **`src/web/validation.rs`**: Validation check definitions and handlers
   - Validation check registry
   - Individual check implementations
   - Auto-fix handlers

### 7.2 New API Endpoints

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/api/arangodb/status` | Check ArangoDB connection status |
| GET | `/api/arangodb/pali-titles` | Get all Pali titles (uid → name mapping) |
| POST | `/api/validation/run` | Run all validation checks, returns errors and auto_fixes |
| POST | `/api/validation/auto-fix/missing-sc-sutta` | Auto-fix missing sc_sutta values |

### 7.3 Database Schema

No database migration required. The existing `sc_sutta` column in `xml_fragments` table will be used to store sutta titles.

### 7.4 Frontend Changes

1. **`index.html`**:
   - Add status indicator element next to Menu button
   - Add "Database Validation" menu item
   - Add validation modal HTML structure

2. **`app.js`**:
   - Add `window.paliTitlesCache` global variable
   - Add `checkArangoStatus()` function with 5-second interval
   - Add `fetchPaliTitles()` function
   - Add validation modal logic (run checks with loading spinner, display results, auto-fix flow)

### 7.5 Dependencies

The project already includes `arangors = "0.6.0"` and `tokio` in Cargo.toml.

## 8. Success Metrics

1. **Connection Status Accuracy**: Status indicator correctly reflects ArangoDB availability 100% of the time within the 5-second polling interval.

2. **Title Cache Completeness**: All Pali root titles from SuttaCentral are successfully cached on startup when ArangoDB is available.

3. **Validation Accuracy**: All validation checks correctly identify fragments matching their criteria with zero false positives/negatives.

4. **Auto-Fix Success Rate**: 100% of auto-fix operations successfully update the database with correct values.

5. **Navigation Accuracy**: 100% of "Open" button clicks correctly navigate to the specified fragment.

## 9. Open Questions

1. **Q**: Should the `sc_sutta` field be shown in the UI metadata form with a visual indicator when it was auto-filled from ArangoDB?
   - **Tentative Answer**: The `sc_sutta` field is already in the metadata form; no additional indicator needed.
