//! `--quiet` and `--verbose`: the report on standard output never changes;
//! `--quiet` silences the warning copies on standard error, `--verbose` adds
//! progress lines there, and both together are a usage error.

use std::fs;
use std::process::Command;

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

/// A one-object inventory without a date column: `--as-of` then warns.
fn warning_inventory(name: &str) -> String {
    let path = std::env::temp_dir().join(format!(
        "s3-inventory-bill-verbosity-{}-{name}.csv",
        std::process::id()
    ));
    fs::write(
        &path,
        "Bucket,Key,Size,StorageClass\nb,a/k,1073741824,STANDARD\n",
    )
    .unwrap();
    path.to_str().unwrap().to_string()
}

#[test]
fn help_lists_quiet_and_verbose() {
    let (code, stdout, _) = run(&["--help"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("--quiet"), "{stdout}");
    assert!(stdout.contains("--verbose"), "{stdout}");
}

#[test]
fn quiet_keeps_the_report_and_drops_the_stderr_warnings() {
    let inv = warning_inventory("quiet");
    let base = ["--json", "--as-of", "2026-11-09"];
    let (code, plain_out, plain_err) = run(&[base[0], base[1], base[2], &inv]);
    assert_eq!(code, 0);
    assert!(plain_err.starts_with("warning: "), "{plain_err}");
    let (code, quiet_out, quiet_err) = run(&["--quiet", base[0], base[1], base[2], &inv]);
    assert_eq!(code, 0, "{quiet_err}");
    assert_eq!(quiet_out, plain_out);
    assert!(quiet_err.is_empty(), "{quiet_err}");
    // The text report keeps its warning lines: they are part of the result.
    let (code, text, err) = run(&["--quiet", "--as-of", "2026-11-09", &inv]);
    assert_eq!(code, 0);
    assert!(
        text.contains("warning: LastModifiedDate column is absent"),
        "{text}"
    );
    assert!(err.is_empty(), "{err}");
    let _ = fs::remove_file(&inv);
}

#[test]
fn quiet_still_shows_errors_with_exit_2() {
    let (code, stdout, stderr) = run(&["--quiet", "no-such-file.csv"]);
    assert_eq!(code, 2);
    assert!(stdout.is_empty(), "{stdout}");
    assert!(
        stderr.starts_with("s3-inventory-bill: no-such-file.csv: "),
        "{stderr}"
    );
}

#[test]
fn verbose_adds_progress_on_stderr_only() {
    let (code, plain_out, plain_err) = run(&["--as-of", "2026-11-09", "examples/inventory.csv"]);
    assert_eq!(code, 0);
    assert!(plain_err.is_empty(), "{plain_err}");
    let (code, out, err) = run(&[
        "--verbose",
        "--as-of",
        "2026-11-09",
        "examples/inventory.csv",
    ]);
    assert_eq!(code, 0, "{err}");
    assert_eq!(out, plain_out);
    let lines: Vec<&str> = err.lines().collect();
    assert_eq!(
        lines,
        [
            "s3-inventory-bill: reading examples/inventory.csv",
            "s3-inventory-bill: pricing 1 file",
            "s3-inventory-bill: done",
        ],
        "{err}"
    );
    let (_, _, err) = run(&[
        "--verbose",
        "examples/inventory.csv",
        "examples/inventory.csv",
    ]);
    assert!(err.contains("pricing 2 files"), "{err}");
}

#[test]
fn verbose_escapes_a_control_character_in_a_path() {
    let (code, _, err) = run(&["--verbose", "no\u{1b}such.csv"]);
    assert_eq!(code, 2);
    assert!(!err.contains('\u{1b}'), "{err:?}");
    assert!(err.contains("reading no\\u{1b}such.csv"), "{err}");
}

#[test]
fn quiet_with_verbose_is_a_usage_error() {
    for args in [
        ["--quiet", "--verbose", "examples/inventory.csv"],
        ["--verbose", "--quiet", "examples/inventory.csv"],
    ] {
        let (code, stdout, stderr) = run(&args);
        assert_eq!(code, 2, "{stderr}");
        assert!(stdout.is_empty(), "{stdout}");
        assert!(
            stderr.contains("--quiet") && stderr.contains("--verbose"),
            "{stderr}"
        );
    }
}

#[test]
fn old_price_data_warns_on_stderr_in_every_mode() {
    let inv = warning_inventory("priceage");
    for mode in [None, Some("--md"), Some("--json")] {
        let mut args = vec!["--as-of", "2027-06-01"];
        args.extend(mode);
        args.push(&inv);
        let (code, _, err) = run(&args);
        assert_eq!(code, 0, "{err}");
        assert!(
            err.contains("warning: price data dated 2026-11-09 is more than 180 days"),
            "{mode:?}: {err}"
        );
        args.insert(0, "--quiet");
        let (code, _, err) = run(&args);
        assert_eq!(code, 0);
        assert!(err.is_empty(), "{mode:?}: {err}");
    }
    let _ = fs::remove_file(&inv);
}
