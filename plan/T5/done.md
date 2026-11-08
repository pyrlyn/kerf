# T5. Return an error for the largest invalid line index: done

Validation: the regression panicked on overflow before the fix. All 44 tests pass under nextest; clippy with -D warnings and formatting checks pass. Reused the standard usize::saturating_add operation and preserved ordinary diagnostic ranges.

Commits: pyrlyn/kerf#12 (`a07473a`).
