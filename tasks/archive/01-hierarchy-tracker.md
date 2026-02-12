# Plan 01: Extract HierarchyTracker to helpers.rs

Status: **DONE**

## Implementation Overview

**Risk Level**: 🟢 **Very Low** - 100% identical across all files

**Scope**: Move the `HierarchyTracker` struct and its implementation from all 13 parser files to `src/parsers/helpers.rs`.

**Why This Is Safe**:
- Confirmed 100% identical across DN, MN, SN, AN, and all text types (mula, atthakatha, tika)
- Pure logic with no nikaya-specific behavior
- Self-contained with no external dependencies beyond standard types

**Estimated Impact**:
- Lines removed: ~1,690 lines (130 lines × 13 files)
- Lines added: ~130 lines (in helpers.rs)
- Net reduction: ~1,560 lines

## Current State

The `HierarchyTracker` struct appears in every parser file at lines 26-155:

```rust
/// Hierarchy tracker for maintaining group level context
///
/// Tracks the current position in the nikaya hierarchy and manages
/// entering/exiting levels according to the nikaya structure.
struct HierarchyTracker {
    current_levels: Vec<GroupLevel>,
    nikaya_structure: NikayaStructure,
}

impl HierarchyTracker {
    /// Create a new hierarchy tracker
    fn new(nikaya_structure: NikayaStructure) -> Self {
        Self {
            current_levels: Vec::new(),
            nikaya_structure,
        }
    }
    
    /// Enter a new hierarchy level
    fn enter_level(
        &mut self,
        level_type: GroupType,
        title: String,
        id: Option<String>,
        number: Option<i32>,
    ) { ... }
    
    /// Get a clone of the current hierarchy levels
    fn get_current_levels(&self) -> Vec<GroupLevel> {
        self.current_levels.clone()
    }
}
```

## Implementation Steps

### Step 1: Add HierarchyTracker to helpers.rs

**File**: `src/parsers/helpers.rs`

Add at the end of the file (after the existing code):

```rust
/// Hierarchy tracker for maintaining group level context
///
/// Tracks the current position in the nikaya hierarchy and manages
/// entering/exiting levels according to the nikaya structure.
#[derive(Debug)]
pub struct HierarchyTracker {
    current_levels: Vec<GroupLevel>,
    nikaya_structure: NikayaStructure,
}

impl HierarchyTracker {
    /// Create a new hierarchy tracker
    pub fn new(nikaya_structure: NikayaStructure) -> Self {
        Self {
            current_levels: Vec::new(),
            nikaya_structure,
        }
    }
    
    /// Enter a new hierarchy level
    ///
    /// Determines the depth of the level type in the nikaya structure,
    /// truncates current_levels to the appropriate depth, and adds the new level.
    /// If a level of the same type exists at that depth, it updates the title but preserves the ID.
    pub fn enter_level(
        &mut self,
        level_type: GroupType,
        title: String,
        id: Option<String>,
        number: Option<i32>,
    ) {
        // Find the depth of this level type in the nikaya structure
        let depth = self.nikaya_structure.levels
            .iter()
            .position(|t| matches!((t, &level_type), 
                (GroupType::Nikaya, GroupType::Nikaya) |
                (GroupType::Book, GroupType::Book) |
                (GroupType::Pannasaka, GroupType::Pannasaka) |
                (GroupType::Vagga, GroupType::Vagga) |
                (GroupType::Samyutta, GroupType::Samyutta) |
                (GroupType::Sutta, GroupType::Sutta)
            ));
        
        if let Some(depth) = depth {
            // Special case: If we're entering a Nikaya level (depth 0) and we already have
            // levels (like Book), this means the XML has the nikaya tag INSIDE the book div.
            // In this case, we should insert the Nikaya at the beginning rather than truncating.
            if depth == 0 && matches!(level_type, GroupType::Nikaya) && !self.current_levels.is_empty() {
                // Check if we already have a Nikaya level
                if self.current_levels.first().map(|l| matches!(l.group_type, GroupType::Nikaya)).unwrap_or(false) {
                    // Update existing Nikaya level
                    self.current_levels[0] = GroupLevel {
                        group_type: level_type,
                        group_number: number,
                        title,
                        id,
                    };
                } else {
                    // Insert Nikaya at the beginning
                    self.current_levels.insert(0, GroupLevel {
                        group_type: level_type,
                        group_number: number,
                        title,
                        id,
                    });
                }
                return;
            }
            
            // Check if we already have a level at this depth with the same type
            if self.current_levels.len() > depth {
                let existing = &self.current_levels[depth];
                // Check if same type
                let same_type = match (&existing.group_type, &level_type) {
                    (GroupType::Nikaya, GroupType::Nikaya) |
                    (GroupType::Book, GroupType::Book) |
                    (GroupType::Pannasaka, GroupType::Pannasaka) |
                    (GroupType::Vagga, GroupType::Vagga) |
                    (GroupType::Samyutta, GroupType::Samyutta) |
                    (GroupType::Sutta, GroupType::Sutta) => true,
                    _ => false,
                };
                
                if same_type {
                    // Update the existing level, but preserve ID if new ID is None
                    let preserved_id = if id.is_none() {
                        existing.id.clone()
                    } else {
                        id.clone()
                    };
                    
                    // Only truncate child levels if we're providing a new ID OR if the title is changing
                    let title_changed = existing.title != title;
                    let should_truncate = id.is_some() || title_changed;
                    
                    if should_truncate {
                        // Truncate levels after this one before updating
                        self.current_levels.truncate(depth + 1);
                    }
                    
                    self.current_levels[depth] = GroupLevel {
                        group_type: level_type,
                        group_number: number,
                        title,
                        id: preserved_id,
                    };
                    return;
                }
            }
            
            // Truncate to the appropriate depth (remove levels at this depth and below)
            self.current_levels.truncate(depth);
            
            // Add the new level
            self.current_levels.push(GroupLevel {
                group_type: level_type,
                group_number: number,
                title,
                id,
            });
        }
    }
    
    /// Get a clone of the current hierarchy levels
    pub fn get_current_levels(&self) -> Vec<GroupLevel> {
        self.current_levels.clone()
    }
}
```

**Note**: Changed visibility from `fn` to `pub fn` for `new`, `enter_level`, and `get_current_levels`.

### Step 2: Update imports in each parser file

For each parser file, add `HierarchyTracker` to the imports from helpers:

**Change in all 13 parser files**:
```rust
use crate::parsers::helpers::{
    LineTrackingReader,
    extract_vagga_title_from_content,
    extract_first_paranum,
    apply_fragment_adjustment,
    populate_sc_fields_from_tsv_conditional,
    HierarchyTracker,  // <-- ADD THIS
};
```

### Step 3: Remove duplicate definitions

**Remove lines 26-155** from each of these files:
- `src/parsers/digha_nikaya_mula.rs`
- `src/parsers/digha_nikaya_atthakatha.rs`
- `src/parsers/digha_nikaya_tika.rs`
- `src/parsers/majjhima_nikaya_mula.rs`
- `src/parsers/majjhima_nikaya_atthakatha.rs`
- `src/parsers/majjhima_nikaya_tika.rs`
- `src/parsers/samyutta_nikaya_mula.rs`
- `src/parsers/samyutta_nikaya_atthakatha.rs`
- `src/parsers/samyutta_nikaya_tika.rs`
- `src/parsers/anguttara_nikaya_mula.rs`
- `src/parsers/anguttara_nikaya_atthakatha.rs`
- `src/parsers/anguttara_nikaya_tika.rs`
- `src/parsers/general.rs`

### Step 4: Verify compilation

```bash
cargo check
```

### Step 5: Run tests

```bash
cargo test
```

## Task List

- [x] Add `HierarchyTracker` struct to `src/parsers/helpers.rs`
- [x] Add `HierarchyTracker` to imports in `src/parsers/digha_nikaya_mula.rs`
- [x] Remove `HierarchyTracker` from `src/parsers/digha_nikaya_mula.rs`
- [x] Add `HierarchyTracker` to imports in `src/parsers/digha_nikaya_atthakatha.rs`
- [x] Remove `HierarchyTracker` from `src/parsers/digha_nikaya_atthakatha.rs`
- [x] Add `HierarchyTracker` to imports in `src/parsers/digha_nikaya_tika.rs`
- [x] Remove `HierarchyTracker` from `src/parsers/digha_nikaya_tika.rs`
- [x] Add `HierarchyTracker` to imports in `src/parsers/majjhima_nikaya_mula.rs`
- [x] Remove `HierarchyTracker` from `src/parsers/majjhima_nikaya_mula.rs`
- [x] Add `HierarchyTracker` to imports in `src/parsers/majjhima_nikaya_atthakatha.rs`
- [x] Remove `HierarchyTracker` from `src/parsers/majjhima_nikaya_atthakatha.rs`
- [x] Add `HierarchyTracker` to imports in `src/parsers/majjhima_nikaya_tika.rs`
- [x] Remove `HierarchyTracker` from `src/parsers/majjhima_nikaya_tika.rs`
- [x] Add `HierarchyTracker` to imports in `src/parsers/samyutta_nikaya_mula.rs`
- [x] Remove `HierarchyTracker` from `src/parsers/samyutta_nikaya_mula.rs`
- [x] Add `HierarchyTracker` to imports in `src/parsers/samyutta_nikaya_atthakatha.rs`
- [x] Remove `HierarchyTracker` from `src/parsers/samyutta_nikaya_atthakatha.rs`
- [x] Add `HierarchyTracker` to imports in `src/parsers/samyutta_nikaya_tika.rs`
- [x] Remove `HierarchyTracker` from `src/parsers/samyutta_nikaya_tika.rs`
- [x] Add `HierarchyTracker` to imports in `src/parsers/anguttara_nikaya_mula.rs`
- [x] Remove `HierarchyTracker` from `src/parsers/anguttara_nikaya_mula.rs`
- [x] Add `HierarchyTracker` to imports in `src/parsers/anguttara_nikaya_atthakatha.rs`
- [x] Remove `HierarchyTracker` from `src/parsers/anguttara_nikaya_atthakatha.rs`
- [x] Add `HierarchyTracker` to imports in `src/parsers/anguttara_nikaya_tika.rs`
- [x] Remove `HierarchyTracker` from `src/parsers/anguttara_nikaya_tika.rs`
- [x] Add `HierarchyTracker` to imports in `src/parsers/general.rs`
- [x] Remove `HierarchyTracker` from `src/parsers/general.rs`
- [x] Run `cargo check` to verify compilation
- [x] Run `cargo test` to verify all tests pass

## Rollback Plan

If issues are encountered:
1. Revert changes to helpers.rs
2. Restore the original `HierarchyTracker` definitions in each parser file
3. Remove `HierarchyTracker` from imports

## Verification Checklist

- [x] All 13 parser files compile successfully
- [x] All tests pass
- [x] No functionality changes (behavior should be identical)
