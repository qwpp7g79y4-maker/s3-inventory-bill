//! Every JSON output starts with `"schema_version": 1`. The errors that
//! `--json` can meet print no JSON at all (the message goes to standard error,
//! exit 2), so no error path can give a document without the field.

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

fn has_schema_version(json: &str) -> bool {
    let parsed: serde_json::Value = serde_json::from_str(json).unwrap();
    let object = parsed.as_object().unwrap();
    object.keys().next().map(String::as_str) == Some("schema_version")
        && object["schema_version"] == 1
        && object["schema_version"].is_u64()
}

#[test]
fn every_json_path_has_schema_version_first() {
    let dir = std::env::temp_dir().join(format!("s3-inventory-bill-schema-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let empty = dir.join("empty.csv");
    fs::write(&empty, "Bucket,Key,Size,StorageClass\n").unwrap();
    let unpriced = dir.join("unpriced.csv");
    fs::write(
        &unpriced,
        "Bucket,Key,Size,StorageClass\nb,a/k,4,COLD_CUSTOM\n",
    )
    .unwrap();
    let escaped = dir.join("escaped.csv");
    fs::write(
        &escaped,
        "Bucket,Key,Size,StorageClass\nb,\"x\ny\u{1b}/\",4,STANDARD\n",
    )
    .unwrap();
    let out_file = dir.join("report.json");
    let (empty, unpriced, escaped) = (
        empty.to_str().unwrap(),
        unpriced.to_str().unwrap(),
        escaped.to_str().unwrap(),
    );
    let inv = "examples/inventory.csv";
    let cases: Vec<Vec<&str>> = vec![
        vec!["--json", inv],
        vec!["--json", "--as-of", "2026-11-09", inv],
        vec!["--json", "--prefix-depth", "3", inv],
        vec!["--json", "--quiet", inv],
        vec!["--json", "--verbose", inv],
        vec!["--json", "--prices", "prices/s3-storage.json", inv],
        vec!["--json", empty],
        vec!["--json", "--as-of", "2026-11-09", unpriced],
        vec!["--json", escaped],
        vec!["--json", inv, inv],
    ];
    for args in &cases {
        let (code, stdout, stderr) = run(args);
        assert_eq!(code, 0, "{args:?}: {stderr}");
        assert!(has_schema_version(&stdout), "{args:?}: {stdout}");
    }
    // The same document goes to a file with --output.
    let target = out_file.to_str().unwrap();
    let (code, _, stderr) = run(&["--json", "--output", target, inv]);
    assert_eq!(code, 0, "{stderr}");
    assert!(has_schema_version(&fs::read_to_string(&out_file).unwrap()));
    // The library renders the same field.
    let bill = s3_inventory_bill::bill_many(
        &[fs::read(inv).unwrap().as_slice()],
        &s3_inventory_bill::prices_builtin(),
        None,
        1,
    )
    .unwrap();
    assert!(has_schema_version(&s3_inventory_bill::render(&bill, true)));
    // Error paths that were asked for JSON print no JSON, only the message.
    for args in [
        vec!["--json", "no-such-file.csv"],
        vec!["--json", "--as-of", "2026-13-40", inv],
        vec!["--json", "--prices", "no-such-prices.json", inv],
        vec!["--json", "--prefix-depth", "0", inv],
        vec!["--json", "--md", inv],
        vec!["--json", "--quiet", "--verbose", inv],
    ] {
        let (code, stdout, stderr) = run(&args);
        assert_eq!(code, 2, "{args:?}: {stderr}");
        assert!(stdout.is_empty(), "{args:?}: {stdout}");
        assert!(!stderr.is_empty());
    }
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn schema_version_is_documented_in_the_readme_and_quality() {
    for name in ["README.md", "QUALITY.md"] {
        let text = fs::read_to_string(format!("{}/{name}", env!("CARGO_MANIFEST_DIR"))).unwrap();
        assert!(text.contains("schema_version"), "{name}");
    }
}
