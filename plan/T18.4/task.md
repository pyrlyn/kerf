# T18.4. Plugins and permissions model

`kerf-plugin-manager`: the plugins list (enabled, disabled for the session after 3 failures, digest mismatch against `kerf-plugins.lock`, project plugins waiting for a per-repository enable) and the permission prompt (grant lines verbatim, digest, wider capabilities asking again), on top of `kerf-plugin` and `kerf-plugin-lock`. Done when tests cover each state and each grant decision; the same through `kerf_plugins_*`.
