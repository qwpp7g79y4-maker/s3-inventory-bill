//! Command-line behavior: exit codes, files, and the report the binary prints.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

struct Scratch {
    path: PathBuf,
}

impl Scratch {
    fn new() -> Self {
        static N: AtomicU64 = AtomicU64::new(0);
        let n = N.fetch_add(1, Ordering::Relaxed);
        let path =
            std::env::temp_dir().join(format!("s3-inventory-bill-cli-{}-{n}", std::process::id()));
        fs::create_dir_all(&path).unwrap();
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

const FIXED_TODAY: &str = "1795132800";

fn command(args: &[&str]) -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_s3-inventory-bill"));
    cmd.current_dir(env!("CARGO_MANIFEST_DIR"));
    cmd.env("NO_COLOR", "1");
    // 2026-11-20T00:00:00Z: the tests never read the real clock.
    cmd.env("SOURCE_DATE_EPOCH", FIXED_TODAY);
    cmd.args(args);
    cmd
}

fn run(args: &[&str]) -> (i32, String, String) {
    let out = command(args).output().unwrap();
    (
        out.status.code().unwrap(),
        String::from_utf8(out.stdout).unwrap(),
        String::from_utf8(out.stderr).unwrap(),
    )
}

#[test]
fn help_and_version_exit_0() {
    let (code, stdout, stderr) = run(&["--help"]);
    assert_eq!(code, 0);
    assert!(stderr.is_empty(), "{stderr}");
    assert!(stdout.contains("no network"));
    assert!(stdout.contains("Exit codes: 0 ok, 2 input or usage error, 130 interrupted (SIGINT)."));
    let (code, stdout, stderr) = run(&["--version"]);
    assert_eq!(code, 0);
    assert!(stderr.is_empty(), "{stderr}");
    assert!(stdout.contains("s3-inventory-bill 1.0.1"));
}

#[test]
fn usage_errors_exit_2() {
    let (code, stdout, stderr) = run(&[]);
    assert_eq!(code, 2);
    assert!(stdout.is_empty());
    assert!(stderr.contains("s3-inventory-bill"));
    let (code, _, stderr) = run(&["--no-such-flag", "x"]);
    assert_eq!(code, 2);
    assert!(!stderr.is_empty());
    let (code, _, stderr) = run(&["--prefix-depth=0", "examples/inventory.csv"]);
    assert_eq!(code, 2);
    assert!(!stderr.is_empty());
    let (code, _, stderr) = run(&["--prefix-depth=-1", "examples/inventory.csv"]);
    assert_eq!(code, 2);
    assert!(!stderr.is_empty());
    let (code, stdout, stderr) = run(&["--as-of", "2025-02-29", "examples/inventory.csv"]);
    assert_eq!(code, 2);
    assert!(stdout.is_empty());
    assert!(stderr.starts_with("s3-inventory-bill: "));
    assert!(stderr.contains("invalid --as-of"));
    assert!(!stderr.contains("example-bucket"));
}

#[test]
fn missing_file_exits_2() {
    let (code, stdout, stderr) = run(&["missing-inventory.csv"]);
    assert_eq!(code, 2);
    assert!(stdout.is_empty());
    assert!(stderr.starts_with("s3-inventory-bill: "));
    assert!(stderr.contains("missing-inventory.csv"));
}

#[test]
fn missing_prices_file_exits_2() {
    let (code, stdout, stderr) =
        run(&["--prices", "missing-prices.json", "examples/inventory.csv"]);
    assert_eq!(code, 2);
    assert!(stdout.is_empty());
    assert!(stderr.starts_with("s3-inventory-bill: "));
    assert!(stderr.contains("missing-prices.json"));
}

#[test]
fn parquet_and_orc_names_exit_2() {
    let scratch = Scratch::new();
    let parquet = scratch.path().join("dump.parquet");
    fs::write(&parquet, b"not-read").unwrap();
    let (code, stdout, stderr) = run(&[parquet.to_str().unwrap()]);
    assert_eq!(code, 2);
    assert!(stdout.is_empty());
    assert!(stderr.contains("parquet inventories are not read; only CSV and gzip CSV are"));
    assert!(!stderr.contains("not-read"));
    let orc = scratch.path().join("dump.orc.gz");
    fs::write(&orc, b"nope").unwrap();
    let (code, _, stderr) = run(&[orc.to_str().unwrap()]);
    assert_eq!(code, 2);
    assert!(stderr.contains("orc inventories are not read; only CSV and gzip CSV are"));
}

#[test]
fn size_with_ascii_space_is_read_and_a_sign_or_other_digits_are_refused() {
    let scratch = Scratch::new();
    let file = scratch.path().join("size.csv");
    let path = file.to_str().unwrap();
    for size in [" 5", "5 ", "\t5", "5\t", "05", "\"5\""] {
        fs::write(
            &file,
            format!("Bucket,Key,Size,StorageClass\nb,k,{size},STANDARD\n"),
        )
        .unwrap();
        let (code, stdout, stderr) = run(&[path]);
        assert_eq!(code, 0, "{size:?}: {stderr}");
        assert!(stdout.contains("total objects=1 raw_bytes=5 "), "{size:?}");
    }
    for size in ["+5", "-0", "1_0", "0x10", "", "\u{a0}5", "\u{663}"] {
        fs::write(
            &file,
            format!("Bucket,Key,Size,StorageClass\nb,k,{size},STANDARD\n"),
        )
        .unwrap();
        let (code, stdout, stderr) = run(&[path]);
        assert_eq!(code, 2, "{size:?}");
        assert!(stdout.is_empty(), "{size:?}");
        assert!(stderr.contains("row 2: invalid Size"), "{size:?}: {stderr}");
    }
}

#[test]
fn example_inventory_exits_0_and_json_keeps_stderr_empty() {
    let (code, stdout, stderr) = run(&["examples/inventory.csv"]);
    assert_eq!(code, 0, "{stderr}");
    assert!(stderr.is_empty(), "{stderr}");
    assert!(stdout.contains("estimated_usd_month=0.023"));
    assert!(stdout.contains("logs/ objects=1"));
    assert!(stdout.contains("omitted: pass --as-of YYYY-MM-DD"));
    let (code, stdout, stderr) =
        run(&["--json", "--as-of", "2026-11-09", "examples/inventory.csv"]);
    assert_eq!(code, 0, "{stderr}");
    assert!(stderr.is_empty(), "{stderr}");
    assert!(stdout.starts_with('{'));
    assert!(stdout.contains("\"estimated_usd_month\": \"0.023\""));
    assert!(stdout.contains("\"not_executed\": true"));
    assert!(stdout.contains("\"warnings\": []"));
}

#[test]
fn prefix_depth_groups_the_sample_key() {
    let (code, stdout, stderr) = run(&["--prefix-depth", "2", "examples/inventory.csv"]);
    assert_eq!(code, 0, "{stderr}");
    assert!(stderr.is_empty());
    assert!(stdout.contains("prefix_depth: 2"));
    assert!(stdout.contains("logs/app/ objects=1"));
}

#[test]
fn two_files_share_one_tier_schedule() {
    let scratch = Scratch::new();
    let one = scratch.path().join("a.csv");
    let two = scratch.path().join("b.csv");
    let row = "Bucket,Key,Size,StorageClass\nb,k,1073741824,STANDARD\n";
    fs::write(&one, row).unwrap();
    fs::write(&two, row).unwrap();
    let (code, stdout, stderr) = run(&[one.to_str().unwrap(), two.to_str().unwrap()]);
    assert_eq!(code, 0, "{stderr}");
    assert!(stderr.is_empty());
    assert!(stdout.contains("objects=2"));
    assert!(stdout.contains("estimated_usd_month=0.046"));
    assert!(stdout.contains("raw_bytes=2147483648"));
}

#[test]
fn symlink_is_followed_and_a_directory_is_rejected() {
    let scratch = Scratch::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;
        let file = scratch.path().join("real.csv");
        fs::write(
            &file,
            "Bucket,Key,Size,StorageClass\nb,k,1073741824,STANDARD\n",
        )
        .unwrap();
        let link = scratch.path().join("link.csv");
        symlink(&file, &link).unwrap();
        let (code, stdout, stderr) = run(&[link.to_str().unwrap()]);
        assert_eq!(code, 0, "{stderr}");
        assert!(stderr.is_empty());
        assert!(stdout.contains("estimated_usd_month=0.023"));
    }
    let (code, stdout, stderr) = run(&[scratch.path().to_str().unwrap()]);
    assert_eq!(code, 2);
    assert!(stdout.is_empty());
    assert!(stderr.contains("is a directory"));
}

#[cfg(unix)]
#[test]
fn prices_fifo_is_refused_without_reading() {
    let scratch = Scratch::new();
    let fifo = scratch.path().join("prices.fifo");
    let made = Command::new("mkfifo").arg(&fifo).status().unwrap();
    assert!(made.success());
    let inv = scratch.path().join("inv.csv");
    fs::write(&inv, "Bucket,Key,Size,StorageClass\nb,k,1,STANDARD\n").unwrap();
    let (code, stdout, stderr) = run(&["--prices", fifo.to_str().unwrap(), inv.to_str().unwrap()]);
    assert_eq!(code, 2);
    assert!(stdout.is_empty());
    assert!(stderr.contains("not a file"), "{stderr}");
}

#[test]
fn header_only_is_zero_and_empty_input_exits_2() {
    let scratch = Scratch::new();
    let header = scratch.path().join("header.csv");
    fs::write(&header, "Bucket,Key,Size,StorageClass\n").unwrap();
    let (code, stdout, stderr) = run(&[header.to_str().unwrap()]);
    assert_eq!(code, 0, "{stderr}");
    assert!(stderr.is_empty());
    assert!(stdout.contains("total objects=0 raw_bytes=0 estimated_usd_month=0.00"));
    let empty = scratch.path().join("empty.csv");
    fs::write(&empty, "").unwrap();
    let (code, stdout, stderr) = run(&[empty.to_str().unwrap()]);
    assert_eq!(code, 2);
    assert!(stdout.is_empty());
    assert!(stderr.contains("empty inventory"));
}

#[test]
fn bad_size_does_not_echo_the_row() {
    let scratch = Scratch::new();
    let path = scratch.path().join("bad.csv");
    fs::write(
        &path,
        "Bucket,Key,Size,StorageClass\nsecret-bucket,super-secret-key,12abc,STANDARD\n",
    )
    .unwrap();
    let (code, stdout, stderr) = run(&[path.to_str().unwrap()]);
    assert_eq!(code, 2);
    assert!(stdout.is_empty());
    assert!(stderr.contains("invalid Size"));
    assert!(!stderr.contains("secret"));
    assert!(!stderr.contains("12abc"));
}

#[test]
fn extreme_price_exits_2_without_panic() {
    let scratch = Scratch::new();
    let mut text = include_str!("../prices/s3-storage.json").to_string();
    let huge = "79228162514264337593543950335";
    assert!(text.contains("\"standard_first_50_tb\": \"0.023\""));
    text = text.replacen(
        "\"standard_first_50_tb\": \"0.023\"",
        &format!("\"standard_first_50_tb\": \"{huge}\""),
        1,
    );
    let prices = scratch.path().join("prices.json");
    fs::write(&prices, text).unwrap();
    let inv = scratch.path().join("inv.csv");
    fs::write(
        &inv,
        "Bucket,Key,Size,StorageClass\nb,a,1073741824,STANDARD\nb,c,1073741824,STANDARD\n",
    )
    .unwrap();
    let (code, stdout, stderr) =
        run(&["--prices", prices.to_str().unwrap(), inv.to_str().unwrap()]);
    assert_eq!(code, 2, "{stderr}");
    assert!(stdout.is_empty());
    assert!(stderr.contains("amount does not fit"));
    assert!(!stderr.to_ascii_lowercase().contains("panic"));
    assert!(!stderr.contains(huge));
}

#[test]
fn surrogate_price_and_unicode_class_use_two_small_files() {
    let scratch = Scratch::new();
    let prices = include_str!("../prices/s3-storage.json").replacen(
        "\"region_name\": \"US East (N. Virginia)\"",
        "\"region_name\": \"\\uD83D\\uDE00\", \"\\uD83D\\uDE00\": \"tag\"",
        1,
    );
    let price_path = scratch.path().join("prices.json");
    let inventory = scratch.path().join("inventory.csv");
    fs::write(&price_path, prices).unwrap();
    fs::write(
        &inventory,
        "Bucket,Key,Size,StorageClass\n\
         b,a,1073741824,standard\n\
         b,b,4,caf\u{e9}\n\
         b,c,4,Caf\u{e9}\n\
         b,d,1,\u{e9}\n",
    )
    .unwrap();
    assert!(fs::metadata(&price_path).unwrap().len() < 4_096);
    assert!(fs::metadata(&inventory).unwrap().len() < 256);
    let (code, stdout, stderr) = run(&[
        "--prices",
        price_path.to_str().unwrap(),
        inventory.to_str().unwrap(),
    ]);
    assert_eq!(code, 0, "{stderr}");
    assert!(stderr.is_empty(), "{stderr}");
    assert!(stdout.contains("region: us-east-1 (\u{1F600})"));
    assert!(stdout.contains("STANDARD objects=1 raw_bytes=1073741824"));
    assert!(stdout.contains("estimated_usd_month=0.023"));
    assert!(stdout
        .contains("CAF\u{e9} objects=2 raw_bytes=8 billable_bytes=8 estimated_usd_month=unpriced"));
    assert!(stdout
        .contains("\u{e9} objects=1 raw_bytes=1 billable_bytes=1 estimated_usd_month=unpriced"));
    assert!(!stdout.contains('\u{00c3}'));

    let lone = scratch.path().join("lone.json");
    fs::write(&lone, "{\"tag\":\"\\uD800\"}\n").unwrap();
    let (code, stdout, stderr) = run(&[
        "--prices",
        lone.to_str().unwrap(),
        inventory.to_str().unwrap(),
    ]);
    assert_eq!(code, 2);
    assert!(stdout.is_empty());
    assert!(stderr.contains("not JSON"));

    let dup = scratch.path().join("dup.json");
    fs::write(&dup, "{\"\u{1F600}\":1,\"\\uD83D\\uDE00\":2}\n").unwrap();
    let (code, stdout, stderr) = run(&[
        "--prices",
        dup.to_str().unwrap(),
        inventory.to_str().unwrap(),
    ]);
    assert_eq!(code, 2);
    assert!(stdout.is_empty());
    assert!(stderr.contains("duplicate key"));
}

#[test]
fn headerless_aws_inventory_line_is_refused_with_the_expected_columns() {
    let dir = Scratch::new();
    let file = dir.path().join("raw.csv");
    fs::write(&file, "b,logs/a,10,2024-01-01T00:00:00.000Z,STANDARD\n").unwrap();
    let (code, stdout, stderr) = run(&[file.to_str().unwrap()]);
    assert_eq!(code, 2);
    assert!(stdout.is_empty(), "{stdout}");
    assert!(stderr.contains("missing column Bucket"), "{stderr}");
    assert!(stderr.contains("Bucket,Key,Size,StorageClass"), "{stderr}");
    assert!(stderr.contains("header line"), "{stderr}");
}

#[test]
fn bidi_and_line_separators_are_escaped_in_text_output() {
    let dir = Scratch::new();
    let file = dir.path().join("bidi.csv");
    fs::write(
        &file,
        "Bucket,Key,Size,StorageClass\n\
         b,a\u{2028}forged/x,1,STANDARD\n\
         b,c\u{202e}rev/y,2,STANDARD\n\
         b,m\u{200f}n\u{2066}o/z,3,STANDARD\n\
         b,k,4,GL\u{202e}ACIER\n",
    )
    .unwrap();
    let (code, stdout, stderr) = run(&[file.to_str().unwrap()]);
    assert_eq!(code, 0, "{stderr}");
    for raw in ['\u{2028}', '\u{202e}', '\u{200f}', '\u{2066}'] {
        assert!(!stdout.contains(raw), "raw {raw:?} in {stdout}");
    }
    assert!(!stdout
        .as_bytes()
        .windows(3)
        .any(|w| w == [0xe2, 0x80, 0xa8]));
    assert!(!stdout
        .as_bytes()
        .windows(3)
        .any(|w| w == [0xe2, 0x80, 0xae]));
    assert!(stdout.contains("a\\u{2028}forged/"), "{stdout}");
    assert!(stdout.contains("c\\u{202e}rev/"), "{stdout}");
    assert!(stdout.contains("m\\u{200f}n\\u{2066}o/"), "{stdout}");
    assert!(stdout.contains("GL\\u{202e}ACIER objects=1"), "{stdout}");
}

#[test]
fn invisible_filler_and_tag_characters_are_escaped_but_still_counted() {
    let dir = Scratch::new();
    let file = dir.path().join("invisible.csv");
    fs::write(
        &file,
        "Bucket,Key,Size,StorageClass\n\
         b,a\u{3164}x/y,1,STANDARD\n\
         b,c\u{e0001}z/y,2,STANDARD\n",
    )
    .unwrap();
    let (code, stdout, stderr) = run(&[file.to_str().unwrap()]);
    assert_eq!(code, 0, "{stderr}");
    for raw in ['\u{3164}', '\u{e0001}'] {
        assert!(!stdout.contains(raw), "raw {raw:?} in {stdout}");
    }
    assert!(stdout.contains("a\\u{3164}x/"), "{stdout}");
    assert!(stdout.contains("c\\u{e0001}z/"), "{stdout}");
    assert!(
        stdout.contains(
            "total objects=2 raw_bytes=3 estimated_usd_month=0.0000000000642612576484680176"
        ),
        "{stdout}"
    );
    assert!(
        stdout.contains("STANDARD objects=2 raw_bytes=3"),
        "{stdout}"
    );
}

#[test]
fn invisible_format_gap_characters_are_escaped_but_still_counted() {
    let dir = Scratch::new();
    let file = dir.path().join("gap.csv");
    fs::write(
        &file,
        "Bucket,Key,Size,StorageClass\n\
         b,a\u{2065}x/y,1,STANDARD\n\
         b,c\u{206a}z/y,2,STANDARD\n",
    )
    .unwrap();
    let (code, stdout, stderr) = run(&[file.to_str().unwrap()]);
    assert_eq!(code, 0, "{stderr}");
    for raw in ['\u{2065}', '\u{206a}'] {
        assert!(!stdout.contains(raw), "raw {raw:?} in {stdout}");
    }
    assert!(stdout.contains("a\\u{2065}x/"), "{stdout}");
    assert!(stdout.contains("c\\u{206a}z/"), "{stdout}");
    assert!(
        stdout.contains(
            "total objects=2 raw_bytes=3 estimated_usd_month=0.0000000000642612576484680176"
        ),
        "{stdout}"
    );
}

#[test]
fn hangul_filler_key_is_escaped_but_still_counted() {
    let dir = Scratch::new();
    let file = dir.path().join("filler.csv");
    fs::write(
        &file,
        "Bucket,Key,Size,StorageClass\n\
         b,a\u{115f}x/y,1,STANDARD\n",
    )
    .unwrap();
    let (code, stdout, stderr) = run(&[file.to_str().unwrap()]);
    assert_eq!(code, 0, "{stderr}");
    assert!(!stdout.contains('\u{115f}'), "{stdout}");
    assert!(stdout.contains("a\\u{115f}x/"), "{stdout}");
    assert!(stdout.contains("total objects=1 raw_bytes=1 "), "{stdout}");
    assert!(!stdout.contains("estimated_usd_month=0 "), "{stdout}");
}

#[cfg(target_os = "linux")]
mod stdout_closed {
    use std::process::Command;

    const SAMPLE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/examples/inventory.csv");

    fn sh(redirect: &str, arg: &str) -> (Option<i32>, String) {
        let bin = env!("CARGO_BIN_EXE_s3-inventory-bill");
        let out = Command::new("sh")
            .env("SOURCE_DATE_EPOCH", crate::FIXED_TODAY)
            .arg("-c")
            .arg(format!(
                "exec \"$0\" {} {redirect}",
                arg.replace(SAMPLE, "\"$1\"")
            ))
            .arg(bin)
            .arg(SAMPLE)
            .output()
            .unwrap();
        (
            out.status.code(),
            String::from_utf8_lossy(&out.stderr).into(),
        )
    }

    fn all_invocations() -> [String; 4] {
        [
            "--help".to_string(),
            "--version".to_string(),
            SAMPLE.to_string(),
            format!("--json {SAMPLE}"),
        ]
    }

    #[test]
    fn closed_stdout_is_exit_2() {
        for arg in all_invocations() {
            let (code, err) = sh(">&-", &arg);
            assert_eq!(code, Some(2), "{arg}");
            assert!(err.contains("could not write output"), "{err}");
            assert!(!err.contains("panicked"), "{err}");
        }
    }

    #[test]
    fn dev_full_is_exit_2() {
        for arg in all_invocations() {
            let (code, err) = sh(">/dev/full", &arg);
            assert_eq!(code, Some(2), "{arg}");
            assert!(err.contains("could not write output"), "{err}");
            assert!(!err.contains("panicked"), "{err}");
        }
    }

    #[test]
    fn dev_null_keeps_real_exit() {
        for arg in all_invocations() {
            for redirect in [">/dev/null", "1<>/dev/null"] {
                let (code, err) = sh(redirect, &arg);
                assert_eq!(code, Some(0), "{arg} {redirect}: {err}");
                assert!(err.is_empty(), "{err}");
            }
        }
        assert_eq!(sh("1<>/dev/null", "--bogus").0, Some(2));
    }
}

#[test]
fn price_labels_with_separators_are_escaped_in_text_output() {
    let dir = Scratch::new();
    let prices = include_str!("../prices/s3-storage.json")
        .replacen(
            "\"region_name\": \"US East (N. Virginia)\"",
            "\"region_name\": \"N\\u2028V\\u2029A\\u202Ez\"",
            1,
        )
        .replacen(
            "\"region\": \"us-east-1\"",
            "\"region\": \"us\\u2028-\\u2029east\\u202E-1\"",
            1,
        );
    let price_path = dir.path().join("prices.json");
    let inv = dir.path().join("inv.csv");
    fs::write(&price_path, prices).unwrap();
    fs::write(&inv, "Bucket,Key,Size,StorageClass\nb,a,1,STANDARD\n").unwrap();
    let (code, stdout, stderr) = run(&[
        "--prices",
        price_path.to_str().unwrap(),
        inv.to_str().unwrap(),
    ]);
    assert_eq!(code, 0, "{stderr}");
    for raw in ['\u{2028}', '\u{2029}', '\u{202e}'] {
        assert!(!stdout.contains(raw), "raw {raw:?} in {stdout}");
    }
    assert!(
        stdout.contains("us\\u{2028}-\\u{2029}east\\u{202e}-1"),
        "{stdout}"
    );
    assert!(
        stdout.contains("N\\u{2028}V\\u{2029}A\\u{202e}z"),
        "{stdout}"
    );
}

#[test]
fn quote_error_names_the_line_and_exits_2() {
    let scratch = Scratch::new();
    let path = scratch.path().join("quote.csv");
    fs::write(
        &path,
        "Bucket,Key,Size,StorageClass\nb,ok,1,STANDARD\nb,a\"b,1,STANDARD\n",
    )
    .unwrap();
    let (code, stdout, stderr) = run(&[path.to_str().unwrap()]);
    assert_eq!(code, 2);
    assert!(stdout.is_empty());
    assert!(stderr.contains("row 3: quote in an unquoted field"));
}

#[test]
fn bad_gzip_in_second_file_names_that_file() {
    let scratch = Scratch::new();
    let good = scratch.path().join("good.csv");
    fs::write(&good, b"Bucket,Key,Size,StorageClass\nb,k,1,STANDARD\n").unwrap();
    let bad = scratch.path().join("second-broken.csv.gz");
    fs::write(&bad, [0x1f, 0x8b, 0x08, 0x00, 0xde, 0xad, 0xbe, 0xef, 0x01]).unwrap();
    let (code, stdout, stderr) = run(&[good.to_str().unwrap(), bad.to_str().unwrap()]);
    assert_eq!(code, 2);
    assert!(stdout.is_empty());
    assert!(stderr.contains("second-broken.csv.gz"), "{stderr}");
    assert!(stderr.contains("not a gzip inventory"), "{stderr}");
    assert!(!stderr.contains("good.csv"), "{stderr}");
}

#[test]
fn prefix_depth_range_is_named_in_help_readme_and_man_and_enforced() {
    let (_, help, _) = run(&["--help"]);
    assert!(
        help.contains("Path segments in the prefix, 1 to 8."),
        "{help}"
    );
    for doc in ["README.md", "man/s3-inventory-bill.1"] {
        let text = fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join(doc)).unwrap();
        assert!(text.contains("1 to 8"), "{doc}");
    }
    for value in ["0", "9", "256", "-1"] {
        let (code, stdout, stderr) = run(&["--prefix-depth", value, "examples/inventory.csv"]);
        assert_eq!(code, 2, "{value}");
        assert!(stdout.is_empty());
        assert!(!stderr.contains("panicked"), "{stderr}");
    }
    for value in ["1", "8"] {
        let (code, _, stderr) = run(&["--prefix-depth", value, "examples/inventory.csv"]);
        assert_eq!(code, 0, "{value}: {stderr}");
    }
}

#[test]
fn keys_are_used_as_written_and_not_url_decoded() {
    let dir = Scratch::new();
    let inv = dir.path().join("inv.csv");
    fs::write(
        &inv,
        "Bucket,Key,Size,StorageClass\nb,my%20dir/file,1,STANDARD\nb,my dir/file,1,STANDARD\n",
    )
    .unwrap();
    let (code, stdout, stderr) = run(&[inv.to_str().unwrap()]);
    assert_eq!(code, 0, "{stderr}");
    assert!(stdout.contains("\nmy%20dir/ objects=1 "), "{stdout}");
    assert!(stdout.contains("\nmy dir/ objects=1 "), "{stdout}");
}

#[test]
fn warnings_go_to_stderr_with_json_and_savings_name_four_targets() {
    let dir = Scratch::new();
    let inv = dir.path().join("inv.csv");
    fs::write(
        &inv,
        "Bucket,Key,Size,StorageClass\nb,a/k,1048576,STANDARD\n",
    )
    .unwrap();
    let (code, stdout, stderr) = run(&["--json", "--as-of", "2026-11-09", inv.to_str().unwrap()]);
    assert_eq!(code, 0, "{stderr}");
    assert!(stderr.starts_with("warning: "), "{stderr}");
    assert!(!stdout.contains("warning: "), "{stdout}");
    let (code, stdout, _) = run(&["--as-of", "2026-11-09", "examples/inventory.csv"]);
    assert_eq!(code, 0);
    let targets: Vec<&str> = stdout
        .lines()
        .filter(|line| line.contains(" horizon_days="))
        .map(|line| line.split(' ').next().unwrap())
        .collect();
    assert_eq!(
        targets,
        ["STANDARD_IA", "GLACIER_IR", "GLACIER", "DEEP_ARCHIVE"]
    );
}

#[test]
fn cr_only_file_is_refused_and_empty_key_shares_the_empty_prefix() {
    let dir = Scratch::new();
    let cr = dir.path().join("cr.csv");
    fs::write(&cr, b"Bucket,Key,Size,StorageClass\rb,k,5,STANDARD\r").unwrap();
    let (code, stdout, stderr) = run(&[cr.to_str().unwrap()]);
    assert_eq!(code, 2);
    assert!(stdout.is_empty());
    assert!(
        stderr.contains("cr.csv: missing column StorageClass: "),
        "{stderr}"
    );
    let empty = dir.path().join("empty.csv");
    fs::write(
        &empty,
        "Bucket,Key,Size,StorageClass\nb,,1,STANDARD\nb,(empty),2,STANDARD\n",
    )
    .unwrap();
    let (code, stdout, stderr) = run(&[empty.to_str().unwrap()]);
    assert_eq!(code, 0, "{stderr}");
    assert!(
        stdout.contains("\n(empty) objects=2 raw_bytes=3 "),
        "{stdout}"
    );
}

#[test]
fn backslash_in_a_path_is_escaped_exactly_once() {
    let (code, stdout, stderr) = run(&[r"C:\Users\me\inv.csv"]);
    assert_eq!(code, 2);
    assert!(stdout.is_empty());
    assert!(
        stderr.starts_with(r"s3-inventory-bill: C:\\Users\\me\\inv.csv: "),
        "{stderr}"
    );
    assert!(!stderr.contains(r"\\\\"), "{stderr}");
    let (code, _, stderr) = run(&["no\u{202e}file.csv"]);
    assert_eq!(code, 2);
    assert!(
        stderr.starts_with("s3-inventory-bill: no\\u{202e}file.csv: "),
        "{stderr}"
    );
}

#[cfg(target_os = "linux")]
#[test]
fn non_utf8_name_is_escaped_in_a_later_csv_error() {
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;
    let scratch = Scratch::new();
    let mut name = b"bad\xff".to_vec();
    name.extend_from_slice(b".csv");
    let file = scratch.path().join(OsString::from_vec(name));
    fs::write(&file, "Bucket,Key,Size,StorageClass\nb,\"k,1,STANDARD\n").unwrap();
    let out = command(&[]).arg(&file).output().unwrap();
    assert_eq!(out.status.code(), Some(2));
    assert!(out.stdout.is_empty());
    let stderr = String::from_utf8(out.stderr).unwrap();
    assert!(stderr.contains("bad\\\\xff.csv"), "{stderr}");
    assert!(stderr.contains("unterminated quote"), "{stderr}");
    assert!(!stderr.contains('\u{FFFD}'), "{stderr}");
}

#[cfg(target_os = "linux")]
#[test]
fn non_utf8_prices_name_is_escaped_and_a_valid_one_loads() {
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;
    let scratch = Scratch::new();
    let inventory = scratch.path().join("inventory.csv");
    fs::write(
        &inventory,
        "Bucket,Key,Size,StorageClass\nb,a,1073741824,standard\n",
    )
    .unwrap();
    let bad = scratch
        .path()
        .join(OsString::from_vec(b"p\xff.json".to_vec()));
    fs::write(&bad, "[]\n").unwrap();
    let out = command(&["--prices"])
        .arg(&bad)
        .arg(&inventory)
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2));
    assert!(out.stdout.is_empty());
    let stderr = String::from_utf8(out.stderr).unwrap();
    assert!(stderr.contains("p\\\\xff.json"), "{stderr}");
    assert!(!stderr.contains('\u{FFFD}'), "{stderr}");

    let good = scratch
        .path()
        .join(OsString::from_vec(b"g\xff.json".to_vec()));
    fs::copy("prices/s3-storage.json", &good).unwrap();
    let with = command(&["--prices"])
        .arg(&good)
        .arg(&inventory)
        .output()
        .unwrap();
    let without = command(&[]).arg(&inventory).output().unwrap();
    assert_eq!(with.status.code(), Some(0));
    assert_eq!(without.status.code(), Some(0));
    assert_eq!(with.stdout, without.stdout);
}

#[test]
fn no_inventory_files_is_refused_with_its_text() {
    let err =
        s3_inventory_bill::execute_paths(&[], &s3_inventory_bill::Options::default()).unwrap_err();
    assert_eq!(err.message(), "no inventory files");
}

#[test]
fn usage_error_escapes_raw_control_characters() {
    let (code, _, stderr) = run(&["--bogus\u{202E}\r\nx", "f"]);
    assert_eq!(code, 2);
    for raw in ['\u{202E}', '\r'] {
        assert!(!stderr.contains(raw), "{stderr:?}");
    }
    assert!(stderr.contains("\\u{202e}"), "{stderr:?}");
    assert!(stderr.contains("\\r\\n"), "{stderr:?}");
}

#[cfg(unix)]
#[test]
fn files_over_the_cap_are_refused_by_name_and_size() {
    let scratch = Scratch::new();
    let big = scratch.path().join("big.csv");
    let file = fs::File::create(&big).unwrap();
    file.set_len(1_073_741_825).unwrap();
    drop(file);
    let (code, stdout, stderr) = run(&[big.to_str().unwrap()]);
    assert_eq!(code, 2, "{stderr}");
    assert!(stdout.is_empty());
    assert!(
        stderr.contains("big.csv: file is larger than 1073741824 bytes"),
        "{stderr}"
    );
    let inv = scratch.path().join("inv.csv");
    fs::write(&inv, "Bucket,Key,Size,StorageClass\nb,k,1,STANDARD\n").unwrap();
    let (code, stdout, stderr) = run(&["--prices", big.to_str().unwrap(), inv.to_str().unwrap()]);
    assert_eq!(code, 2, "{stderr}");
    assert!(stdout.is_empty());
    assert!(
        stderr.contains("big.csv: file is larger than 1048576 bytes"),
        "{stderr}"
    );
}

#[test]
fn price_file_that_is_not_an_object_names_the_price_file() {
    let scratch = Scratch::new();
    let prices = scratch.path().join("list.json");
    fs::write(&prices, "[1,2]").unwrap();
    let (code, stdout, stderr) = run(&[
        "--prices",
        prices.to_str().unwrap(),
        "examples/inventory.csv",
    ]);
    assert_eq!(code, 2, "{stderr}");
    assert!(stdout.is_empty());
    assert!(stderr.contains("price file "), "{stderr}");
    assert!(stderr.contains("list.json: expected an object"), "{stderr}");
}

#[test]
fn bad_bucket_row_is_named_by_row_through_the_binary() {
    let scratch = Scratch::new();
    let path = scratch.path().join("nobucket.csv");
    fs::write(&path, "Bucket,Key,Size,StorageClass\n,k,1,STANDARD\n").unwrap();
    let (code, stdout, stderr) = run(&[path.to_str().unwrap()]);
    assert_eq!(code, 2, "{stderr}");
    assert!(stdout.is_empty());
    assert!(stderr.contains("row 2: empty Bucket"), "{stderr}");
}

#[test]
fn csv_quote_errors_and_bad_gzip_name_the_file() {
    let scratch = Scratch::new();
    let head = "Bucket,Key,Size,StorageClass\n";
    let cases = [
        (
            "mid.csv",
            format!("{head}b,k\"x,1,STANDARD\n"),
            "mid.csv: row 2: quote in an unquoted field",
        ),
        (
            "open.csv",
            format!("{head}b,\"k,1,STANDARD\n"),
            "open.csv: row 2: unterminated quote",
        ),
    ];
    for (name, body, want) in cases {
        let path = scratch.path().join(name);
        fs::write(&path, body).unwrap();
        let (code, stdout, stderr) = run(&[path.to_str().unwrap()]);
        assert_eq!(code, 2, "{stderr}");
        assert!(stdout.is_empty());
        assert!(stderr.contains(want), "{stderr}");
    }
    let bad = scratch.path().join("bad.csv.gz");
    fs::write(&bad, [0x1f, 0x8b, 0, 0]).unwrap();
    let (code, _, stderr) = run(&[bad.to_str().unwrap()]);
    assert_eq!(code, 2, "{stderr}");
    assert!(
        stderr.contains("bad.csv.gz: not a gzip inventory"),
        "{stderr}"
    );
}

#[cfg(target_os = "linux")]
#[test]
fn stdout_set_to_dev_null_by_the_parent_is_not_an_error() {
    // A parent that hands the child /dev/null as stdout (as Python's
    // `subprocess.DEVNULL` does) must see exit 0 and no message.
    let out = command(&["examples/inventory.csv"])
        .stdout(std::process::Stdio::null())
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(0));
    assert!(out.stderr.is_empty(), "{:?}", out.stderr);
}

#[test]
fn as_of_day_is_the_whole_day_and_the_next_day_is_not_eligible() {
    let scratch = Scratch::new();
    let head = "Bucket,Key,Size,LastModifiedDate,StorageClass\n";
    let line = |name: &str, date: &str| {
        let path = scratch.path().join(name);
        fs::write(&path, format!("{head}b,k,1073741824,{date},STANDARD\n")).unwrap();
        path
    };
    let eligible = |path: &Path| {
        let (code, stdout, stderr) = run(&["--as-of", "2024-01-15", path.to_str().unwrap()]);
        assert_eq!(code, 0, "{stderr}");
        stdout
            .lines()
            .find(|l| l.starts_with("STANDARD_IA "))
            .unwrap()
            .to_string()
    };
    let same_midnight = eligible(&line("a.csv", "2024-01-15T00:00:00.000Z"));
    assert!(
        same_midnight.contains("eligible_objects=1"),
        "{same_midnight}"
    );
    let same_afternoon = eligible(&line("b.csv", "2024-01-15T15:00:00.000Z"));
    assert!(
        same_afternoon.contains("eligible_objects=1"),
        "{same_afternoon}"
    );
    // Only the date and the separator are read; the rest of the field is not checked.
    let odd_time = eligible(&line("d.csv", "2024-01-15T99:99:99.000Z"));
    assert!(odd_time.contains("eligible_objects=1"), "{odd_time}");
    let next_day = eligible(&line("c.csv", "2024-01-16T00:00:00.000Z"));
    assert!(next_day.contains("eligible_objects=0"), "{next_day}");
}

#[test]
fn as_of_help_names_the_day_rule_in_help_readme_and_man() {
    let (_, help, _) = run(&["--help"]);
    assert!(help.contains("on or before that day"), "{help}");
    assert!(!help.contains("midnight"), "{help}");
    for doc in ["README.md", "man/s3-inventory-bill.1"] {
        let text = fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join(doc)).unwrap();
        assert!(text.contains("on or before that day"), "{doc}");
        assert!(!text.contains("midnight"), "{doc}");
    }
}

#[test]
fn text_report_keeps_warnings_on_stdout_and_stderr_empty() {
    let dir = Scratch::new();
    let inv = dir.path().join("inv.csv");
    fs::write(&inv, "Bucket,Key,Size,StorageClass\nb,a/k,5,STANDARD\n").unwrap();
    let (code, stdout, stderr) = run(&["--as-of", "2026-11-09", inv.to_str().unwrap()]);
    assert_eq!(code, 0, "{stderr}");
    assert!(stderr.is_empty(), "{stderr}");
    assert!(
        stdout.contains("\nwarning: LastModifiedDate column is absent\n"),
        "{stdout}"
    );
}

#[test]
fn cr_only_file_says_the_line_ends_are_the_cause() {
    let dir = Scratch::new();
    let cr = dir.path().join("cr.csv");
    fs::write(&cr, b"Bucket,Key,Size,StorageClass\rb,k,5,STANDARD\r").unwrap();
    let (code, stdout, stderr) = run(&[cr.to_str().unwrap()]);
    assert_eq!(code, 2);
    assert!(stdout.is_empty());
    assert!(stderr.contains("only CR line ends"), "{stderr}");
    // A real header problem on an LF file does not get that hint.
    let lf = dir.path().join("lf.csv");
    fs::write(&lf, b"Bucket,Key,Size\nb,k,5\n").unwrap();
    let (code, _, stderr) = run(&[lf.to_str().unwrap()]);
    assert_eq!(code, 2);
    assert!(stderr.contains("missing column StorageClass"), "{stderr}");
    assert!(!stderr.contains("CR line ends"), "{stderr}");
}

#[cfg(unix)]
#[test]
fn inventory_fifo_is_refused_without_reading() {
    let scratch = Scratch::new();
    let fifo = scratch.path().join("inv.fifo");
    let made = Command::new("mkfifo").arg(&fifo).status().unwrap();
    assert!(made.success());
    let (code, stdout, stderr) = run(&[fifo.to_str().unwrap()]);
    assert_eq!(code, 2);
    assert!(stdout.is_empty());
    assert!(stderr.contains("inv.fifo: not a file"), "{stderr}");
}

#[test]
fn a_file_error_names_the_file_with_one_file_too() {
    let scratch = Scratch::new();
    let bad = scratch.path().join("only.csv");
    fs::write(&bad, "Bucket,Key,Size,StorageClass\nb,k,x,STANDARD\n").unwrap();
    let (code, stdout, stderr) = run(&[bad.to_str().unwrap()]);
    assert_eq!(code, 2);
    assert!(stdout.is_empty());
    assert!(stderr.contains("only.csv: row 2: invalid Size"), "{stderr}");
}

fn prices_with_standard_tiers(first: &str, next: &str, over: &str) -> String {
    let mut text = include_str!("../prices/s3-storage.json").to_string();
    for (old, new) in [
        ("\"standard_first_50_tb\": \"0.023\"", first),
        ("\"standard_next_450_tb\": \"0.022\"", next),
        ("\"standard_over_500_tb\": \"0.021\"", over),
    ] {
        assert!(text.contains(old), "{old}");
        let key = old.split(':').next().unwrap();
        text = text.replacen(old, &format!("{key}: \"{new}\""), 1);
    }
    text
}

#[test]
fn large_valid_totals_with_a_high_rate_are_not_refused_for_an_intermediate_loss() {
    let scratch = Scratch::new();
    let cases = [
        (
            "2688123546345455679",
            "7628359478944001404",
            ["1.1882", "1.1887", "1.1831"],
            "estimated_usd_month=11367196327.415037670449633151",
        ),
        (
            "1000000000000000007",
            "1000000000000000013",
            ["200", "100", "50"],
            "estimated_usd_month=93162977461.54785249382257462",
        ),
    ];
    for (first, second, rates, expected) in cases {
        let prices = scratch.path().join("prices.json");
        fs::write(
            &prices,
            prices_with_standard_tiers(rates[0], rates[1], rates[2]),
        )
        .unwrap();
        let inv = scratch.path().join("inv.csv");
        fs::write(
            &inv,
            format!(
                "Bucket,Key,Size,LastModifiedDate,StorageClass\n\
                 b,p0/k,{first},2024-01-01,STANDARD\n\
                 b,p1/k,{second},2024-01-01,STANDARD\n"
            ),
        )
        .unwrap();
        for json in [false, true] {
            let mut args = vec!["--prices", prices.to_str().unwrap()];
            if json {
                args.push("--json");
            }
            args.push(inv.to_str().unwrap());
            let (code, stdout, stderr) = run(&args);
            assert_eq!(code, 0, "{stderr}");
            assert!(stderr.is_empty(), "{stderr}");
            if !json {
                assert!(stdout.contains(expected), "{stdout}");
            }
        }
    }
}

#[test]
fn a_quoted_record_has_the_same_column_limit_as_a_plain_one() {
    let scratch = Scratch::new();
    let inv = scratch.path().join("inv.csv");
    for quoted in [false, true] {
        let cell = |text: &str| {
            if quoted {
                format!("\"{text}\"")
            } else {
                text.to_string()
            }
        };
        let mut header = ["Bucket", "Key", "Size", "StorageClass"]
            .map(&cell)
            .to_vec();
        let mut row = ["b", "k", "5", "STANDARD"].map(&cell).to_vec();
        for n in 0..100 {
            header.push(cell(&format!("x{n}")));
            row.push(cell("y"));
        }
        fs::write(&inv, format!("{}\n{}\n", header.join(","), row.join(","))).unwrap();
        let (code, stdout, stderr) = run(&[inv.to_str().unwrap()]);
        assert_eq!(code, 2, "quoted={quoted}: {stdout}{stderr}");
        assert!(stdout.is_empty());
        assert!(stderr.contains("too many columns"), "{stderr}");
    }
}

#[test]
fn size_with_leading_zeros_is_read_whatever_its_length() {
    let scratch = Scratch::new();
    let inv = scratch.path().join("inv.csv");
    fs::write(
        &inv,
        format!(
            "Bucket,Key,Size,StorageClass\nb,k,{}5,STANDARD\nb,j,0000000000000000000000,STANDARD\n",
            "0".repeat(40)
        ),
    )
    .unwrap();
    let (code, stdout, stderr) = run(&[inv.to_str().unwrap()]);
    assert_eq!(code, 0, "{stderr}");
    assert!(stdout.contains("raw_bytes=5"), "{stdout}");
}

const HEAD: &str = "Bucket,Key,Size,StorageClass\n";

fn run_inventory(scratch: &Scratch, extra: &[&str], text: &[u8]) -> (i32, String, String) {
    let inv = scratch.path().join("inv.csv");
    fs::write(&inv, text).unwrap();
    let mut args: Vec<&str> = extra.to_vec();
    args.push(inv.to_str().unwrap());
    run(&args)
}

#[test]
fn readme_input_rules_hold_for_white_space_class_names_nul_and_sniffing() {
    let scratch = Scratch::new();
    // Form feed is white space, a vertical tab is not.
    let (code, stdout, stderr) = run_inventory(
        &scratch,
        &[],
        format!("{HEAD}b,k,\u{c}5,STANDARD\n").as_bytes(),
    );
    assert_eq!(code, 0, "{stderr}");
    assert!(stdout.contains("raw_bytes=5"), "{stdout}");
    let (code, _, stderr) = run_inventory(
        &scratch,
        &[],
        format!("{HEAD}b,k,\u{b}5,STANDARD\n").as_bytes(),
    );
    assert_eq!(code, 2);
    assert!(stderr.contains("invalid Size"), "{stderr}");
    // Class names are not case sensitive; an unknown class is shown in capitals.
    let (code, stdout, _) = run_inventory(
        &scratch,
        &[],
        format!("{HEAD}b,k,5,standard\nb,j,7,Standard_IA\nb,i,9,cls\n").as_bytes(),
    );
    assert_eq!(code, 0);
    assert!(stdout.contains("STANDARD objects=1"), "{stdout}");
    assert!(stdout.contains("STANDARD_IA objects=1"), "{stdout}");
    assert!(stdout.contains("CLS objects=1"), "{stdout}");
    // A NUL byte in the first 512 bytes is refused; a later one is part of the key.
    let (code, _, stderr) = run_inventory(
        &scratch,
        &[],
        format!("{HEAD}b,k\0,5,STANDARD\n").as_bytes(),
    );
    assert_eq!(code, 2);
    assert!(stderr.contains("not a CSV inventory"), "{stderr}");
    let mut late = HEAD.to_string();
    for _ in 0..100 {
        late.push_str("b,k,1,STANDARD\n");
    }
    late.push_str("b,j\0,5,STANDARD\n");
    let (code, stdout, stderr) = run_inventory(&scratch, &[], late.as_bytes());
    assert_eq!(code, 0, "{stderr}");
    assert!(stdout.contains("j\\0 objects=1"), "{stdout}");
    // A header that starts like an ORC file is refused as ORC.
    let (code, _, stderr) = run_inventory(
        &scratch,
        &[],
        b"ORC_ID,Bucket,Key,Size,StorageClass\n1,b,k,5,STANDARD\n",
    );
    assert_eq!(code, 2);
    assert!(stderr.contains("orc inventories are not read"), "{stderr}");
    // Anything after the last gzip member, even zeros, is refused.
    let mut gz = Vec::new();
    {
        use flate2::{write::GzEncoder, Compression};
        use std::io::Write;
        let mut enc = GzEncoder::new(&mut gz, Compression::fast());
        enc.write_all(format!("{HEAD}b,k,1,STANDARD\n").as_bytes())
            .unwrap();
        enc.finish().unwrap();
    }
    let (code, _, _) = run_inventory(&scratch, &[], &gz);
    assert_eq!(code, 0);
    gz.extend_from_slice(&[0, 0, 0, 0]);
    let (code, _, stderr) = run_inventory(&scratch, &[], &gz);
    assert_eq!(code, 2);
    assert!(stderr.contains("not a gzip inventory"), "{stderr}");
    // `--prefix-depth` accepts a leading zero and a plus sign.
    let ok = format!("{HEAD}b,a/b/c,1,STANDARD\n");
    for depth in ["08", "+3"] {
        let (code, _, stderr) = run_inventory(&scratch, &["--prefix-depth", depth], ok.as_bytes());
        assert_eq!(code, 0, "{depth}: {stderr}");
    }
}

#[test]
fn readme_number_limits_hold_for_sizes_prices_days_and_dates() {
    let scratch = Scratch::new();
    let sample = "examples/inventory.csv";
    let base = include_str!("../prices/s3-storage.json");
    let priced = |old: &str, new: &str| {
        assert!(base.contains(old), "{old}");
        let path = scratch.path().join("p.json");
        fs::write(&path, base.replacen(old, new, 1)).unwrap();
        run(&["--prices", path.to_str().unwrap(), sample])
    };
    let first = "\"standard_first_50_tb\": \"0.023\"";
    for (value, ok) in [
        ("79228162514264337593543950335", true),
        ("79228162514264337593543950336", false),
        ("9.9999999999999999999999999999", false),
        ("1.0000000000000000000000000001", true),
        ("1e5", false),
    ] {
        let (code, _, stderr) = priced(first, &format!("\"standard_first_50_tb\": \"{value}\""));
        // A huge rate is a valid price file; the bill may still not fit.
        if ok {
            assert!(
                code == 0 || stderr.contains("amount does not fit"),
                "{value}: {stderr}"
            );
        } else {
            assert_eq!(code, 2, "{value}");
            assert!(stderr.contains("bad standard_first_50_tb"), "{stderr}");
        }
    }
    for (days, ok) in [("0", false), ("1", true), ("36500", true), ("36501", false)] {
        let (code, _, stderr) = priced("\"ia_min_days\": 30", &format!("\"ia_min_days\": {days}"));
        assert_eq!(code == 0, ok, "{days}: {stderr}");
    }
    for (bytes, ok) in [
        ("0", true),
        ("18446744073709551615", true),
        ("131072.0", false),
        ("\"131072\"", false),
        ("-1", false),
    ] {
        let (code, _, stderr) = priced(
            "\"min_billable_bytes\": 131072",
            &format!("\"min_billable_bytes\": {bytes}"),
        );
        assert_eq!(code == 0, ok, "{bytes}: {stderr}");
    }
    for (day, ok) in [
        ("0000-01-01", true),
        ("9999-12-31", true),
        ("10000-01-01", false),
        ("2026-02-29", false),
    ] {
        let (code, _, _) = run(&["--as-of", day, sample]);
        assert_eq!(code == 0, ok, "{day}");
    }
    // The 64-bit byte total: the largest size plus an empty object is fine,
    // and with `--as-of` the empty object counts as the minimum size.
    let text = b"Bucket,Key,Size,LastModifiedDate,StorageClass\n\
                 b,a/k,18446744073709551615,2024-01-01,STANDARD\n\
                 b,a/k2,0,2024-01-01,STANDARD\n";
    let (code, stdout, stderr) = run_inventory(&scratch, &[], text);
    assert_eq!(code, 0, "{stderr}");
    assert!(
        stdout.contains("raw_bytes=18446744073709551615"),
        "{stdout}"
    );
    let (code, _, stderr) = run_inventory(&scratch, &["--as-of", "2026-01-01"], text);
    assert_eq!(code, 2);
    assert!(stderr.contains("byte total does not fit"), "{stderr}");
    // 3 GiB at 3.3333333333333333333333333333 is 9.9999999999999999999999999999
    // exactly. That value does not fit, and the one rounding shows 10.00.
    let (code, stdout, stderr) = {
        let path = scratch.path().join("three.json");
        fs::write(
            &path,
            base.replacen(
                first,
                "\"standard_first_50_tb\": \"3.3333333333333333333333333333\"",
                1,
            ),
        )
        .unwrap();
        let inv = scratch.path().join("three.csv");
        fs::write(&inv, format!("{HEAD}b,k,3221225472,STANDARD\n")).unwrap();
        run(&["--prices", path.to_str().unwrap(), inv.to_str().unwrap()])
    };
    assert_eq!(code, 0, "{stderr}");
    assert!(
        stdout.contains("total objects=1 raw_bytes=3221225472 estimated_usd_month=10.00"),
        "{stdout}"
    );
}

#[test]
fn gzip_with_very_many_members_is_refused_in_a_moment() {
    use flate2::{write::GzEncoder, Compression};
    use std::io::Write;
    let scratch = Scratch::new();
    let member = |bytes: &[u8]| {
        let mut enc = GzEncoder::new(Vec::new(), Compression::fast());
        enc.write_all(bytes).unwrap();
        enc.finish().unwrap()
    };
    let mut data = member(b"Bucket,Key,Size,StorageClass\nb,a/k,5,STANDARD\n");
    let empty = member(b"");
    for _ in 0..200_000 {
        data.extend_from_slice(&empty);
    }
    let path = scratch.path().join("many.csv.gz");
    fs::write(&path, &data).unwrap();
    let start = std::time::Instant::now();
    let (code, stdout, stderr) = run(&[path.to_str().unwrap()]);
    assert_eq!(code, 2, "{stdout}{stderr}");
    assert!(stdout.is_empty());
    assert!(stderr.contains("more than 100000 members"), "{stderr}");
    // The debug build needs about 5 s for 100,000 members; the release build 0.5 s.
    assert!(start.elapsed().as_secs() < 30, "took {:?}", start.elapsed());
    // A few members are still read as one file.
    let mut few = member(b"Bucket,Key,Size,StorageClass\nb,a/k,5,STANDARD\n");
    few.extend_from_slice(&member(b"b,a/k2,7,STANDARD\n"));
    fs::write(&path, &few).unwrap();
    let (code, stdout, stderr) = run(&[path.to_str().unwrap()]);
    assert_eq!(code, 0, "{stderr}");
    assert!(stdout.contains("total objects=2 raw_bytes=12"), "{stdout}");
}

#[test]
fn text_after_a_closing_quote_is_refused_and_a_quoted_field_still_reads() {
    let scratch = Scratch::new();
    let inv = scratch.path().join("q.csv");
    for (row, line) in [
        ("b,k,\"1\"0,STANDARD", 2),
        ("b,\"a\"b/x,10,STANDARD", 2),
        ("b,k,10,\"STANDARD\" ", 2),
    ] {
        fs::write(&inv, format!("Bucket,Key,Size,StorageClass\n{row}\n")).unwrap();
        let (code, stdout, stderr) = run(&[inv.to_str().unwrap()]);
        assert_eq!(code, 2, "{row}: {stdout}{stderr}");
        assert!(stdout.is_empty());
        assert!(
            stderr.contains(&format!("row {line}: text after a closing quote")),
            "{row}: {stderr}"
        );
        assert!(!stderr.contains("STANDARD\""), "{stderr}");
    }
    // Quoted fields, doubled quotes, CRLF and a quoted last field still read.
    fs::write(
        &inv,
        "\"Bucket\",\"Key\",\"Size\",\"StorageClass\"\r\n\"b\",\"a\"\"x/k\",\"10\",\"STANDARD\"\r\nb,\"p/q\",5,\"STANDARD\"",
    )
    .unwrap();
    let (code, stdout, stderr) = run(&[inv.to_str().unwrap()]);
    assert_eq!(code, 0, "{stderr}");
    assert!(stdout.contains("total objects=2 raw_bytes=15"), "{stdout}");
}

#[test]
fn cr_only_file_is_refused_also_when_the_last_column_is_optional() {
    let scratch = Scratch::new();
    let cr = scratch.path().join("cr.csv");
    for last in ["LastModifiedDate", "IntelligentTieringAccessTier"] {
        fs::write(
            &cr,
            format!("Bucket,Key,Size,StorageClass,{last}\rb,k,5,STANDARD,2024-01-15\rb,k2,7,STANDARD,2024-01-15\r"),
        )
        .unwrap();
        let (code, stdout, stderr) = run(&["--as-of", "2026-11-09", cr.to_str().unwrap()]);
        assert_eq!(code, 2, "{last}: {stdout}{stderr}");
        assert!(stdout.is_empty());
        assert!(stderr.contains("only CR line ends"), "{stderr}");
    }
}

#[test]
fn a_price_file_over_one_mebibyte_is_refused_and_one_at_the_limit_loads() {
    let scratch = Scratch::new();
    let json = include_str!("../prices/s3-storage.json");
    let path = scratch.path().join("p.json");
    let padded = |len: usize| {
        let mut text = json.to_string();
        text.push_str(&" ".repeat(len - text.len()));
        text
    };
    fs::write(&path, padded(1_048_576)).unwrap();
    let (code, _, stderr) = run(&["--prices", path.to_str().unwrap(), "examples/inventory.csv"]);
    assert_eq!(code, 0, "{stderr}");
    fs::write(&path, padded(1_048_577)).unwrap();
    let (code, stdout, stderr) =
        run(&["--prices", path.to_str().unwrap(), "examples/inventory.csv"]);
    assert_eq!(code, 2, "{stdout}");
    assert!(stdout.is_empty());
    assert!(
        stderr.contains("file is larger than 1048576 bytes"),
        "{stderr}"
    );
}

const ESTIMATE_LINE: &str =
    "Estimate from list prices, not your AWS invoice (free tier, credits, discounts not included)";

#[test]
fn monthly_cost_is_labelled_as_an_estimate() {
    let (code, stdout, stderr) = run(&["--as-of", "2026-11-09", "examples/inventory.csv"]);
    assert_eq!(code, 0, "{stderr}");
    assert!(stdout.contains("STANDARD objects=1 raw_bytes=1073741824 billable_bytes=1073741824 estimated_usd_month=0.023"));
    assert!(stdout.contains("total objects=1 raw_bytes=1073741824 estimated_usd_month=0.023"));
    assert!(stdout.contains("logs/ objects=1 raw_bytes=1073741824 estimated_usd_month=0.023"));
    assert!(!stdout.contains(" usd_month="), "{stdout}");
    assert_eq!(stdout.matches(ESTIMATE_LINE).count(), 1, "{stdout}");
    let (code, stdout, stderr) = run(&["--json", "examples/inventory.csv"]);
    assert_eq!(code, 0, "{stderr}");
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(parsed["estimate"], true);
    assert_eq!(parsed["estimated_usd_month"], "0.023");
    assert_eq!(parsed["classes"][0]["estimated_usd_month"], "0.023");
    assert_eq!(parsed["prefixes"][0]["estimated_usd_month"], "0.023");
    assert!(parsed.get("usd_month").is_none());
    assert!(parsed["classes"][0].get("usd_month").is_none());
}

fn prices_dated(dir: &Scratch, date: &str) -> String {
    let text = fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/prices/s3-storage.json"
    ))
    .unwrap()
    .replacen(
        "\"source_date\": \"2026-11-09\"",
        &format!("\"source_date\": \"{date}\""),
        1,
    );
    assert!(text.contains(date));
    let path = dir.path().join(format!("prices-{date}.json"));
    fs::write(&path, text).unwrap();
    path.to_str().unwrap().to_string()
}

#[test]
fn old_price_data_warns_after_180_days_and_still_exits_0() {
    let dir = Scratch::new();
    // 2026-11-09 minus 180 days is 2026-05-13: still fine. One day older warns.
    let edge = prices_dated(&dir, "2026-05-13");
    let old = prices_dated(&dir, "2026-05-12");
    let (code, stdout, stderr) = run(&[
        "--prices",
        &edge,
        "--as-of",
        "2026-11-09",
        "examples/inventory.csv",
    ]);
    assert_eq!(code, 0, "{stderr}");
    assert!(!stdout.contains("older than 180 days"), "{stdout}");
    assert!(stderr.is_empty(), "{stderr}");
    let (code, stdout, stderr) = run(&[
        "--prices",
        &old,
        "--as-of",
        "2026-11-09",
        "examples/inventory.csv",
    ]);
    assert_eq!(code, 0, "{stderr}");
    assert!(
        stdout.contains(
            "warning: price data dated 2026-05-12 is more than 180 days older than --as-of"
        ),
        "{stdout}"
    );
    let (code, stdout, stderr) = run(&[
        "--json",
        "--prices",
        &old,
        "--as-of",
        "2026-11-09",
        "examples/inventory.csv",
    ]);
    assert_eq!(code, 0, "{stderr}");
    assert!(
        stderr.contains(
            "warning: price data dated 2026-05-12 is more than 180 days older than --as-of"
        ),
        "{stderr}"
    );
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(parsed["warnings"].as_array().unwrap().len(), 1);
    // Without --as-of the day is today, here fixed by SOURCE_DATE_EPOCH to 2026-11-20.
    let (code, stdout, _) = run(&["--prices", &old, "examples/inventory.csv"]);
    assert_eq!(code, 0);
    assert!(
        stdout.contains(
            "warning: price data dated 2026-05-12 is more than 180 days older than today"
        ),
        "{stdout}"
    );
    let fresh = prices_dated(&dir, "2026-05-24");
    let (_, stdout, _) = run(&["--prices", &fresh, "examples/inventory.csv"]);
    assert!(!stdout.contains("warning:"), "{stdout}");
    let stale = prices_dated(&dir, "2026-05-23");
    let (_, stdout, _) = run(&["--prices", &stale, "examples/inventory.csv"]);
    assert!(
        stdout.contains("dated 2026-05-23 is more than 180 days older than today"),
        "{stdout}"
    );
    // The built-in list (2026-11-09) is 11 days old on the fixed day.
    let (_, stdout, _) = run(&["examples/inventory.csv"]);
    assert!(!stdout.contains("warning:"), "{stdout}");
}
