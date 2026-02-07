# Plan 03: Create Macro for XmlParser Trait Implementations

## Implementation Overview

**Risk Level**: 🟢 **Very Low** - Identical pattern across all files

**Scope**: Create a declarative macro `impl_xml_parser!` in helpers.rs that generates the `impl XmlParser` block, then replace all 13 manual implementations with macro invocations.

**Why This Is Safe**:
- All 13 implementations follow the exact same pattern
- Only the struct name differs
- The macro expansion is compile-time verified
- No runtime behavior changes

**Estimated Impact**:
- Lines removed: ~390 lines (30 lines × 13 files)
- Lines added: ~35 lines (macro definition in helpers.rs)
- Net reduction: ~355 lines
- Additional benefit: Future parsers can use the same macro

## Current State

Every parser file has this identical implementation pattern:

```rust
impl XmlParser for DighaNikayaMula {  // struct name varies
    fn parse_into_fragments(
        &self,
        xml_content: &str,
        nikaya_structure: &NikayaStructure,
        cst_file: &str,
        overrides: &ParserOverrides,
        populate_sc_fields: bool,
    ) -> Result<Vec<XmlFragment>> {
        // Delegate to the public function
        parse_into_fragments(xml_content, nikaya_structure, cst_file, overrides, populate_sc_fields)
    }
}
```

The only variation is the struct name (`DighaNikayaMula`, `MajjhimaNikayaMula`, etc.)

## Implementation Steps

### Step 1: Create impl_xml_parser! macro in helpers.rs

**File**: `src/parsers/helpers.rs`

Add at the end of the file (after all existing code):

```rust
/// Macro to implement the XmlParser trait for a nikaya parser struct
///
/// This macro generates a standard XmlParser implementation that delegates
/// to the shared `parse_into_fragments` function. All nikaya-specific parsers
/// use this same pattern.
///
/// # Usage
/// ```ignore
/// impl_xml_parser!(DighaNikayaMula);
/// impl_xml_parser!(MajjhimaNikayaMula);
/// ```
#[macro_export]
macro_rules! impl_xml_parser {
    ($struct_name:ident) => {
        impl XmlParser for $struct_name {
            fn parse_into_fragments(
                &self,
                xml_content: &str,
                nikaya_structure: &NikayaStructure,
                cst_file: &str,
                overrides: &ParserOverrides,
                populate_sc_fields: bool,
            ) -> Result<Vec<XmlFragment>> {
                // Delegate to the public function
                parse_into_fragments(
                    xml_content,
                    nikaya_structure,
                    cst_file,
                    overrides,
                    populate_sc_fields,
                )
            }
        }
    };
}

/// Re-export the macro for convenience
pub use impl_xml_parser;
```

### Step 2: Update imports in each parser file

Add the macro import to each parser file:

**Add to imports in all 13 parser files**:
```rust
use crate::parsers::helpers::{
    LineTrackingReader,
    extract_vagga_title_from_content,
    extract_first_paranum,
    apply_fragment_adjustment,
    populate_sc_fields_from_tsv_conditional,
    impl_xml_parser,  // <-- ADD THIS
};
```

### Step 3: Replace manual implementations with macro

**In each parser file, replace** (approximately lines 1372-1390):

```rust
impl XmlParser for DighaNikayaMula {
    fn parse_into_fragments(
        &self,
        xml_content: &str,
        nikaya_structure: &NikayaStructure,
        cst_file: &str,
        overrides: &ParserOverrides,
        populate_sc_fields: bool,
    ) -> Result<Vec<XmlFragment>> {
        // Delegate to the public function
        parse_into_fragments(xml_content, nikaya_structure, cst_file, overrides, populate_sc_fields)
    }
}
```

**With**:

```rust
impl_xml_parser!(DighaNikayaMula);
```

### Step 4: Apply to all parser files

Repeat Step 3 for each parser with the appropriate struct name:

**Files to modify**:

1. `src/parsers/digha_nikaya_mula.rs`
   - Replace with: `impl_xml_parser!(DighaNikayaMula);`

2. `src/parsers/digha_nikaya_atthakatha.rs`
   - Replace with: `impl_xml_parser!(DighaNikayaAtthakatha);`

3. `src/parsers/digha_nikaya_tika.rs`
   - Replace with: `impl_xml_parser!(DighaNikayaTika);`

4. `src/parsers/majjhima_nikaya_mula.rs`
   - Replace with: `impl_xml_parser!(MajjhimaNikayaMula);`

5. `src/parsers/majjhima_nikaya_atthakatha.rs`
   - Replace with: `impl_xml_parser!(MajjhimaNikayaAtthakatha);`

6. `src/parsers/majjhima_nikaya_tika.rs`
   - Replace with: `impl_xml_parser!(MajjhimaNikayaTika);`

7. `src/parsers/samyutta_nikaya_mula.rs`
   - Replace with: `impl_xml_parser!(SamyuttaNikayaMula);`

8. `src/parsers/samyutta_nikaya_atthakatha.rs`
   - Replace with: `impl_xml_parser!(SamyuttaNikayaAtthakatha);`

9. `src/parsers/samyutta_nikaya_tika.rs`
   - Replace with: `impl_xml_parser!(SamyuttaNikayaTika);`

10. `src/parsers/anguttara_nikaya_mula.rs`
    - Replace with: `impl_xml_parser!(AnguttaraNikayaMula);`

11. `src/parsers/anguttara_nikaya_atthakatha.rs`
    - Replace with: `impl_xml_parser!(AnguttaraNikayaAtthakatha);`

12. `src/parsers/anguttara_nikaya_tika.rs`
    - Replace with: `impl_xml_parser!(AnguttaraNikayaTika);`

13. `src/parsers/general.rs`
    - Replace with: `impl_xml_parser!(GeneralParser);`

### Step 5: Verify compilation

```bash
cargo check
```

The macro expansion will be checked at compile time.

### Step 6: Run tests

```bash
cargo test
```

## Task List

- [ ] Add `impl_xml_parser!` macro definition to `src/parsers/helpers.rs`
- [ ] Add `impl_xml_parser` to imports in `src/parsers/digha_nikaya_mula.rs`
- [ ] Replace manual impl with macro in `src/parsers/digha_nikaya_mula.rs`
- [ ] Add `impl_xml_parser` to imports in `src/parsers/digha_nikaya_atthakatha.rs`
- [ ] Replace manual impl with macro in `src/parsers/digha_nikaya_atthakatha.rs`
- [ ] Add `impl_xml_parser` to imports in `src/parsers/digha_nikaya_tika.rs`
- [ ] Replace manual impl with macro in `src/parsers/digha_nikaya_tika.rs`
- [ ] Add `impl_xml_parser` to imports in `src/parsers/majjhima_nikaya_mula.rs`
- [ ] Replace manual impl with macro in `src/parsers/majjhima_nikaya_mula.rs`
- [ ] Add `impl_xml_parser` to imports in `src/parsers/majjhima_nikaya_atthakatha.rs`
- [ ] Replace manual impl with macro in `src/parsers/majjhima_nikaya_atthakatha.rs`
- [ ] Add `impl_xml_parser` to imports in `src/parsers/majjhima_nikaya_tika.rs`
- [ ] Replace manual impl with macro in `src/parsers/majjhima_nikaya_tika.rs`
- [ ] Add `impl_xml_parser` to imports in `src/parsers/samyutta_nikaya_mula.rs`
- [ ] Replace manual impl with macro in `src/parsers/samyutta_nikaya_mula.rs`
- [ ] Add `impl_xml_parser` to imports in `src/parsers/samyutta_nikaya_atthakatha.rs`
- [ ] Replace manual impl with macro in `src/parsers/samyutta_nikaya_atthakatha.rs`
- [ ] Add `impl_xml_parser` to imports in `src/parsers/samyutta_nikaya_tika.rs`
- [ ] Replace manual impl with macro in `src/parsers/samyutta_nikaya_tika.rs`
- [ ] Add `impl_xml_parser` to imports in `src/parsers/anguttara_nikaya_mula.rs`
- [ ] Replace manual impl with macro in `src/parsers/anguttara_nikaya_mula.rs`
- [ ] Add `impl_xml_parser` to imports in `src/parsers/anguttara_nikaya_atthakatha.rs`
- [ ] Replace manual impl with macro in `src/parsers/anguttara_nikaya_atthakatha.rs`
- [ ] Add `impl_xml_parser` to imports in `src/parsers/anguttara_nikaya_tika.rs`
- [ ] Replace manual impl with macro in `src/parsers/anguttara_nikaya_tika.rs`
- [ ] Add `impl_xml_parser` to imports in `src/parsers/general.rs`
- [ ] Replace manual impl with macro in `src/parsers/general.rs`
- [ ] Run `cargo check` to verify compilation
- [ ] Run `cargo test` to verify all tests pass

## Rollback Plan

If issues are encountered:
1. Remove the macro definition from helpers.rs
2. Restore the original manual `impl XmlParser` blocks in each parser file
3. Remove `impl_xml_parser` from imports

## Verification Checklist

- [ ] All 13 parser files compile successfully
- [ ] All tests pass
- [ ] Macro expands to identical code as before
- [ ] No manual XmlParser implementations remain

## Benefits

1. **Reduced code duplication**: ~355 lines removed
2. **Consistency**: All parsers use identical implementation
3. **Maintainability**: Changes to the trait implementation only need to be made in one place
4. **Type safety**: Compile-time macro expansion ensures correctness
5. **Future-proof**: New parsers can easily use the same macro

## Notes

- The macro uses `$crate::` to ensure proper path resolution
- The `#[macro_export]` attribute makes the macro available at the crate root
- The `pub use impl_xml_parser;` re-export allows importing via `helpers::impl_xml_parser`
