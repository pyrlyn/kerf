use std::fs;
use std::path::{Path, PathBuf};

use super::{Digest, Entry, Error, FILE_NAME, Lockfile, MAX_BYTES, VerifyError};

const DIGEST_A: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
const DIGEST_B: &str = "fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210";

fn entry(version: &str, digest: &str) -> Entry {
    Entry {
        version: version.to_owned(),
        digest: Digest::parse(digest).expect("test digest"),
    }
}

fn sample() -> Lockfile {
    let mut lock = Lockfile::new();
    lock.pin("rust-tools", entry("0.3.1", DIGEST_A));
    lock.pin("acme.lint", entry("1.0.0", DIGEST_B));
    lock
}

fn lock_path(dir: &tempfile::TempDir) -> PathBuf {
    dir.path().join(FILE_NAME)
}

fn read_text(path: &Path, text: &str) -> Result<Lockfile, Error> {
    fs::write(path, text).expect("write fixture");
    Lockfile::read(path)
}

#[test]
fn a_written_lockfile_reads_back_equal() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = lock_path(&dir);
    let lock = sample();
    lock.write(&path).expect("write");
    assert_eq!(Lockfile::read(&path).expect("read"), lock);
}

#[test]
fn the_written_file_is_sorted_by_id_and_starts_with_the_header() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = lock_path(&dir);
    sample().write(&path).expect("write");
    let text = fs::read_to_string(&path).expect("read text");
    assert!(text.starts_with("# Kerf plugin lockfile."), "{text}");
    assert!(text.contains("format = 1"), "{text}");
    let first = text.find("acme.lint").expect("acme.lint entry");
    let second = text.find("rust-tools").expect("rust-tools entry");
    assert!(first < second, "{text}");
}

#[test]
fn writing_replaces_an_existing_file() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = lock_path(&dir);
    sample().write(&path).expect("first write");
    let mut lock = Lockfile::read(&path).expect("read");
    lock.unpin("acme.lint");
    lock.write(&path).expect("second write");
    let read = Lockfile::read(&path).expect("read again");
    assert!(read.get("acme.lint").is_none());
    assert_eq!(read.iter().count(), 1);
}

#[test]
fn a_hand_written_file_reads() {
    let dir = tempfile::tempdir().expect("tempdir");
    let text = format!(
        "format = 1\n\n[plugins.\"rust-tools\"]\nversion = \"0.3.1\"\ndigest = \"{DIGEST_A}\"\n"
    );
    let lock = read_text(&lock_path(&dir), &text).expect("read");
    assert_eq!(lock.get("rust-tools"), Some(&entry("0.3.1", DIGEST_A)));
}

#[test]
fn a_file_with_no_plugins_is_an_empty_lock() {
    let dir = tempfile::tempdir().expect("tempdir");
    let lock = read_text(&lock_path(&dir), "format = 1\n").expect("read");
    assert_eq!(lock, Lockfile::new());
}

#[test]
fn a_matching_package_is_allowed() {
    sample()
        .verify("rust-tools", "0.3.1", DIGEST_A)
        .expect("matching package");
}

#[test]
fn a_digest_mismatch_is_refused() {
    let err = sample()
        .verify("rust-tools", "0.3.1", DIGEST_B)
        .expect_err("changed bytes");
    assert_eq!(
        err,
        VerifyError::DigestMismatch {
            id: "rust-tools".to_owned(),
            locked: Digest::parse(DIGEST_A).expect("digest"),
            found: DIGEST_B.to_owned(),
        }
    );
}

#[test]
fn a_digest_in_another_case_is_refused() {
    let upper = DIGEST_A.to_ascii_uppercase();
    let err = sample()
        .verify("rust-tools", "0.3.1", &upper)
        .expect_err("uppercase digest");
    assert!(matches!(err, VerifyError::DigestMismatch { .. }), "{err}");
}

#[test]
fn a_version_mismatch_is_refused() {
    let err = sample()
        .verify("rust-tools", "0.4.0", DIGEST_A)
        .expect_err("other version");
    assert!(
        matches!(&err, VerifyError::VersionMismatch { locked, found, .. }
            if locked == "0.3.1" && found == "0.4.0"),
        "{err}"
    );
}

#[test]
fn a_missing_entry_is_refused() {
    let err = sample()
        .verify("unknown", "1.0.0", DIGEST_A)
        .expect_err("unlocked plugin");
    assert_eq!(
        err,
        VerifyError::NotLocked {
            id: "unknown".to_owned()
        }
    );
}

#[test]
fn bad_toml_is_malformed_and_names_the_file() {
    let dir = tempfile::tempdir().expect("tempdir");
    let err = read_text(&lock_path(&dir), "format = 1\n[plugins\n").expect_err("bad toml");
    assert!(matches!(err, Error::Malformed { .. }), "{err}");
    assert!(err.to_string().contains(FILE_NAME), "{err}");
}

#[test]
fn an_unknown_key_is_malformed() {
    let dir = tempfile::tempdir().expect("tempdir");
    let text = format!(
        "format = 1\n[plugins.a]\nversion = \"1\"\ndigest = \"{DIGEST_A}\"\nsource = \"x\"\n"
    );
    let err = read_text(&lock_path(&dir), &text).expect_err("unknown key");
    assert!(matches!(err, Error::Malformed { .. }), "{err}");
}

#[test]
fn a_duplicate_id_is_malformed() {
    let dir = tempfile::tempdir().expect("tempdir");
    let one = format!("[plugins.a]\nversion = \"1\"\ndigest = \"{DIGEST_A}\"\n");
    let text = format!("format = 1\n{one}{one}");
    let err = read_text(&lock_path(&dir), &text).expect_err("duplicate id");
    assert!(matches!(err, Error::Malformed { .. }), "{err}");
}

#[test]
fn an_unsupported_format_is_malformed() {
    let dir = tempfile::tempdir().expect("tempdir");
    let err = read_text(&lock_path(&dir), "format = 2\n").expect_err("format 2");
    assert!(err.to_string().contains("format 2"), "{err}");
}

#[test]
fn a_missing_format_is_malformed() {
    let dir = tempfile::tempdir().expect("tempdir");
    let err = read_text(&lock_path(&dir), "").expect_err("no format");
    assert!(matches!(err, Error::Malformed { .. }), "{err}");
}

#[test]
fn a_malformed_digest_is_refused() {
    let dir = tempfile::tempdir().expect("tempdir");
    for bad in [
        "abc",
        &DIGEST_A.to_ascii_uppercase(),
        &format!("{DIGEST_A}00"),
    ] {
        let text = format!("format = 1\n[plugins.a]\nversion = \"1\"\ndigest = \"{bad}\"\n");
        let err = read_text(&lock_path(&dir), &text).expect_err("bad digest");
        assert!(matches!(err, Error::Malformed { .. }), "{bad}: {err}");
    }
    assert!(Digest::parse("g".repeat(64).as_str()).is_err());
}

#[test]
fn a_file_over_the_cap_is_refused_and_one_at_it_is_read() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = lock_path(&dir);
    let cap = usize::try_from(MAX_BYTES).expect("cap fits usize");
    let head = "format = 1\n#";
    let at_cap = format!("{head}{}", "x".repeat(cap - head.len()));
    assert_eq!(read_text(&path, &at_cap).expect("at cap"), Lockfile::new());
    let err = read_text(&path, &format!("{at_cap}x")).expect_err("over cap");
    assert!(matches!(err, Error::TooLarge { .. }), "{err}");
}

#[test]
fn a_non_utf8_file_is_refused() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = lock_path(&dir);
    fs::write(&path, [b'f', 0xff, 0xfe]).expect("write");
    let err = Lockfile::read(&path).expect_err("not utf-8");
    assert!(matches!(err, Error::NotUtf8 { .. }), "{err}");
}

#[test]
fn a_missing_file_is_an_io_error() {
    let dir = tempfile::tempdir().expect("tempdir");
    let err = Lockfile::read(&lock_path(&dir)).expect_err("no file");
    assert!(
        matches!(&err, Error::Io { source, .. } if source.kind() == std::io::ErrorKind::NotFound),
        "{err}"
    );
}

#[test]
fn the_committed_schema_is_current() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs/schema/kerf-plugins.lock.schema.json");
    config_schema::check_schema::<Lockfile>(
        &path,
        Some("Generated from crates/kerf-plugin-lock. Regenerate with KERF_BLESS_SCHEMA=1 cargo test -p kerf-plugin-lock."),
        "KERF_BLESS_SCHEMA",
    )
    .expect("schema is current");
}
