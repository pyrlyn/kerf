# T14. Local code completion: done

Result: runa [pyrlyn/runa#101](https://github.com/pyrlyn/runa/pull/101) adds `POST /v1/infill` (runa P20.1). `crates/kerf-complete` has `Context::around` (prefix tail and suffix head from a `TextBuffer`, snapped to character boundaries), `Client` (timeout, reply byte limit, completion cap, `content` shape check, no redirects or env proxy, API key redacted in `Debug`) and `Completer` (one worker thread, generation counter, `on_edit` cancels, only the newest queued request is sent). 8 unit tests and 9 tests against `httpmock`.

Commits: pyrlyn/kerf#25 (`1e5a971`).
