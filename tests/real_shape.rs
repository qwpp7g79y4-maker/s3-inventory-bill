//! The input as S3 Inventory documents it, run through the binary.
//!
//! Source of the shape: the S3 Inventory chapter of the Amazon S3 User Guide,
//! <https://docs.aws.amazon.com/AmazonS3/latest/userguide/storage-inventory.html>
//! (the list of fields, and the `fileSchema` line of `manifest.json`). A data
//! file there has no header line, every field is double-quoted, and a key is
//! URL-encoded. The tool needs a header line, so each file below gets the
//! `fileSchema` names as its first row, as the README says.
//!
//! The shape is reproduced from what is documented, with no network.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use flate2::write::GzEncoder;
use flate2::Compression;

/// `fileSchema` of a manifest that lists every optional field the docs name.
const FULL_SCHEMA: &str = "Bucket, Key, VersionId, IsLatest, IsDeleteMarker, Size, LastModifiedDate, ETag, StorageClass, IsMultipartUploaded, ReplicationStatus, EncryptionStatus, ObjectLockRetainUntilDate, ObjectLockMode, ObjectLockLegalHoldStatus, IntelligentTieringAccessTier, BucketKeyStatus, ChecksumAlgorithm";

const GIB: u64 = 1_073_741_824;

struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        static N: AtomicU64 = AtomicU64::new(0);
        let n = N.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "s3-inventory-bill-shape-{}-{n}",
            std::process::id()
        ));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn write(&self, name: &str, bytes: &[u8]) -> PathBuf {
        let path = self.0.join(name);
        fs::write(&path, bytes).unwrap();
        path
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// The header row a reader adds: the `fileSchema` names, without the spaces.
fn header(schema: &str) -> String {
    schema.replace(", ", ",")
}

/// One documented data row: every field in double quotes.
fn full_row(key: &str, size: u64, modified: &str, class: &str, tier: &str) -> String {
    let fields = [
        "example-bucket",
        key,
        "",
        "true",
        "false",
        &size.to_string(),
        modified,
        "d41d8cd98f00b204e9800998ecf8427e",
        class,
        "false",
        "",
        "SSE-S3",
        "",
        "",
        "",
        tier,
        "DISABLED",
        "",
    ];
    assert_eq!(fields.len(), FULL_SCHEMA.split(", ").count());
    let quoted: Vec<String> = fields.iter().map(|f| format!("\"{f}\"")).collect();
    format!("{}\n", quoted.join(","))
}

fn run(args: &[&Path]) -> (i32, String, String) {
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

fn gzip(bytes: &[u8]) -> Vec<u8> {
    let mut encoder = GzEncoder::new(Vec::new(), Compression::fast());
    encoder.write_all(bytes).unwrap();
    encoder.finish().unwrap()
}

#[test]
fn full_schema_with_quoted_fields_and_url_encoded_keys() {
    let dir = Scratch::new();
    let mut csv = header(FULL_SCHEMA) + "\n";
    csv += &full_row(
        "photos/2024/summer%20trip/IMG_0001.jpg",
        GIB,
        "2024-01-15T10:20:30.000Z",
        "STANDARD",
        "",
    );
    csv += &full_row(
        "logs/app/a%2Cb%22c.log",
        2 * GIB,
        "2024-02-01T00:00:00.000Z",
        "STANDARD_IA",
        "",
    );
    csv += &full_row(
        "archive/caf%C3%A9.tar",
        GIB,
        "2023-12-31T23:59:59.000Z",
        "GLACIER_IR",
        "",
    );
    csv += &full_row(
        "zone/one.bin",
        GIB,
        "2024-03-03T03:03:03.000Z",
        "ONEZONE_IA",
        "",
    );
    let file = dir.write("inventory.csv", csv.as_bytes());
    let (code, stdout, stderr) = run(&[&file]);
    assert_eq!(code, 0, "{stderr}");
    assert!(stderr.is_empty(), "{stderr}");
    // 1 GiB at 0.023, 2 GiB at 0.0125, 1 GiB at 0.004, 1 GiB at 0.01.
    assert!(stdout.contains("STANDARD objects=1 raw_bytes=1073741824 billable_bytes=1073741824 estimated_usd_month=0.023\n"), "{stdout}");
    assert!(stdout.contains("STANDARD_IA objects=1 raw_bytes=2147483648 billable_bytes=2147483648 estimated_usd_month=0.025\n"), "{stdout}");
    assert!(stdout.contains("GLACIER_IR objects=1 raw_bytes=1073741824 billable_bytes=1073741824 estimated_usd_month=0.004\n"), "{stdout}");
    assert!(stdout.contains("ONEZONE_IA objects=1 raw_bytes=1073741824 billable_bytes=1073741824 estimated_usd_month=0.01\n"), "{stdout}");
    assert!(
        stdout.contains("total objects=4 raw_bytes=5368709120 estimated_usd_month=0.062\n"),
        "{stdout}"
    );
    // Keys are used as written: the percent escapes stay.
    assert!(stdout.contains("photos/ objects=1"), "{stdout}");
    assert!(stdout.contains("logs/ objects=1"), "{stdout}");
    assert!(stdout.contains("archive/ objects=1"), "{stdout}");
    assert!(stdout.contains("zone/ objects=1"), "{stdout}");
    assert!(
        stdout.contains("omitted: pass --as-of YYYY-MM-DD"),
        "{stdout}"
    );
    assert!(!stdout.contains("warning:"), "{stdout}");
}

#[test]
fn optional_fields_missing_and_columns_in_another_order() {
    let dir = Scratch::new();
    let minimal = dir.write(
        "minimal.csv",
        format!(
            "Bucket,Key,Size,StorageClass\n\"b\",\"a/k1\",\"{GIB}\",\"STANDARD\"\n\"b\",\"a/k2\",\"{GIB}\",\"STANDARD\"\n"
        )
        .as_bytes(),
    );
    let (code, stdout, stderr) = run(&[&minimal]);
    assert_eq!(code, 0, "{stderr}");
    assert!(
        stdout.contains("total objects=2 raw_bytes=2147483648 estimated_usd_month=0.046\n"),
        "{stdout}"
    );
    // The order of the columns is whatever the manifest says.
    let shuffled = dir.write(
        "shuffled.csv",
        format!("StorageClass,Size,Key,Bucket\nSTANDARD,{GIB},a/k1,b\nSTANDARD,{GIB},a/k2,b\n")
            .as_bytes(),
    );
    let (code, stdout2, stderr) = run(&[&shuffled]);
    assert_eq!(code, 0, "{stderr}");
    assert_eq!(stdout, stdout2);
    // No LastModifiedDate column: priced, and --as-of says savings cannot be shown.
    let (code, stdout, stderr) = run(&[&minimal, Path::new("--as-of=2026-11-09")]);
    assert_eq!(code, 0, "{stderr}");
    assert!(
        stdout.contains("warning: LastModifiedDate column is absent"),
        "{stdout}"
    );
    assert!(
        stdout.contains("total objects=2 raw_bytes=2147483648 estimated_usd_month=0.046\n"),
        "{stdout}"
    );
}

#[test]
fn empty_lists_give_a_zero_report() {
    let dir = Scratch::new();
    let only_header = dir.write("empty.csv", format!("{}\n", header(FULL_SCHEMA)).as_bytes());
    let (code, stdout, stderr) = run(&[&only_header]);
    assert_eq!(code, 0, "{stderr}");
    assert!(
        stdout.contains("total objects=0 raw_bytes=0 estimated_usd_month=0.00\n"),
        "{stdout}"
    );
    assert!(
        stdout.contains("storage by prefix\n\nlifecycle\n"),
        "{stdout}"
    );
}

#[test]
fn several_files_and_a_gzip_file_add_up_exactly() {
    let dir = Scratch::new();
    let one = header(FULL_SCHEMA)
        + "\n"
        + &full_row("a/1", GIB, "2024-01-01T00:00:00.000Z", "STANDARD", "");
    let two = header(FULL_SCHEMA)
        + "\n"
        + &full_row("b/2", GIB, "2024-01-01T00:00:00.000Z", "STANDARD", "")
        + &full_row("b/3", GIB, "2024-01-01T00:00:00.000Z", "STANDARD_IA", "");
    let first = dir.write("part-0.csv", one.as_bytes());
    let second = dir.write("part-1.csv.gz", &gzip(two.as_bytes()));
    let (code, stdout, stderr) = run(&[&first, &second]);
    assert_eq!(code, 0, "{stderr}");
    // 2 GiB Standard at 0.023, 1 GiB Standard-IA at 0.0125.
    assert!(
        stdout.contains("total objects=3 raw_bytes=3221225472 estimated_usd_month=0.0585\n"),
        "{stdout}"
    );
    assert!(stdout.contains("a/ objects=1"), "{stdout}");
    assert!(stdout.contains("b/ objects=2"), "{stdout}");
}

#[test]
fn keys_with_special_characters_stay_on_one_line_and_are_counted() {
    let dir = Scratch::new();
    let mut csv = header(FULL_SCHEMA) + "\n";
    // A literal comma, a doubled quote, a space, non-ASCII text and a raw newline
    // inside a quoted key. S3 encodes keys, but a reader must survive all of them.
    for key in [
        "dir with space/a",
        "comma,dir/b",
        "quote\"\"dir/c",
        "caf\u{e9}/d",
        "line\nbreak/e",
    ] {
        csv += &full_row(key, GIB, "2024-01-01T00:00:00.000Z", "STANDARD", "");
    }
    let file = dir.write("special.csv", csv.as_bytes());
    let (code, stdout, stderr) = run(&[&file]);
    assert_eq!(code, 0, "{stderr}");
    assert!(
        stdout.contains("total objects=5 raw_bytes=5368709120 estimated_usd_month=0.115\n"),
        "{stdout}"
    );
    assert!(stdout.contains("quote\"dir/ objects=1"), "{stdout}");
    assert!(stdout.contains("caf\u{e9}/ objects=1"), "{stdout}");
    assert!(stdout.contains("line\\nbreak/ objects=1"), "{stdout}");
    assert!(stdout
        .lines()
        .all(|line| !line.chars().any(char::is_control)));
}

#[test]
fn intelligent_tiering_access_tier_column_is_read() {
    let dir = Scratch::new();
    let csv = header(FULL_SCHEMA)
        + "\n"
        + &full_row(
            "it/a",
            GIB,
            "2024-01-01T00:00:00.000Z",
            "INTELLIGENT_TIERING",
            "FREQUENT",
        )
        + &full_row(
            "it/b",
            GIB,
            "2024-01-01T00:00:00.000Z",
            "INTELLIGENT_TIERING",
            "INFREQUENT",
        );
    let file = dir.write("it.csv", csv.as_bytes());
    let (code, stdout, stderr) = run(&[&file]);
    assert_eq!(code, 0, "{stderr}");
    // 0.023 + 0.0125 and 2 x 0.0000025 monitoring.
    assert!(
        stdout.contains("total objects=2 raw_bytes=2147483648 estimated_usd_month=0.035505\n"),
        "{stdout}"
    );
}
