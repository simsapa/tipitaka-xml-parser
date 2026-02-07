# Plan 04: Extract FragmentBoundaryDetector to helpers.rs

Status: **TODO**

## Implementation Overview

**Risk Level**: 🟢 **Low** - 95% identical, SN version is a superset that handles all cases

**Scope**: Move the `FragmentBoundaryDetector` struct from all parser files to `src/parsers/helpers.rs`, using the SN version which has the most complete logic.

**Why This Is Safe**:
- Only difference is SN's extra handling for `<head rend="chapter">` = Samyutta
- SN version works correctly for all nikayas (the extra branch only triggers for SN)
- `is_sutta_start()` method is 100% identical across all files
- The struct is self-contained with no external dependencies

**Difference Analysis**:

**DN/MN/AN version** (lines 220-228):
```rust
"head" if attributes.get("rend") == Some(&"chapter".to_string()) => {
    // In DN, chapter = Sutta
    // In MN/AN, chapter = Vagga
    if self.nikaya_structure.nikaya == "digha" {
        Some((GroupType::Sutta, String::new(), None, None))
    } else {
        Some((GroupType::Vagga, String::new(), None, None))
    }
},
```

**SN version** (lines 220-231):
```rust
"head" if attributes.get("rend") == Some(&"chapter".to_string()) => {
    // In DN, chapter = Sutta
    // In SN, chapter = Samyutta  // <-- Extra comment
    // In MN/AN, chapter = Vagga
    if self.nikaya_structure.nikaya == "digha" {
        Some((GroupType::Sutta, String::new(), None, None))
    } else if self.nikaya_structure.nikaya == "samyutta" {  // <-- Extra branch
        Some((GroupType::Samyutta, String::new(), None, None))
    } else {
        Some((GroupType::Vagga, String::new(), None, None))
    }
},
```

The SN version adds an `else if` branch that only executes when `nikaya_structure.nikaya == "samyutta"`. For DN/MN/AN, this branch is skipped and execution falls through to the `else` block, producing identical behavior.

**Estimated Impact**:
- Lines removed: ~1,794 lines (138 lines × 13 files)
- Lines added: ~141 lines (in helpers.rs)
- Net reduction: ~1,653 lines

## Current State

The `FragmentBoundaryDetector` struct appears in every parser file at:
- Lines 157-294 in DN/MN/AN
- Lines 157-297 in SN (3 extra lines)

```rust
/// Fragment boundary detector
///
/// Detects boundaries between fragments based on nikaya-specific rules
/// and extracts relevant metadata.
struct FragmentBoundaryDetector<'a> {
    nikaya_structure: &'a NikayaStructure,
    cst_file: &'a str,
}

impl<'a> FragmentBoundaryDetector<'a> {
    fn new(nikaya_structure: &'a NikayaStructure, cst_file: &'a str) -> Self {
        Self { nikaya_structure, cst_file }
    }
    
    /// Check if an element marks a level boundary and extract metadata
    fn check_boundary(
        &self,
        tag_name: &str,
        attributes: &HashMap<String, String>,
    ) -> Option<(GroupType, String, Option<String>, Option<i32>)> {
        match tag_name {
            // ... various cases
            "head" if attributes.get("rend") == Some(&"chapter".to_string()) => {
                // In DN, chapter = Sutta
                // In SN, chapter = Samyutta
                // In MN/AN, chapter = Vagga
                if self.nikaya_structure.nikaya == "digha" {
                    Some((GroupType::Sutta, String::new(), None, None))
                } else if self.nikaya_structure.nikaya == "samyutta" {
                    Some((GroupType::Samyutta, String::new(), None, None))
                } else {
                    Some((GroupType::Vagga, String::new(), None, None))
                }
            },
            // ... more cases
        }
    }
    
    /// Check if this is a sutta boundary (start of actual sutta content)
    fn is_sutta_start(&self, tag_name: &str, attributes: &HashMap<String, String>) -> bool {
        // ... implementation
    }
}
```

## Implementation Steps

### Step 1: Add FragmentBoundaryDetector to helpers.rs

**File**: `src/parsers/helpers.rs`

Add after the `HierarchyTracker` definition:

```rust
/// Fragment boundary detector
///
/// Detects boundaries between fragments based on nikaya-specific rules
/// and extracts relevant metadata.
#[derive(Debug)]
pub struct FragmentBoundaryDetector<'a> {
    nikaya_structure: &'a NikayaStructure,
    cst_file: &'a str,
}

impl<'a> FragmentBoundaryDetector<'a> {
    /// Create a new fragment boundary detector
    pub fn new(nikaya_structure: &'a NikayaStructure, cst_file: &'a str) -> Self {
        Self { nikaya_structure, cst_file }
    }
    
    /// Check if an element marks a level boundary and extract metadata
    ///
    /// Returns Some((GroupType, title, id, number)) if this is a boundary element
    pub fn check_boundary(
        &self,
        tag_name: &str,
        attributes: &HashMap<String, String>,
    ) -> Option<(GroupType, String, Option<String>, Option<i32>)> {
        match tag_name {
            "p" if attributes.get("rend") == Some(&"nikaya".to_string()) => {
                Some((GroupType::Nikaya, String::new(), None, None))
            },
            "p" if attributes.get("rend") == Some(&"book".to_string()) => {
                Some((GroupType::Book, String::new(), None, None))
            },
            "div" if attributes.get("type") == Some(&"book".to_string()) => {
                let id = attributes.get("id").cloned();
                Some((GroupType::Book, String::new(), id, None))
            },
            "div" if attributes.get("type") == Some(&"samyutta".to_string()) => {
                let id = attributes.get("id").cloned();
                Some((GroupType::Samyutta, String::new(), id, None))
            },
            "div" if attributes.get("type") == Some(&"pannasaka".to_string()) => {
                let id = attributes.get("id").cloned();
                Some((GroupType::Pannasaka, String::new(), id, None))
            },
            "div" if attributes.get("type") == Some(&"vagga".to_string()) => {
                let id = attributes.get("id").cloned();
                Some((GroupType::Vagga, String::new(), id, None))
            },
            "div" if attributes.get("type") == Some(&"sutta".to_string()) => {
                let id = attributes.get("id").cloned();
                Some((GroupType::Sutta, String::new(), id, None))
            },
            "head" if attributes.get("rend") == Some(&"book".to_string()) => {
                Some((GroupType::Book, String::new(), None, None))
            },
            "head" if attributes.get("rend") == Some(&"nikaya".to_string()) => {
                Some((GroupType::Nikaya, String::new(), None, None))
            },
            "head" if attributes.get("rend") == Some(&"title".to_string()) => {
                // In AN, <head rend="title"> = Pannasaka title
                if self.nikaya_structure.nikaya == "anguttara" {
                    Some((GroupType::Pannasaka, String::new(), None, None))
                } else {
                    None
                }
            },
            "head" if attributes.get("rend") == Some(&"chapter".to_string()) => {
                // In DN, chapter = Sutta
                // In SN, chapter = Samyutta
                // In MN/AN, chapter = Vagga
                if self.nikaya_structure.nikaya == "digha" {
                    Some((GroupType::Sutta, String::new(), None, None))
                } else if self.nikaya_structure.nikaya == "samyutta" {
                    Some((GroupType::Samyutta, String::new(), None, None))
                } else {
                    Some((GroupType::Vagga, String::new(), None, None))
                }
            },
            "p" if attributes.get("rend") == Some(&"title".to_string()) => {
                // In SN, <p rend="title"> = Vagga title
                // In AN (commentary/tika), <p rend="title"> = Pannasaka title
                if self.nikaya_structure.nikaya == "samyutta" {
                    Some((GroupType::Vagga, String::new(), None, None))
                } else if self.nikaya_structure.nikaya == "anguttara" {
                    Some((GroupType::Pannasaka, String::new(), None, None))
                } else {
                    None
                }
            },
            "p" if attributes.get("rend") == Some(&"chapter".to_string()) => {
                // In AN (commentary/tika), <p rend="chapter"> = Vagga title
                if self.nikaya_structure.nikaya == "anguttara" {
                    Some((GroupType::Vagga, String::new(), None, None))
                } else {
                    None
                }
            },
            "p" if attributes.get("rend") == Some(&"subhead".to_string()) => {
                // In MN, SN, and AN, subhead = Sutta title
                if self.nikaya_structure.nikaya == "majjhima" || 
                   self.nikaya_structure.nikaya == "samyutta" ||
                   self.nikaya_structure.nikaya == "anguttara" {
                    Some((GroupType::Sutta, String::new(), None, None))
                } else {
                    None
                }
            },
            _ => None,
        }
    }
    
    /// Check if this is a sutta boundary (start of actual sutta content)
    pub fn is_sutta_start(&self, tag_name: &str, attributes: &HashMap<String, String>) -> bool {
        // Check if this is a commentary or sub-commentary file
        let is_commentary = self.cst_file.ends_with(".att.xml") || self.cst_file.ends_with(".tik.xml");
        
        match self.nikaya_structure.nikaya.as_str() {
            "digha" => {
                if is_commentary {
                    // DN commentary: Use <head rend="chapter"> for sutta boundaries
                    // NOT <div type="sutta"> which marks introduction sections
                    tag_name == "head" && attributes.get("rend") == Some(&"chapter".to_string())
                } else {
                    // DN base text: Suttas are wrapped in <div type="sutta">
                    tag_name == "div" && attributes.get("type") == Some(&"sutta".to_string())
                }
            },
            "majjhima" | "samyutta" => {
                // MN/SN: Suttas are delimited by <p rend="subhead">
                // Each subhead starts a new sutta
                tag_name == "p" && attributes.get("rend") == Some(&"subhead".to_string())
            },
            "anguttara" => {
                // AN: Similar to MN/SN
                tag_name == "p" && attributes.get("rend") == Some(&"subhead".to_string())
            },
            _ => {
                // Default: look for div or subhead
                (tag_name == "div" && attributes.get("type") == Some(&"sutta".to_string())) ||
                (tag_name == "p" && attributes.get("rend") == Some(&"subhead".to_string()))
            }
        }
    }
}
```

**Note**: Changed visibility from `fn` to `pub fn` for all methods.

### Step 2: Update imports in each parser file

**Add to imports in all 13 parser files**:
```rust
use crate::parsers::helpers::{
    LineTrackingReader,
    extract_vagga_title_from_content,
    extract_first_paranum,
    apply_fragment_adjustment,
    populate_sc_fields_from_tsv_conditional,
    FragmentBoundaryDetector,  // <-- ADD THIS
};
```

### Step 3: Remove duplicate definitions

**Remove lines 157-294** (or 157-297 in SN) from each of these files:
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

### Step 6: Verify behavior (manual testing)

Run tests specifically for each nikaya to ensure boundary detection still works correctly:

```bash
cargo test --test test_digha_parsing
cargo test --test test_majjhima_parsing
cargo test --test test_samyutta_parsing
cargo test --test test_anguttara_parsing
```

## Task List

- [ ] Add `FragmentBoundaryDetector` struct to `src/parsers/helpers.rs`
- [ ] Add `FragmentBoundaryDetector` to imports in `src/parsers/digha_nikaya_mula.rs`
- [ ] Remove `FragmentBoundaryDetector` from `src/parsers/digha_nikaya_mula.rs`
- [ ] Add `FragmentBoundaryDetector` to imports in `src/parsers/digha_nikaya_atthakatha.rs`
- [ ] Remove `FragmentBoundaryDetector` from `src/parsers/digha_nikaya_atthakatha.rs`
- [ ] Add `FragmentBoundaryDetector` to imports in `src/parsers/digha_nikaya_tika.rs`
- [ ] Remove `FragmentBoundaryDetector` from `src/parsers/digha_nikaya_tika.rs`
- [ ] Add `FragmentBoundaryDetector` to imports in `src/parsers/majjhima_nikaya_mula.rs`
- [ ] Remove `FragmentBoundaryDetector` from `src/parsers/majjhima_nikaya_mula.rs`
- [ ] Add `FragmentBoundaryDetector` to imports in `src/parsers/majjhima_nikaya_atthakatha.rs`
- [ ] Remove `FragmentBoundaryDetector` from `src/parsers/majjhima_nikaya_atthakatha.rs`
- [ ] Add `FragmentBoundaryDetector` to imports in `src/parsers/majjhima_nikaya_tika.rs`
- [ ] Remove `FragmentBoundaryDetector` from `src/parsers/majjhima_nikaya_tika.rs`
- [ ] Add `FragmentBoundaryDetector` to imports in `src/parsers/samyutta_nikaya_mula.rs`
- [ ] Remove `FragmentBoundaryDetector` from `src/parsers/samyutta_nikaya_mula.rs`
- [ ] Add `FragmentBoundaryDetector` to imports in `src/parsers/samyutta_nikaya_atthakatha.rs`
- [ ] Remove `FragmentBoundaryDetector` from `src/parsers/samyutta_nikaya_atthakatha.rs`
- [ ] Add `FragmentBoundaryDetector` to imports in `src/parsers/samyutta_nikaya_tika.rs`
- [ ] Remove `FragmentBoundaryDetector` from `src/parsers/samyutta_nikaya_tika.rs`
- [ ] Add `FragmentBoundaryDetector` to imports in `src/parsers/anguttara_nikaya_mula.rs`
- [ ] Remove `FragmentBoundaryDetector` from `src/parsers/anguttara_nikaya_mula.rs`
- [ ] Add `FragmentBoundaryDetector` to imports in `src/parsers/anguttara_nikaya_atthakatha.rs`
- [ ] Remove `FragmentBoundaryDetector` from `src/parsers/anguttara_nikaya_atthakatha.rs`
- [ ] Add `FragmentBoundaryDetector` to imports in `src/parsers/anguttara_nikaya_tika.rs`
- [ ] Remove `FragmentBoundaryDetector` from `src/parsers/anguttara_nikaya_tika.rs`
- [ ] Add `FragmentBoundaryDetector` to imports in `src/parsers/general.rs`
- [ ] Remove `FragmentBoundaryDetector` from `src/parsers/general.rs`
- [ ] Run `cargo check` to verify compilation
- [ ] Run `cargo test` to verify all tests pass
- [ ] Run nikaya-specific tests to verify boundary detection behavior

## Rollback Plan

If issues are encountered:
1. Revert changes to helpers.rs
2. Restore the original `FragmentBoundaryDetector` definitions in each parser file
3. Remove `FragmentBoundaryDetector` from imports

## Verification Checklist

- [ ] All 13 parser files compile successfully
- [ ] All tests pass
- [ ] Boundary detection works correctly for DN
- [ ] Boundary detection works correctly for MN
- [ ] Boundary detection works correctly for SN
- [ ] Boundary detection works correctly for AN
- [ ] No behavioral changes observed

## Why This Won't Break Anything

The SN version of `check_boundary()` includes this extra logic:

```rust
} else if self.nikaya_structure.nikaya == "samyutta" {
    Some((GroupType::Samyutta, String::new(), None, None))
```

For DN/MN/AN parsers:
- `nikaya_structure.nikaya` is "digha", "majjhima", or "anguttara"
- The condition `self.nikaya_structure.nikaya == "samyutta"` is `false`
- Execution falls through to the `else` block
- Result: identical behavior to the DN/MN/AN versions

For SN parsers:
- `nikaya_structure.nikaya` is "samyutta"
- The condition `self.nikaya_structure.nikaya == "samyutta"` is `true`
- The Samyutta branch executes
- Result: correct SN-specific behavior

This is a safe unification because the SN version is a proper superset.
