//! Kerf's Rust core: text buffer, event loop and semantic layer.
//!
//! Stub so the workspace has a member and `cargo test --workspace` is green
//! before T2.1 (buffer) and T2.2 (event loop) land their real modules.

#[cfg(test)]
mod tests {
    #[test]
    fn crate_name_is_kerf_core() {
        assert_eq!(env!("CARGO_PKG_NAME"), "kerf-core");
    }
}
