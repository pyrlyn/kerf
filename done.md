# Done

Nothing closed yet.

### T5. Return an error for the largest invalid line index

TextBuffer::line must reject every out-of-range index without panicking or wrapping its diagnostic range.

Execution plan: add a usize::MAX regression in buffer.rs; use the standard saturating integer operation for the diagnostic end; run the crate tests, clippy and formatting. Existing range checks remain authoritative; no shared helper or dependency is needed for a single integer operation.

Validation: the regression panicked on overflow before the fix. All 44 tests pass under nextest; clippy with -D warnings and formatting checks pass. Reused the standard usize::saturating_add operation and preserved ordinary diagnostic ranges.
