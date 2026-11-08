# T11. Language-server and debug-adapter binaries through ketch: done

Result: `crates/kerf-binaries` drives the `ketch` CLI. `ketch-core` is a library but unpublished, pulls SQLite, octocrab/tokio and signature stacks, and is GPL-only, which would remove Kerf's royalty-free licence option. `Installer::install` writes a one-entry `ketch.lock` for the host target and runs `ketch sync`, the only CLI path that holds a download to a caller-chosen digest; ketch refuses a mismatch before unpacking. Downloads are capped through `KETCH_MAX_DOWNLOAD_MIB`; ketch has no unpack cap, so Kerf measures the installed tree and uninstalls it when over its cap. Six integration tests run the real ketch (from `mise.toml`, also installed in CI) against a loopback stand-in for the GitHub API.

Commits: pyrlyn/kerf#18 (`d17a002`).
