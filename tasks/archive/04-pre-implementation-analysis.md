# Plan 04 Pre-Implementation Analysis

Status: **DONE**

## FragmentBoundaryDetector Differences Analysis

**Date**: 2026-02-07  
**Purpose**: Verify differences before extracting FragmentBoundaryDetector

## Key Findings

### 1. is_sutta_start() Method - 100% Identical ✅

The `is_sutta_start()` method is **identical** across all 13 parser files. It handles nikaya-specific logic through runtime checks:

```rust
fn is_sutta_start(&self, tag_name: &str, attributes: &HashMap<String, String>) -> bool {
    let is_commentary = self.cst_file.ends_with(".att.xml") || self.cst_file.ends_with(".tik.xml");
    
    match self.nikaya_structure.nikaya.as_str() {
        "digha" => { /* DN logic */ },
        "majjhima" | "samyutta" => { /* MN/SN logic */ },
        "anguttara" => { /* AN logic */ },
        _ => { /* default */ }
    }
}
```

**Conclusion**: Safe to extract as-is.

### 2. check_boundary() Method - 95% Identical ⚠️

**One difference found** in the `<head rend="chapter">` handling:

#### DN/MN/AN Version (lines 220-228):
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

#### SN Version (lines 220-231):
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

### 3. Why SN Version is Safe for All Parsers

**Runtime behavior analysis**:

| Nikaya | `nikaya == "samyutta"` | Branch Taken | Result |
|--------|------------------------|--------------|---------|
| DN | false | else | Vagga |
| MN | false | else | Vagga |
| SN | true | else if | Samyutta |
| AN | false | else | Vagga |

For DN/MN/AN parsers:
- The condition `self.nikaya_structure.nikaya == "samyutta"` evaluates to `false`
- Execution falls through to the `else` block
- Result: `Some((GroupType::Vagga, ...))` - **identical to original**

For SN parsers:
- The condition evaluates to `true`
- Returns `Some((GroupType::Samyutta, ...))` - **correct SN behavior**

**Conclusion**: SN version is a **proper superset** that handles all nikayas correctly.

## Implementation Strategy

### Safe Approach: Use SN Version for All Parsers ✅

1. Extract the SN version of `FragmentBoundaryDetector` to `helpers.rs`
2. Use it in all 13 parser files
3. No behavioral changes for any parser

### Why This Won't Break Anything

1. **DN/MN/AN parsers**: The extra `else if` branch is never executed
2. **SN parsers**: Get correct Samyutta handling
3. **Compile-time safety**: Same types, same signatures
4. **Test coverage**: All existing tests pass

## Code to Extract

The SN version from `src/parsers/samyutta_nikaya_mula.rs` (lines 157-297):

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
    pub fn check_boundary(
        &self,
        tag_name: &str,
        attributes: &HashMap<String, String>,
    ) -> Option<(GroupType, String, Option<String>, Option<i32>)> {
        // SN version (lines 174-261 in SN file)
        // Includes the extra Samyutta handling
    }
    
    /// Check if this is a sutta boundary
    pub fn is_sutta_start(&self, tag_name: &str, attributes: &HashMap<String, String>) -> bool {
        // Identical in all files (lines 263-294)
    }
}
```

## Risk Assessment

| Component | Risk | Mitigation |
|-----------|------|------------|
| `is_sutta_start()` | 🟢 None | 100% identical |
| `check_boundary()` | 🟢 Low | SN version is superset |
| Import changes | 🟢 None | Add to existing import block |
| Test breakage | 🟢 Low | All tests currently pass |

## Verification Plan

1. **Before implementation**:
   - ✅ Completed: Diff analysis shows SN version is safe

2. **During implementation**:
   - Add SN version to helpers.rs
   - Update imports in all 13 files
   - Remove local definitions
   - Run `cargo check`

3. **After implementation**:
   - Run `cargo test`
   - Run nikaya-specific tests
   - Verify no behavioral changes

## Expected Impact

- **Lines removed**: ~1,794 lines (138 lines × 13 files)
- **Lines added**: ~145 lines (in helpers.rs)
- **Net savings**: **~1,649 lines**

## Decision

**Proceed with implementation** using the SN version of `FragmentBoundaryDetector`. The analysis confirms:

1. ✅ SN version is a proper superset
2. ✅ No behavioral changes for DN/MN/AN
3. ✅ SN continues to work correctly
4. ✅ All tests should pass

This is the **largest and safest** refactoring of the four plans.
