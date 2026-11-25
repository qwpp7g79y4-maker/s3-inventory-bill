//! `--md`: the same amounts as the text report and the JSON, in tables.

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

/// The cells of one `| a | b |` row.
fn cells(line: &str) -> Vec<String> {
    let inner = line.trim().trim_start_matches('|').trim_end_matches('|');
    inner.split(" | ").map(|c| c.trim().to_string()).collect()
}

fn rows_after(md: &str, heading: &str) -> Vec<Vec<String>> {
    let start = md
        .find(heading)
        .unwrap_or_else(|| panic!("no {heading}\n{md}"));
    md[start..]
        .lines()
        .skip(1)
        .skip_while(|line| !line.starts_with('|'))
        .take_while(|line| line.starts_with('|'))
        .skip(2)
        .map(cells)
        .collect()
}

const INVENTORY: &str = "Bucket,Key,Size,LastModifiedDate,StorageClass\n\
b,logs/app/a.bin,1073741824,2024-01-15T00:00:00.000Z,STANDARD\n\
b,logs/app/b.bin,1073741824,2024-01-15T00:00:00.000Z,STANDARD_IA\n\
b,pics/c.bin,5,2024-01-15T00:00:00.000Z,COLD_CUSTOM\n";

#[test]
fn markdown_amounts_equal_text_and_json() {
    let dir = std::env::temp_dir().join(format!("s3-inventory-bill-md-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let inv = dir.join("inv.csv");
    fs::write(&inv, INVENTORY).unwrap();
    let inv = inv.to_str().unwrap();
    let (code, md, err) = run(&["--md", "--as-of", "2026-11-09", inv]);
    assert_eq!(code, 0, "{err}");
    assert!(err.is_empty(), "{err}");
    let (_, text, _) = run(&["--as-of", "2026-11-09", inv]);
    let (_, json, _) = run(&["--json", "--as-of", "2026-11-09", inv]);
    let json: serde_json::Value = serde_json::from_str(&json).unwrap();

    assert!(md.starts_with("# s3-inventory-bill\n"), "{md}");
    assert!(
        md.contains("Estimate from list prices, not your AWS invoice (free tier, credits, discounts not included)\n"),
        "{md}"
    );
    assert_eq!(md.matches("Estimate from list prices").count(), 1);

    let classes = rows_after(&md, "## Storage by class");
    for (i, class) in json["classes"].as_array().unwrap().iter().enumerate() {
        let amount = class["estimated_usd_month"].as_str().unwrap_or("unpriced");
        assert_eq!(classes[i][0], class["class"].as_str().unwrap());
        assert_eq!(classes[i][1], class["objects"].to_string());
        assert_eq!(classes[i][2], class["raw_bytes"].as_str().unwrap());
        assert_eq!(classes[i][3], class["billable_bytes"].as_str().unwrap());
        assert_eq!(classes[i][4], amount);
        assert!(
            text.contains(&format!(
                "{} objects={} raw_bytes={} billable_bytes={} estimated_usd_month={amount}\n",
                classes[i][0], classes[i][1], classes[i][2], classes[i][3]
            )),
            "{text}"
        );
    }
    let total = classes.last().unwrap();
    assert_eq!(total[0], "**Total**");
    assert_eq!(total[4], json["estimated_usd_month"].as_str().unwrap());
    assert!(text.contains(&format!("estimated_usd_month={}\n", total[4])));

    let prefixes = rows_after(&md, "## Storage by prefix");
    assert_eq!(prefixes.len(), json["prefixes"].as_array().unwrap().len());
    for (i, prefix) in json["prefixes"].as_array().unwrap().iter().enumerate() {
        assert_eq!(prefixes[i][0], prefix["prefix"].as_str().unwrap());
        assert_eq!(
            prefixes[i][3],
            prefix["estimated_usd_month"].as_str().unwrap()
        );
        assert!(text.contains(&format!(
            "{} objects={} raw_bytes={} estimated_usd_month={}\n",
            prefixes[i][0], prefixes[i][1], prefixes[i][2], prefixes[i][3]
        )));
    }

    let targets = rows_after(&md, "## Lifecycle");
    let json_targets = json["lifecycle"]["targets"].as_array().unwrap();
    assert_eq!(targets.len(), json_targets.len());
    for (i, target) in json_targets.iter().enumerate() {
        assert_eq!(targets[i][0], target["target"].as_str().unwrap());
        assert_eq!(targets[i][2], target["eligible_objects"].to_string());
        for (col, key) in [
            (3, "monthly_now_usd"),
            (4, "monthly_after_usd"),
            (5, "transition_request_usd"),
            (6, "early_delete_usd"),
            (7, "savings_over_horizon_usd"),
        ] {
            assert_eq!(targets[i][col], target[key].as_str().unwrap(), "{key}");
            assert!(
                text.contains(&format!("{key}={}", targets[i][col])),
                "{key}"
            );
        }
        assert_eq!(targets[i][8], "Not executed");
    }
    assert!(md.contains("## Warnings\n"), "{md}");
    assert!(
        md.contains("- objects without a built-in rate: 1\n"),
        "{md}"
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn markdown_without_as_of_says_the_lifecycle_is_omitted() {
    let (code, md, _) = run(&["--md", "examples/inventory.csv"]);
    assert_eq!(code, 0);
    assert!(
        md.contains("## Lifecycle\n\nOmitted: pass --as-of YYYY-MM-DD\n"),
        "{md}"
    );
    assert!(!md.contains("## Warnings"), "{md}");
}

#[test]
fn pipes_and_line_breaks_in_cells_are_escaped() {
    let dir = std::env::temp_dir().join(format!("s3-inventory-bill-mdesc-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let inv = dir.join("inv.csv");
    fs::write(
        &inv,
        "Bucket,Key,Size,StorageClass\nb,\"a|b\nc|/d\",5,\"X|Y\"\nb,\"back\\|slash/e\",5,STANDARD\n",
    )
    .unwrap();
    let (code, md, err) = run(&["--md", inv.to_str().unwrap()]);
    assert_eq!(code, 0, "{err}");
    assert!(md.contains("| a\\|b\\nc\\|/ |"), "{md}");
    assert!(md.contains("| X\\|Y |"), "{md}");
    assert!(md.contains("| back\\\\\\|slash/ |"), "{md}");
    for line in md.lines().filter(|l| l.starts_with('|')) {
        assert!(!line.chars().any(char::is_control), "{line:?}");
        // Every row has the same number of unescaped pipes as its header row.
        let bare = line.replace("\\\\", "").replace("\\|", "");
        assert!(bare.matches('|').count() >= 3, "{line}");
    }
    let (_, table_check, _) = run(&["--md", inv.to_str().unwrap()]);
    let class_rows = rows_after(&table_check, "## Storage by class");
    assert!(
        class_rows.iter().all(|r| r.len() == class_rows[0].len()),
        "{class_rows:?}"
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn md_and_json_cannot_be_combined_and_help_lists_md() {
    let (code, stdout, stderr) = run(&["--md", "--json", "examples/inventory.csv"]);
    assert_eq!(code, 2, "{stderr}");
    assert!(stdout.is_empty(), "{stdout}");
    assert!(
        stderr.contains("--md") && stderr.contains("--json"),
        "{stderr}"
    );
    let (code, stdout, _) = run(&["--help"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("--md"), "{stdout}");
}

#[test]
fn markdown_has_the_same_report_with_quiet_and_nothing_on_stderr() {
    let (code, md, err) = run(&["--md", "--as-of", "2026-11-09", "examples/inventory.csv"]);
    assert_eq!(code, 0, "{err}");
    assert!(err.is_empty(), "{err}");
    let (code, quiet, err) = run(&[
        "--md",
        "--quiet",
        "--as-of",
        "2026-11-09",
        "examples/inventory.csv",
    ]);
    assert_eq!(code, 0, "{err}");
    assert_eq!(quiet, md);
    assert!(md.contains("| Not executed |"), "{md}");
}

#[test]
fn readme_markdown_example_is_what_the_binary_prints() {
    let readme = fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/README.md"))
        .unwrap()
        .replace("\r\n", "\n");
    let fence = "```markdown\n";
    let start = readme.find(fence).expect("README has a markdown block") + fence.len();
    let end = start + readme[start..].find("\n```\n").expect("block is closed");
    let (code, md, err) = run(&["--md", "--as-of", "2026-11-09", "examples/inventory.csv"]);
    assert_eq!(code, 0, "{err}");
    assert_eq!(format!("{}\n", &readme[start..end]), md);
}

#[test]
fn markdown_syntax_in_keys_is_shown_as_plain_text() {
    let dir = std::env::temp_dir().join(format!("s3-inventory-bill-mdinj-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let inv = dir.join("inv.csv");
    fs::write(
        &inv,
        "Bucket,Key,Size,StorageClass\nb,\"[click](http://evil.example/x)/f\",5,STANDARD\nb,\"![i](http:)/<b>x</b>/`c`/*e*/_u_/a@b.c/f\",5,STANDARD\nb,\"www.evil.example/x/f\",5,STANDARD\n",
    )
    .unwrap();
    let (code, md, err) = run(&["--md", "--prefix-depth", "8", inv.to_str().unwrap()]);
    assert_eq!(code, 0, "{err}");
    assert!(md.contains("\\[click\\](http:\\//evil.example"), "{md}");
    assert!(md.contains("\\!\\[i\\]"), "{md}");
    assert!(md.contains("\\<b\\>x\\</b\\>"), "{md}");
    assert!(md.contains("\\`c\\`"), "{md}");
    assert!(md.contains("\\*e\\*"), "{md}");
    assert!(md.contains("\\_u\\_"), "{md}");
    assert!(md.contains("a\\@b.c"), "{md}");
    assert!(md.contains("www\\.evil.example/x"), "{md}");
    assert!(!md.contains("www.evil"), "{md}");
    for line in md.lines().filter(|l| l.contains("evil.example")) {
        assert!(!line.contains("[click]("), "{line}");
        assert!(!line.contains("://"), "{line}");
    }
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn source_url_from_a_price_file_is_shown_as_plain_text() {
    let dir = std::env::temp_dir().join(format!("s3-inventory-bill-mdurl-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let root = env!("CARGO_MANIFEST_DIR");
    let mut prices: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(format!("{root}/prices/s3-storage.json")).unwrap(),
    )
    .unwrap();
    prices["source_url"] = serde_json::Value::from(
        "https://x.example/[click](https://evil.example/p)![t](https://evil.example/t.png)<img/src=//evil.example/t.png>*b*`c`",
    );
    let file = dir.join("prices.json");
    fs::write(&file, prices.to_string()).unwrap();
    let inv = format!("{root}/examples/inventory.csv");
    let (code, md, err) = run(&["--md", "--prices", file.to_str().unwrap(), &inv]);
    assert_eq!(code, 0, "{err}");
    let line = md.lines().find(|l| l.starts_with("- Source:")).unwrap();
    assert!(!line.contains("[click]("), "{line}");
    assert!(!line.contains("![t]"), "{line}");
    assert!(!line.replace("\\<", "").contains('<'), "{line}");
    assert!(line.contains("\\[click\\]"), "{line}");
    assert!(line.contains("\\!\\[t\\]"), "{line}");
    assert!(line.contains("\\<img/"), "{line}");
    let (code, text, _) = run(&["--prices", file.to_str().unwrap(), &inv]);
    assert_eq!(code, 0);
    assert!(
        text.contains("source: https://x.example/[click]("),
        "{text}"
    );
    let (code, json, _) = run(&["--json", "--prices", file.to_str().unwrap(), &inv]);
    assert_eq!(code, 0);
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert!(parsed["source_url"]
        .as_str()
        .unwrap()
        .contains("[click](https://evil"));
    let _ = fs::remove_dir_all(&dir);
}
