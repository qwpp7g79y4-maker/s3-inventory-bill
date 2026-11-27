//! `-V` and `--version` print the crate version. A debug build also prints
//! the commit of the checkout it was built from, when git reports one for this
//! very directory; a release build never prints it.

use std::path::PathBuf;
use std::process::Command;

fn manifest_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn git(args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(manifest_dir())
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8(output.stdout).ok()?.trim().to_string())
}

/// The commit of this directory's own repository, or None when the tree is a
/// package unpacked outside a checkout.
fn own_commit() -> Option<String> {
    let top = PathBuf::from(git(&["rev-parse", "--show-toplevel"])?);
    if top.canonicalize().ok()? != manifest_dir().canonicalize().ok()? {
        return None;
    }
    let sha = git(&["rev-parse", "--verify", "HEAD"])?;
    (sha.len() == 40 && sha.bytes().all(|b| b.is_ascii_hexdigit())).then_some(sha)
}

fn version_line(flag: &str) -> String {
    let output = Command::new(env!("CARGO_BIN_EXE_s3-inventory-bill"))
        .arg(flag)
        .output()
        .unwrap();
    assert!(output.status.success(), "{flag}");
    assert!(output.stderr.is_empty(), "{flag}");
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.ends_with('\n'), "{flag}: {text:?}");
    text.trim_end_matches('\n').to_string()
}

#[test]
fn version_flags_print_the_crate_version_and_in_debug_the_commit() {
    let expected = match (cfg!(debug_assertions), own_commit()) {
        (true, Some(sha)) => format!("s3-inventory-bill 1.0.2 ({sha})"),
        _ => "s3-inventory-bill 1.0.2".to_string(),
    };
    for flag in ["--version", "-V"] {
        assert_eq!(version_line(flag), expected, "{flag}");
    }
}
