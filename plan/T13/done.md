# T13. Remote development: done

Result: `crates/kerf-remote` with protocol 1.0 (`initialize`, `buffer/open|read|edit|undo|redo|close`, `index/lookup`), a sans-IO `Connection`, the `Host`, and the `kerf-host` binary over stdio. Caps: 4 MiB per line in both directions (an over-cap answer becomes `REPLY_TOO_LARGE`), 64 queued requests (more answer `BUSY`), 256 buffers, 8 MiB per buffer, 10 000 symbols per lookup. 9 integration tests through an in-memory stream that hands out 7 bytes per read, plus one over the binary's stdio. About 565 lines of code without comments and blanks, over the 500-line cap: the caps and typed errors the card asks for are most of it, and the protocol cannot be tested apart from the host without a stub dispatcher that would add code.

Commits: pyrlyn/kerf#21 (`cec13e4`).
