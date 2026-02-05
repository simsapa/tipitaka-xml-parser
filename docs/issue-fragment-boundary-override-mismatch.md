# Issue: Fragment Boundary Override Causes Slice Panic

## Summary

When regenerating the database with checked fragment overrides, a panic occurs because boundary overrides from the reference database can result in an `end_pos` that is smaller than the `frag_start_pos`, causing an invalid slice operation.

**Error message:**
```
[33/52] Processing: "s0305t.tik.xml"
[34/52] Processing: "s0401m.mul.xml"
  → Copied 3 reviewed fragments from reference
[35/52] Processing: "s0402m1.mul.xml"

thread 'main' (2033378) panicked at src/parsers/anguttara_nikaya_mula.rs:1147:62:
begin <= end (11937 <= 5714) when slicing `<?xml version="1.0" encoding="UTF-8"?>
<?xml-stylesheet type="text/xsl" href="tipitaka-latn.xsl"?>
<TEI.2>
<teiHeader></teiHeader>
<text>
<front></front><body xml:space="preserve">
<div id="an2" n="an2" type="book">


<p rend="centre" n="0">. Namo tassa bh`[...]
```

**Context:**
- File being processed: `s0402m1.mul.xml`
- Previous file `s0401m.mul.xml` had "3 reviewed fragments from reference" copied

## Root Cause

The fundamental issue is that **fragment indices (`frag_idx`) can shift between parse runs**, but boundary overrides are keyed solely by `(cst_file, frag_idx)`.

### How Fragment Index Shifting Happens

1. During the first parse, fragments are numbered sequentially as they're detected
2. A user marks fragment N as "checked" with specific `end_line`/`end_char` values
3. On subsequent parses, due to:
   - Changes in parser logic
   - Previous boundary adjustments propagating through
   - Different content detection thresholds

   The same logical content might now be fragment N-1 or N+1

### The Failure Scenario

1. **First parse:**
   - Fragment 5 starts at byte 500, ends at byte 1000
   - User marks fragment 5 as "checked" with `end_line=50, end_char=30`

2. **Second parse (regeneration):**
   - Due to earlier boundary adjustments, fragment indices shift
   - What was content at byte 500-1000 is now fragment 4
   - Fragment 5 now starts at byte 11937 (different content)
   - The override for frag_idx=5 is applied: end_line=50, end_char=30 → byte 5714
   - **PANIC:** Cannot slice `xml_content[11937..5714]` because start > end

### Code Flow

```rust
// In anguttara_nikaya_mula.rs:1136-1147
let (end_pos, end_line, end_char) = apply_fragment_adjustment(
    xml_content,
    close_pos,      // Parser-detected end position
    close_line,
    close_char,
    cst_file,
    fragments.len(), // Current frag_idx
    overrides.checked_overrides.as_ref(),
    overrides.adjustments.as_ref(),
);

// Line 1147 - PANICS when end_pos < frag_start_pos
let content_xml = xml_content[frag_start_pos..end_pos].to_string();
```

### Propagation Effect

After applying a boundary override, the next fragment's start position is set to the adjusted end position (line 1171):

```rust
current_fragment_start = Some((end_pos, end_line, end_char));
```

This causes a **cascading effect** where one incorrect override shifts all subsequent fragment boundaries, compounding the problem.

## Affected Files

- `src/parsers/helpers.rs:229-247` - `apply_fragment_adjustment()` lacks validation
- `src/parsers/anguttara_nikaya_mula.rs:1147` - Panic location
- All 13 nikaya parsers have similar patterns

## Potential Solutions

### Option 1: Validate and Skip Invalid Overrides

Add validation in `apply_fragment_adjustment` to ensure the returned `end_pos` is greater than or equal to the current fragment's start position. If not, fall back to the parser-detected position.

**Pros:** Simple, defensive fix
**Cons:** Silently ignores potentially valuable user corrections; doesn't fix the root cause

### Option 2: Key Overrides by Content Hash

Instead of keying by `frag_idx`, use a content-based key (e.g., hash of first N characters, or start line/char from the original parse).

**Pros:** More robust against index shifting
**Cons:** Significant refactoring; may not match if content changes

### Option 3: Key Overrides by Start Position

Store the original `start_line`/`start_char` in the override and only apply if the current fragment's start position matches.

**Pros:** Direct verification that override applies to intended fragment
**Cons:** Requires schema change; won't work if start position shifts

### Option 4: Two-Pass Approach (Recommended)

1. **First pass:** Parse without boundary overrides to get "natural" fragment boundaries
2. **Second pass:** Match checked overrides to fragments by content similarity or position proximity, then apply

**Pros:** Robust against index shifting; preserves user corrections
**Cons:** More complex; requires content matching logic

### Option 5: Immediate Fix - Clamp End Position

As an immediate safety fix, clamp `end_pos` to be at least `frag_start_pos`:

```rust
let end_pos = end_pos.max(frag_start_pos);
```

**Pros:** Prevents panic
**Cons:** Produces potentially empty or incorrect fragments; doesn't fix root cause

## Recommended Approach

1. **Immediate:** Implement Option 1 (validation) to prevent panics
2. **Short-term:** Add logging when overrides are skipped to help diagnose issues
3. **Medium-term:** Implement Option 3 (key by start position) with migration support

## Related PRD Requirements

From `tasks/prd-single-file-reparse-with-checked-overrides.md`:

> **FR2.6**: The override affects both the current fragment AND updates parser state for subsequent fragments
> - Boundary overrides (`end_line`, `end_char`) affect only the current fragment

The PRD assumes boundary overrides correctly map to their intended fragments, which is not guaranteed when frag_idx shifts.

## Test Case to Reproduce

1. Parse any Anguttara Nikaya file (e.g., s0401m.mul.xml)
2. Mark a fragment as "checked" with specific end_line/end_char values
3. Regenerate the database with the current DB as reference
4. Observe panic if fragment indices have shifted

## Files to Investigate

- `src/fragment_exporter.rs:223-288` - `extract_checked_overrides()` implementation
- `src/parsers/helpers.rs:342-372` - `get_boundary_override()` implementation
- `data/fragments.sqlite3` - Check actual frag_review='checked' records for the failing file
