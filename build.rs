//! Records the commit of the checkout for debug builds, so that `--version`
//! says which tree a development binary came from. Release builds, and trees
//! that are not a git checkout of their own (an unpacked package inside some
//! other repository, no git, a failed command), get nothing and print the
//! crate version only.

use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    println!("cargo:rustc-check-cfg=cfg(has_git_commit)");
    println!("cargo:rerun-if-changed=build.rs");
    let dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".into()));
    watch_head(&dir);
    if std::env::var("PROFILE").as_deref() != Ok("debug") {
        return;
    }
    if let Some(sha) = own_commit(&dir) {
        println!("cargo:rustc-cfg=has_git_commit");
        println!("cargo:rustc-env=GIT_COMMIT={sha}");
    }
}

fn git(dir: &Path, args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(dir)
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8(output.stdout).ok()?.trim().to_string())
}

/// The full commit of HEAD, only when `dir` is itself the top of the checkout.
fn own_commit(dir: &Path) -> Option<String> {
    let top = PathBuf::from(git(dir, &["rev-parse", "--show-toplevel"])?);
    if top.canonicalize().ok()? != dir.canonicalize().ok()? {
        return None;
    }
    let sha = git(dir, &["rev-parse", "--verify", "HEAD"])?;
    (sha.len() == 40 && sha.bytes().all(|b| b.is_ascii_hexdigit())).then_some(sha)
}

/// Rebuild when HEAD or the branch it names moves.
fn watch_head(dir: &Path) {
    let Some(head) = git(dir, &["rev-parse", "--git-path", "HEAD"]) else {
        return;
    };
    let head = dir.join(head);
    println!("cargo:rerun-if-changed={}", head.display());
    let Ok(text) = std::fs::read_to_string(&head) else {
        return;
    };
    if let Some(name) = text.trim().strip_prefix("ref:") {
        if let Some(path) = git(dir, &["rev-parse", "--git-path", name.trim()]) {
            println!("cargo:rerun-if-changed={}", dir.join(path).display());
        }
    }
}
