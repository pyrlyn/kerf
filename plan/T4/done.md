# T4. IDE permissions in `cox-plugin-api`: done

MiMo's [pyrlyn/cox#367](https://github.com/pyrlyn/cox/pull/367) had already landed both subtasks in cox with names that differ from the first draft of `docs/architecture.md` §4: `[capabilities.ide]` with `ide.*` grant lines, and the LSP/DAP tables keyed by `languages` (plus `globs`, `env`, `config`). Those names were kept, so T6 needed no change. [pyrlyn/cox#385](https://github.com/pyrlyn/cox/pull/385) (cox T380) tightened least privilege: `ide.buffer` is `"read"` | `"edit"` and `ide.git` is `"read"` | `"write"` (stage and commit only), with grant lines `ide.buffer.read`, `ide.buffer.edit`, `ide.git.read`, `ide.git.write`; a language server needs `languages` or `globs` and a debug adapter needs `languages`. `docs/architecture.md` §3.4, §4, §7 and §8 decisions 6, 8 and 9 now describe what cox has.

Tests (in cox): `ide_caps_parse_per_family_and_validate`, `ide_buffer_and_git_refuse_a_flag_or_an_unknown_level`, `language_server_and_debug_adapter_need_something_to_route_on`, `widening_ide_buffer_or_git_needs_approval`, plus #367's table, schema and API-index drift tests; `cargo test -p cox-plugin-api -p cox-plugin`, workspace clippy `-D warnings` and `fmt --check` green.

Closed through its subtasks: T4.1, T4.2.
