//! A real SIGINT ends the run with exit 130, one line on standard error and
//! nothing on standard output.

#![cfg(unix)]

use std::fs;
use std::io::Write;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use flate2::write::GzEncoder;
use flate2::Compression;

fn gzip(bytes: &[u8]) -> Vec<u8> {
    let mut encoder = GzEncoder::new(Vec::new(), Compression::fast());
    encoder.write_all(bytes).unwrap();
    encoder.finish().unwrap()
}

/// A small file that takes a while to read: a gzip file of many members, each
/// inflating to about 1 MiB of rows (about 100 MiB in all).
fn slow_input(path: &std::path::Path) {
    let rows = "b,logs/a.bin,1073741824,STANDARD\n".repeat(32 * 1024);
    let first = gzip(format!("Bucket,Key,Size,StorageClass\n{rows}").as_bytes());
    let rest = gzip(rows.as_bytes());
    let mut file = first;
    for _ in 0..100 {
        file.extend_from_slice(&rest);
    }
    assert!(file.len() < 4 * 1024 * 1024, "{}", file.len());
    fs::write(path, file).unwrap();
}

/// Wait until the child has a handler for SIGINT, where /proc tells.
fn wait_for_handler(pid: u32) {
    let status = format!("/proc/{pid}/status");
    let deadline = Instant::now() + Duration::from_secs(10);
    if !std::path::Path::new(&status).exists() {
        std::thread::sleep(Duration::from_millis(300));
        return;
    }
    while Instant::now() < deadline {
        let text = fs::read_to_string(&status).unwrap_or_default();
        let caught = text
            .lines()
            .find_map(|line| line.strip_prefix("SigCgt:"))
            .and_then(|hex| u64::from_str_radix(hex.trim(), 16).ok())
            .unwrap_or(0);
        if caught & (1 << (2 - 1)) != 0 {
            return;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    panic!("the child never installed a SIGINT handler");
}

fn interrupted(extra: &[&str], input: &std::path::Path) -> (Option<i32>, String, String) {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_s3-inventory-bill"));
    cmd.current_dir(env!("CARGO_MANIFEST_DIR"));
    cmd.env("SOURCE_DATE_EPOCH", "1795132800");
    cmd.args(extra).arg(input);
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
    let child = cmd.spawn().unwrap();
    wait_for_handler(child.id());
    let sent = Command::new("kill")
        .args(["-INT", &child.id().to_string()])
        .status()
        .unwrap();
    assert!(sent.success());
    let out = child.wait_with_output().unwrap();
    (
        out.status.code(),
        String::from_utf8(out.stdout).unwrap(),
        String::from_utf8(out.stderr).unwrap(),
    )
}

#[test]
fn sigint_while_reading_exits_130_with_one_line_and_no_output() {
    let dir = std::env::temp_dir().join(format!("s3-inventory-bill-sigint-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let input = dir.join("slow.csv.gz");
    slow_input(&input);
    for extra in [&[][..], &["--json"][..]] {
        let (code, stdout, stderr) = interrupted(extra, &input);
        assert_eq!(code, Some(130), "{extra:?}: {stderr}");
        assert!(stdout.is_empty(), "{extra:?}: {stdout}");
        assert!(
            stderr.ends_with("s3-inventory-bill: interrupted by SIGINT\n"),
            "{extra:?}: {stderr}"
        );
        assert_eq!(stderr.matches("interrupted").count(), 1, "{stderr}");
    }
    // The same input without a signal finishes and prints a report.
    let out = Command::new(env!("CARGO_BIN_EXE_s3-inventory-bill"))
        .env("SOURCE_DATE_EPOCH", "1795132800")
        .arg(&input)
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(0));
    assert!(String::from_utf8(out.stdout)
        .unwrap()
        .contains("total objects=3309568 "));
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn sigint_leaves_an_existing_output_file_as_it_was_and_creates_none() {
    let dir = std::env::temp_dir().join(format!(
        "s3-inventory-bill-sigint-out-{}",
        std::process::id()
    ));
    fs::create_dir_all(&dir).unwrap();
    let input = dir.join("slow.csv.gz");
    slow_input(&input);
    let kept = dir.join("kept.txt");
    fs::write(&kept, "old report").unwrap();
    let fresh = dir.join("fresh.txt");
    for target in [&kept, &fresh] {
        let (code, stdout, stderr) = interrupted(&["--output", target.to_str().unwrap()], &input);
        assert_eq!(code, Some(130), "{stderr}");
        assert!(stdout.is_empty(), "{stdout}");
        assert!(stderr.ends_with("interrupted by SIGINT\n"), "{stderr}");
    }
    assert_eq!(fs::read_to_string(&kept).unwrap(), "old report");
    assert!(!fresh.exists());
    let mut names: Vec<_> = fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    assert_eq!(names, ["kept.txt", "slow.csv.gz"]);
    let _ = fs::remove_dir_all(&dir);
}
