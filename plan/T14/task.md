# T14. Local code completion

A fill-in-the-middle endpoint in runa, used by the editor. Chat and agents go through `cox acp`, not here. The runa side is a matching task in runa's `plan.md`; the Kerf side is a completion client with a timeout, a size cap and cancellation on the next edit. Done when Kerf's client passes tests against a fake endpoint and runa's endpoint has its own tests.

Plan:

1. runa (own repo, own worktree and PR): task P20.1 in runa's `plan.md`, a `POST /v1/infill` with llama-server's `/infill` fields (`input_prefix`, `input_suffix`, `n_predict`; reply `content`) built on the model's own FIM tokens. A model without FIM tokens gets a 400, never a chat-template imitation.
2. `crates/kerf-complete`: blocking `ureq` client (no async runtime in Kerf), run on a worker thread so the event loop only polls. Context is the prefix and suffix around the cursor read from `kerf-core`'s `TextBuffer`, each capped in bytes and snapped to char boundaries.
3. Hard caps on everything runa returns: a request timeout, a response body cap, a completion length cap, JSON shape checked. A `Completer` holds at most one request and cancels it on the next `EditEvent`; a cancelled or stale result is never delivered.
4. Tests against a fake endpoint (`httpmock`): happy path, timeout, size cap, cancel on edit, malformed reply. fmt, clippy `-D warnings`, test.
5. Crate map row in `docs/architecture.md`, `toolchain.md` rows for `ureq` and `httpmock`.
