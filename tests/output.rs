//! `--output FILE`: the report goes to a file, written whole and renamed into
//! place; a symbolic link is replaced, not followed; a failure is exit 2.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        static N: AtomicU64 = AtomicU64::new(0);
        let n = N.fetch_add(1, Ordering::Relaxed);
        let path =
            std::env::temp_dir().join(format!("s3-inventory-bill-out-{}-{n}", std::process::id()));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn join(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }

    fn names(&self) -> Vec<String> {
        let mut names: Vec<String> = fs::read_dir(&self.0)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(&self.0, fs::Permissions::from_mode(0o755));
        }
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn run(args: &[&str]) -> (i32, String, String) {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_s3-inventory-bill"));
    cmd.current_dir(env!("CARGO_MANIFEST_DIR"));
    cmd.env("NO_COLOR", "1");
    cmd.env("SOURCE_DATE_EPOCH", "1795132800");
    cmd.args(args);
    let out = cmd.output().unwrap();
    (
        out.status.code().unwrap(),
        String::from_utf8(out.stdout).unwrap(),
        String::from_utf8(out.stderr).unwrap(),
    )
}

fn s(path: &Path) -> &str {
    path.to_str().unwrap()
}

const INV: &str = "examples/inventory.csv";

#[test]
fn the_report_goes_to_the_file_and_not_to_standard_output() {
    let dir = Scratch::new();
    for flags in [&[][..], &["--json"][..], &["--md"][..]] {
        let mut plain: Vec<&str> = flags.to_vec();
        plain.extend(["--as-of", "2026-11-09", INV]);
        let (code, expected, err) = run(&plain);
        assert_eq!(code, 0, "{err}");
        let target = dir.join("report.out");
        let mut args: Vec<&str> = flags.to_vec();
        args.extend(["--output", s(&target), "--as-of", "2026-11-09", INV]);
        let (code, stdout, stderr) = run(&args);
        assert_eq!(code, 0, "{stderr}");
        assert!(stdout.is_empty(), "{stdout}");
        assert!(stderr.is_empty(), "{stderr}");
        assert_eq!(fs::read_to_string(&target).unwrap(), expected, "{flags:?}");
        assert_eq!(dir.names(), ["report.out"], "no temporary file is left");
    }
}

#[test]
fn an_existing_file_is_replaced_whole() {
    let dir = Scratch::new();
    let target = dir.join("report.txt");
    fs::write(&target, "Q".repeat(100_000)).unwrap();
    let (code, _, err) = run(&["--output", s(&target), INV]);
    assert_eq!(code, 0, "{err}");
    let text = fs::read_to_string(&target).unwrap();
    assert!(text.starts_with("s3-inventory-bill\n"), "{text}");
    assert!(!text.contains('Q'));
    assert_eq!(dir.names(), ["report.txt"]);
}

#[cfg(unix)]
#[test]
fn a_symlink_is_replaced_and_never_followed() {
    let dir = Scratch::new();
    let victim = dir.join("victim.txt");
    fs::write(&victim, "keep me").unwrap();
    let link = dir.join("link.txt");
    std::os::unix::fs::symlink(&victim, &link).unwrap();
    let (code, _, err) = run(&["--output", s(&link), INV]);
    assert_eq!(code, 0, "{err}");
    assert_eq!(fs::read_to_string(&victim).unwrap(), "keep me");
    let meta = fs::symlink_metadata(&link).unwrap();
    assert!(meta.is_file() && !meta.file_type().is_symlink());
    assert!(fs::read_to_string(&link)
        .unwrap()
        .starts_with("s3-inventory-bill\n"));
    // A link that points nowhere is replaced too.
    let dangling = dir.join("dangling.txt");
    std::os::unix::fs::symlink(dir.join("nowhere"), &dangling).unwrap();
    let (code, _, err) = run(&["--output", s(&dangling), INV]);
    assert_eq!(code, 0, "{err}");
    assert!(!dir.join("nowhere").exists());
    assert!(fs::symlink_metadata(&dangling).unwrap().is_file());
}

#[test]
fn write_failures_are_exit_2_with_a_message_and_nothing_is_created() {
    let dir = Scratch::new();
    let missing = dir.join("no-such-dir").join("report.txt");
    let (code, stdout, stderr) = run(&["--output", s(&missing), INV]);
    assert_eq!(code, 2, "{stderr}");
    assert!(stdout.is_empty(), "{stdout}");
    assert!(
        stderr.starts_with("s3-inventory-bill: could not write report to "),
        "{stderr}"
    );
    assert!(!dir.join("no-such-dir").exists());
    assert_eq!(stderr.lines().count(), 1, "{stderr}");
    assert!(!stderr.contains("panicked"), "{stderr}");

    let folder = dir.join("folder");
    fs::create_dir(&folder).unwrap();
    let (code, _, stderr) = run(&["--output", s(&folder), INV]);
    assert_eq!(code, 2, "{stderr}");
    assert!(stderr.contains("is a directory"), "{stderr}");
    assert!(folder.is_dir());
    assert_eq!(dir.names(), ["folder"]);
}

#[cfg(unix)]
#[test]
fn a_special_file_is_refused_and_a_read_only_directory_is_exit_2() {
    use std::os::unix::fs::PermissionsExt;
    let dir = Scratch::new();
    let fifo = dir.join("pipe");
    assert!(Command::new("mkfifo")
        .arg(&fifo)
        .status()
        .unwrap()
        .success());
    let (code, _, stderr) = run(&["--output", s(&fifo), INV]);
    assert_eq!(code, 2, "{stderr}");
    assert!(stderr.contains("not a regular file"), "{stderr}");
    let (code, _, stderr) = run(&["--output", "/dev/null", INV]);
    assert_eq!(code, 2, "{stderr}");
    assert!(stderr.contains("not a regular file"), "{stderr}");

    let locked = dir.join("locked");
    fs::create_dir(&locked).unwrap();
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o555)).unwrap();
    let probe = locked.join("probe");
    let writable = fs::write(&probe, b"").is_ok();
    let target = locked.join("report.txt");
    let (code, _, stderr) = run(&["--output", s(&target), INV]);
    if writable {
        // Running with rights that ignore the mode (for example as root).
        assert_eq!(code, 0, "{stderr}");
    } else {
        assert_eq!(code, 2, "{stderr}");
        assert!(stderr.contains("could not write report to"), "{stderr}");
        assert!(!stderr.contains("panicked"), "{stderr}");
    }
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o755)).unwrap();
}

#[test]
fn a_failed_run_leaves_an_existing_output_file_as_it_was() {
    let dir = Scratch::new();
    let target = dir.join("report.txt");
    fs::write(&target, "old report").unwrap();
    let (code, _, stderr) = run(&["--output", s(&target), "no-such-input.csv"]);
    assert_eq!(code, 2, "{stderr}");
    assert_eq!(fs::read_to_string(&target).unwrap(), "old report");
    assert_eq!(dir.names(), ["report.txt"]);
}

#[test]
fn json_warnings_stay_on_stderr_and_a_control_character_in_the_path_is_escaped() {
    let dir = Scratch::new();
    let inv = dir.join("inv.csv");
    fs::write(&inv, "Bucket,Key,Size,StorageClass\nb,a/k,5,STANDARD\n").unwrap();
    let target = dir.join("out.json");
    let (code, stdout, stderr) = run(&[
        "--json",
        "--as-of",
        "2026-11-09",
        "--output",
        s(&target),
        s(&inv),
    ]);
    assert_eq!(code, 0, "{stderr}");
    assert!(stdout.is_empty());
    assert!(stderr.starts_with("warning: "), "{stderr}");
    let parsed: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&target).unwrap()).unwrap();
    let listed = parsed["warnings"].as_array().unwrap().len();
    assert!(listed >= 1);
    assert_eq!(stderr.lines().count(), listed, "{stderr}");

    let bad = dir.join("no\u{1b}such").join("x\ny.txt");
    let (code, _, stderr) = run(&["--output", s(&bad), INV]);
    assert_eq!(code, 2);
    assert!(!stderr.contains('\u{1b}'), "{stderr:?}");
    assert_eq!(stderr.lines().count(), 1, "{stderr:?}");
    assert!(stderr.contains("no\\u{1b}such"), "{stderr}");
}

#[test]
fn verbose_names_the_file_and_help_lists_the_option() {
    let dir = Scratch::new();
    let target = dir.join("r.txt");
    let (code, stdout, stderr) = run(&["--verbose", "--output", s(&target), INV]);
    assert_eq!(code, 0, "{stderr}");
    assert!(stdout.is_empty());
    assert!(stderr.contains("s3-inventory-bill: wrote "), "{stderr}");
    let (_, help, _) = run(&["--help"]);
    assert!(help.contains("--output <FILE>"), "{help}");
}
