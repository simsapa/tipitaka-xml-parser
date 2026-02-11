# Nikaya Parser Refactoring: Execution Plan Summary

## Overview

This document summarizes the 4-phase refactoring plan for unifying the safest, most reliable code blocks across all 13 nikaya XML parser files.

## Execution Order

The plans are numbered in order of **safety and dependencies**:

1. [x] DONE **Plan 01** - Extract HierarchyTracker (🟢 Very Low Risk)
2. [x] DONE **Plan 02** - Extract extract_sutta_title_from_content (🟢 Very Low Risk)
3. [x] DONE **Plan 03** - Create XmlParser Macro (🟢 Very Low Risk)
4. [x] DONE **Plan 04** - Extract FragmentBoundaryDetector (🟢 Low Risk)

## Plan Dependencies

```
Plan 01 (HierarchyTracker) ─┬─► Can be done independently
                            │
Plan 02 (Title Extraction) ─┤  No dependencies between plans
                            │  Each is self-contained
Plan 03 (Trait Macro) ──────┤
                            │
Plan 04 (Boundary Detector)─┘
```

**All plans can be executed in parallel** since they don't depend on each other.

However, for review and testing purposes, it's recommended to do them sequentially.

## Total Impact Summary

| Plan | Lines Removed | Lines Added | Net Reduction | Risk Level |
|------|---------------|-------------|---------------|------------|
| 01 - HierarchyTracker | 1,690 | 130 | **1,560** | 🟢 Very Low |
| 02 - Title Extraction | 1,040 | 80 | **960** | 🟢 Very Low |
| 03 - Trait Macro | 390 | 35 | **355** | 🟢 Very Low |
| 04 - Boundary Detector | 1,794 | 141 | **1,653** | 🟢 Low |
| **TOTAL** | **4,914** | **386** | **4,528** | - |

**Overall code reduction**: ~4,500 lines (18% of total parser code)

## Recommended Execution Strategy

### Option A: Sequential (Recommended for First Time)

Execute plans one at a time, running full test suite after each:

1. Implement Plan 01
2. Run `cargo test`
3. Commit changes
4. Implement Plan 02
5. Run `cargo test`
6. Commit changes
7. Continue with Plans 03 and 04

**Benefits**:
- Easy to identify which change caused any issues
- Easier code review
- Lower cognitive load

### Option B: Batch (Faster, Requires Confidence)

Implement all 4 plans in a single branch:

1. Implement Plans 01-04 in order
2. Run `cargo test` once at the end
3. Single commit with all changes

**Benefits**:
- Faster execution
- Fewer CI runs
- Single review cycle

**Requirements**:
- High confidence in the changes
- Good test coverage
- Willingness to debug multiple changes if issues arise

## Pre-Implementation Checklist

Before starting any refactoring:

- [ ] Ensure all tests currently pass: `cargo test`
- [ ] Ensure code compiles without warnings: `cargo check`
- [ ] Create a backup branch: `git checkout -b backup/pre-refactoring`
- [ ] Review each plan document thoroughly
- [ ] Identify which tests cover the affected code

## Testing Strategy

### After Each Plan

Run these commands:

```bash
# Check compilation
cargo check

# Run all tests
cargo test

# Run clippy for code quality
cargo clippy
```

### After All Plans Complete

Run comprehensive tests:

```bash
# Run all tests with output
cargo test -- --nocapture

# Run specific nikaya tests if available
cargo test digha
cargo test majjhima
cargo test samyutta
cargo test anguttara

# Check for any warnings
cargo check --all-targets --all-features
```

## Rollback Strategy

If issues are discovered:

### For Sequential Execution

```bash
# Revert the most recent plan
git checkout HEAD~1

# Or revert specific files
git checkout HEAD -- src/parsers/helpers.rs
git checkout HEAD -- src/parsers/digha_nikaya_mula.rs
# ... etc
```

### For Batch Execution

```bash
# Switch to backup branch
git checkout backup/pre-refactoring

# Or revert entire branch
git reset --hard HEAD~1
```

## Code Review Checklist

When reviewing each plan's implementation:

- [ ] All visibility modifiers updated (`fn` → `pub fn`)
- [ ] All imports added correctly
- [ ] No duplicate definitions remain
- [ ] Documentation comments preserved
- [ ] No logic changes (only code movement)
- [ ] Tests still pass
- [ ] No compiler warnings

## Future Refactoring Opportunities

After completing these 4 safe plans, consider the remaining opportunities from the main analysis:

### Medium Risk (Next Phase)
- **DONE** **derive_cst_fields**: Requires configuration for SN's `.rev()` iteration
- **DONE** **derive_cst_code**: Requires configuration for SN-specific logic

### High Risk (Careful Analysis Required)
- **parse_into_fragments**: Complex function with significant variations between nikayas
  - SN clears `current_frag_type` for Samyutta boundaries
  - AN uses `should_close` logic vs `has_sutta_content` in others
  - Requires careful abstraction design

## Success Criteria

The refactoring is successful when:

1. ✅ All 13 parser files compile without errors
2. ✅ All tests pass
3. ✅ No behavioral changes (identical output)
4. ✅ ~4,500 lines of code removed
5. ✅ Code is more maintainable
6. ✅ Future parsers can reuse the extracted components

## Questions to Address

Before starting implementation, consider:

1. **Should we keep the original files as backups temporarily?**
   - Recommendation: Use git branches for backup

2. **What if we discover the code isn't as identical as we thought?**
   - Recommendation: Abort and update the plan document with findings

3. **Should we add more tests before refactoring?**
   - Recommendation: If test coverage is low, add tests first

4. **How do we verify no behavioral changes?**
   - Recommendation: Run parsers on sample data before/after and compare outputs

## Next Steps

1. Review all 4 plan documents
2. Decide on execution strategy (sequential vs batch)
3. Create backup branch
4. Start with Plan 01
5. Document any issues or deviations encountered
6. Celebrate when all 4 plans are complete!

## Document Locations

- Plan 01: `tasks/01-hierarchy-tracker.md`
- Plan 02: `tasks/02-title-extraction.md`
- Plan 03: `tasks/03-xml-parser-macro.md`
- Plan 04: `tasks/04-boundary-detector.md`
- Full Analysis: `tasks/nikaya-parser-refactoring.md`

## Contact

If issues arise during implementation, refer to:
- The specific plan document for detailed instructions
- The full analysis document for context
- The original source files for reference
