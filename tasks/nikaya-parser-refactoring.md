# Nikaya XML Parser Refactoring Analysis

## Executive Summary

After detailed comparison of all 13 nikaya parser files, I've identified that:

1. **DN, MN, and AN parsers are 99%+ identical** - only struct names and minor whitespace differ
2. **SN parser has unique handling** for Samyutta-level structures and uses `.rev()` iteration
3. **Within each nikaya, mula/atthakatha/tika parsers are identical** - only struct names differ
4. **The differences between files are primarily in `derive_cst_fields`, `derive_cst_code`, and `parse_into_fragments`**

**Total Lines of Code**: ~25,000 lines across 14 files
**Estimated Refactoring Savings**: ~15,000-20,000 lines (60-80% reduction)

---

## Refactoring Strategy Overview

### Approach
Instead of blindly extracting everything, we need a **nuanced approach** that:
- Extracts truly identical code blocks to `helpers.rs`
- Preserves nikaya-specific edge case handling
- Uses composition or configuration rather than duplication for variations

### Files Grouping

#### Group 1: Truly Identical (can be unified)
- `digha_nikaya_mula.rs`
- `digha_nikaya_atthakatha.rs`
- `digha_nikaya_tika.rs`
- `majjhima_nikaya_mula.rs`
- `majjhima_nikaya_atthakatha.rs`
- `majjhima_nikaya_tika.rs`
- `anguttara_nikaya_mula.rs`
- `anguttara_nikaya_atthakatha.rs`
- `anguttara_nikaya_tika.rs`

#### Group 2: Requires Special Handling (SN-specific logic)
- `samyutta_nikaya_mula.rs`
- `samyutta_nikaya_atthakatha.rs`
- `samyutta_nikaya_tika.rs`

**Note**: The SN atthakatha/tika files appear to have removed some SN-specific logic and are closer to Group 1 behavior.

---

## Detailed Refactoring Cases

### Case 1: HierarchyTracker Struct (SAFE TO EXTRACT)

**Status**: ✅ **100% identical across all files**

**Location in files**:
- Lines 26-155 in all files

**Current code** (identical in all 13 files):
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
    ) {
        // ... implementation
    }
    
    /// Get a clone of the current hierarchy levels
    fn get_current_levels(&self) -> Vec<GroupLevel> {
        self.current_levels.clone()
    }
}
```

**Refactoring plan**:
1. Move to `helpers.rs` as `pub struct HierarchyTracker`
2. Remove from all 13 parser files
3. Update imports

**Files to modify**:
- `src/parsers/helpers.rs` (add)
- `src/parsers/digha_nikaya_mula.rs` (remove)
- `src/parsers/digha_nikaya_atthakatha.rs` (remove)
- `src/parsers/digha_nikaya_tika.rs` (remove)
- `src/parsers/majjhima_nikaya_mula.rs` (remove)
- `src/parsers/majjhima_nikaya_atthakatha.rs` (remove)
- `src/parsers/majjhima_nikaya_tika.rs` (remove)
- `src/parsers/samyutta_nikaya_mula.rs` (remove)
- `src/parsers/samyutta_nikaya_atthakatha.rs` (remove)
- `src/parsers/samyutta_nikaya_tika.rs` (remove)
- `src/parsers/anguttara_nikaya_mula.rs` (remove)
- `src/parsers/anguttara_nikaya_atthakatha.rs` (remove)
- `src/parsers/anguttara_nikaya_tika.rs` (remove)
- `src/parsers/general.rs` (remove)

**Task list**:
- [ ] Move `HierarchyTracker` struct and impl to `src/parsers/helpers.rs`
- [ ] Make struct and methods `pub`
- [ ] Update `pub use` statements in `src/parsers/mod.rs`
- [ ] Remove duplicate definitions from all 13 parser files
- [ ] Run tests: `cargo test`

---

### Case 2: FragmentBoundaryDetector Struct (PARTIALLY EXTRACTABLE)

**Status**: ⚠️ **95% identical with one SN-specific variation**

**Location in files**:
- Lines 157-294 in DN/MN/AN
- Lines 157-297 in SN (3 extra lines)

**Differences**:

**SN only** (lines 220-231):
```rust
"head" if attributes.get("rend") == Some(&"chapter".to_string()) => {
    // In DN, chapter = Sutta
    // In SN, chapter = Samyutta  // <-- SN-specific comment
    // In MN/AN, chapter = Vagga
    if self.nikaya_structure.nikaya == "digha" {
        Some((GroupType::Sutta, String::new(), None, None))
    } else if self.nikaya_structure.nikaya == "samyutta" {  // <-- SN-specific branch
        Some((GroupType::Samyutta, String::new(), None, None))
    } else {
        Some((GroupType::Vagga, String::new(), None, None))
    }
},
```

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

**Recommendation**: Use the SN version (with the extra `else if` branch) for all parsers, as it handles all cases correctly.

**Refactoring plan**:
1. Use the SN version of `check_boundary()` which handles all nikayas
2. Move to `helpers.rs` as `pub struct FragmentBoundaryDetector`
3. The `is_sutta_start()` method is identical in all files

**Task list**:
- [ ] Move SN version of `FragmentBoundaryDetector` to `helpers.rs`
- [ ] Make struct and methods `pub`
- [ ] Update imports in all 13 parser files
- [ ] Remove duplicate definitions from each parser file
- [ ] Verify tests pass: `cargo test`

---

### Case 3: extract_sutta_title_from_content Function (SAFE TO EXTRACT)

**Status**: ✅ **100% identical across all files**

**Location in files**:
- Lines 366-445 in DN/MN/AN
- Lines 376-455 in SN (offset by 10 lines due to extra code above)

**Current code** (identical in all files):
```rust
fn extract_sutta_title_from_content(content: &str) -> Option<String> {
    use quick_xml::Reader;
    use quick_xml::events::Event;
    
    let mut reader = Reader::from_str(content);
    reader.trim_text(false);
    let mut buf = Vec::new();
    
    let mut first_chapter_title: Option<String> = None;
    let mut first_subhead_title: Option<String> = None;
    
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                // ... logic to extract titles
            },
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {},
        }
        buf.clear();
    }
    
    first_subhead_title.or(first_chapter_title)
}
```

**Task list**:
- [ ] Move `extract_sutta_title_from_content` to `helpers.rs`
- [ ] Make function `pub`
- [ ] Remove from all 13 parser files
- [ ] Run tests: `cargo test`

---

### Case 4: derive_cst_fields Function (REQUIRES CONFIGURATION)

**Status**: ⚠️ **70% identical with SN-specific differences**

**Key differences between SN and others**:

**SN version** uses `.rev()` iteration:
```rust
// SN uses .rev() to get the LAST (most recent) Vagga level
let cst_vagga = fragment.group_levels.iter()
    .rev()  // <-- SN ONLY
    .find(|level| matches!(level.group_type, crate::types::GroupType::Vagga))
    // ...

// SN uses .rev() for sutta too
let cst_sutta = fragment.group_levels.iter()
    .rev()  // <-- SN ONLY
    .find(|level| matches!(level.group_type, crate::types::GroupType::Sutta))
    // ...
```

**DN/MN/AN version** uses forward iteration:
```rust
// Others use .find() which gets the FIRST match
let has_vagga_level = nikaya_structure.levels.iter()
    .any(|t| matches!(t, crate::types::GroupType::Vagga));

let cst_vagga = if has_vagga_level {
    fragment.group_levels.iter()
        .find(|level| matches!(level.group_type, crate::types::GroupType::Vagga))
        // ...
```

**Also SN has special fallback logic**:
```rust
// SN does NOT use extract_vagga_title_from_content fallback
.or_else(|| {
    // Fallback: Extract vagga title from <head rend="chapter"> tag
    // NOTE: Do NOT use this fallback for SN because in SN, <head rend="chapter"> is a Samyutta marker
    if nikaya_structure.nikaya == "majjhima" {  // <-- Only for MN
        extract_vagga_title_from_content(&fragment.content_xml)
    } else {
        None
    }
});
```

**Recommendation**: Create a configurable version that accepts a `NikayaType` parameter:

```rust
pub fn derive_cst_fields(
    fragment: &XmlFragment,
    nikaya_structure: &NikayaStructure,
    use_rev_iteration: bool,  // true for SN
    use_vagga_fallback: bool, // true for MN only
) -> (String, Option<String>, Option<String>, Option<String>, Option<String>)
```

**Task list**:
- [ ] Design configuration parameters for `derive_cst_fields`
- [ ] Create unified implementation in `helpers.rs`
- [ ] Update all parsers to use configuration-based version
- [ ] SN parser: `use_rev_iteration=true, use_vagga_fallback=false`
- [ ] MN parser: `use_rev_iteration=false, use_vagga_fallback=true`
- [ ] DN/AN parsers: `use_rev_iteration=false, use_vagga_fallback=true`
- [ ] Run tests: `cargo test`

---

### Case 5: derive_cst_code Function (REQUIRES CONFIGURATION)

**Status**: ⚠️ **60% identical with significant SN-specific logic**

**Key differences**:

**SN uses `.rev()` for level extraction** (lines 445-467, 568-594 in SN):
```rust
// SN uses .rev() to get the most recent Vagga/Sutta
let vagga_number = fragment.group_levels.iter()
    .rev()  // <-- SN ONLY
    .find_map(|level| { ... });

let sutta_number = fragment.group_levels.iter()
    .rev()  // <-- SN ONLY
    .find_map(|level| { ... });
```

**SN has special handling for suttas without vaggas**:
```rust
"samyutta" => {
    match (book_id, samyutta_number, vagga_number, sutta_number) {
        (Some(book), Some(samyutta), Some(vagga), Some(sutta)) => {
            Some(format!("{}.{}.{}.{}", book, samyutta, vagga, sutta))
        }
        // SN ONLY: Handle suttas without vaggas (Bhikkhunīsaṃyuttaṃ)
        (Some(book), Some(samyutta), None, Some(sutta)) => {
            Some(format!("{}.{}.1.{}", book, samyutta, sutta))  // use 1 as vagga
        }
        _ => None,
    }
}
```

**Recommendation**: Similar to Case 4, create a configurable version with nikaya-specific flags.

**Task list**:
- [ ] Analyze all differences between nikaya implementations
- [ ] Design configuration enum/struct for `derive_cst_code`
- [ ] Create unified implementation in `helpers.rs`
- [ ] Update all parsers with appropriate configuration
- [ ] Run tests: `cargo test`

---

### Case 6: parse_into_fragments Function (COMPLEX - REQUIRES CAREFUL ANALYSIS)

**Status**: ⚠️ **50% identical with significant structural differences**

This is the largest function (~1,300 lines) and has the most variation:

#### SN-specific differences:

**1. Fragment type clearing for Samyutta boundaries** (SN lines 807-890):
```rust
// For Samyutta boundaries, don't set fragment type yet - wait for actual sutta
if !matches!(group_type, GroupType::Samyutta) {
    current_frag_type = Some(FragmentType::Sutta);
} else {
    // For Samyutta: clear fragment type, will be set when first sutta arrives
    current_frag_type = None;  // <-- SN ONLY
}
```

**2. Different group_levels update logic** (SN lines 1021-1145):
```rust
// SN only updates group_levels after entering new level if fragment type is set
if current_frag_type.is_some() {
    current_fragment_group_levels = hierarchy.get_current_levels();
}
```

#### AN-specific differences:

**AN uses `should_close` logic** (AN lines 802-866):
```rust
// AN ONLY
let should_close = if frag_start_pos >= event_start_pos {
    true
} else {
    let tentative_content = xml_content[frag_start_pos..event_start_pos].to_string();
    tentative_content.contains("rend=\"subhead\"") ||
    tentative_content.contains("rend=\"chapter\"") ||
    tentative_content.contains("rend=\"bodytext\"")
};

if should_close {
    // ... close fragment
}
```

**DN/MN/SN use `has_sutta_content` logic**:
```rust
// DN/MN/SN
let has_sutta_content = tentative_content.contains("rend=\"subhead\"") || 
                       tentative_content.contains("rend=\"chapter\"") ||
                       tentative_content.contains("rend=\"bodytext\"");

if has_sutta_content {
    // ... close fragment
}
```

**Recommendation**: This function is too complex to easily unify. Options:

1. **Keep separate implementations** but extract shared helper functions
2. **Create a trait-based approach** with default implementations and nikaya-specific overrides
3. **Use a configuration struct** with function pointers for nikaya-specific behavior

**Task list**:
- [ ] Create detailed diff of all `parse_into_fragments` implementations
- [ ] Identify common patterns that can be extracted as helper functions
- [ ] Design abstraction layer (trait or config struct)
- [ ] Implement shared base logic in `helpers.rs`
- [ ] Refactor each nikaya parser to use shared logic + nikaya-specific overrides
- [ ] Run comprehensive tests: `cargo test`

---

### Case 7: XmlParser Trait Implementations (CAN BE GENERATED)

**Status**: ✅ **100% identical pattern across all files**

Each file has the same `impl XmlParser for X` block:

```rust
impl XmlParser for DighaNikayaMula {  // struct name varies
    fn parse(
        &self,
        xml_content: &str,
        nikaya_structure: &NikayaStructure,
        cst_file: &str,
        overrides: &ParserOverrides,
        populate_sc_fields: bool,
    ) -> Result<Vec<XmlFragment>> {
        let mut fragments = parse_into_fragments(
            xml_content,
            nikaya_structure,
            cst_file,
            overrides,
            populate_sc_fields,
        )?;
        
        // Apply SC overrides from correction fragments
        if let Some(ref correction_overrides) = overrides.correction_overrides {
            apply_sc_overrides(
                &mut fragments,
                correction_overrides,
                cst_file,
                None,
            );
        }
        
        Ok(fragments)
    }
}
```

**Recommendation**: Use a macro to generate this boilerplate:

```rust
macro_rules! impl_xml_parser {
    ($struct_name:ident) => {
        impl XmlParser for $struct_name {
            fn parse(
                &self,
                xml_content: &str,
                nikaya_structure: &NikayaStructure,
                cst_file: &str,
                overrides: &ParserOverrides,
                populate_sc_fields: bool,
            ) -> Result<Vec<XmlFragment>> {
                let mut fragments = parse_into_fragments(
                    xml_content,
                    nikaya_structure,
                    cst_file,
                    overrides,
                    populate_sc_fields,
                )?;
                
                if let Some(ref correction_overrides) = overrides.correction_overrides {
                    apply_sc_overrides(&mut fragments, correction_overrides, cst_file, None);
                }
                
                Ok(fragments)
            }
        }
    };
}

// Usage in each parser file:
impl_xml_parser!(DighaNikayaMula);
```

**Task list**:
- [ ] Create `impl_xml_parser!` macro in `helpers.rs`
- [ ] Replace all 13 manual `impl XmlParser` blocks with macro invocation
- [ ] Run tests: `cargo test`

---

## Suggested Refactoring Order

### Phase 1: Safe Extractions (100% identical)
1. **HierarchyTracker** struct - 0 risk
2. **extract_sutta_title_from_content** function - 0 risk
3. **impl_xml_parser!** macro - low risk

### Phase 2: Configuration-based Unification
4. **FragmentBoundaryDetector** - use SN version for all
5. **derive_cst_fields** - add configuration parameters
6. **derive_cst_code** - add configuration parameters

### Phase 3: Complex Refactoring
7. **parse_into_fragments** - requires careful design and testing
   - Extract common helper functions first
   - Consider trait-based abstraction
   - Maintain nikaya-specific overrides

### Phase 4: Cleanup
8. Remove duplicate code from all parser files
9. Update imports and module structure
10. Comprehensive testing

---

## Risk Assessment

| Component | Risk Level | Testing Required |
|-----------|------------|------------------|
| HierarchyTracker | 🟢 Low | Unit tests |
| FragmentBoundaryDetector | 🟢 Low | Unit tests + boundary detection tests |
| extract_sutta_title_from_content | 🟢 Low | Unit tests |
| derive_cst_fields | 🟡 Medium | Field extraction tests per nikaya |
| derive_cst_code | 🟡 Medium | Code generation tests per nikaya |
| parse_into_fragments | 🔴 High | Full integration tests, edge cases |
| impl_xml_parser macro | 🟢 Low | Compile-time check |

---

## Estimated Impact

### Before Refactoring
- **Total lines**: ~25,000
- **Files**: 14 parser files
- **Duplication**: ~80%

### After Phase 1 (Safe Extractions)
- **Lines removed**: ~5,000
- **Remaining**: ~20,000

### After Phase 2 (Configuration-based)
- **Lines removed**: ~8,000
- **Remaining**: ~12,000

### After Phase 3 (Complex Refactoring)
- **Lines removed**: ~5,000
- **Remaining**: ~7,000

### Final Result
- **Total reduction**: ~72%
- **Maintainability**: Significantly improved
- **Test coverage**: Easier to achieve
