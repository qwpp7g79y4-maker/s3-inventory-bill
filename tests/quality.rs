//! One million inventory rows, priced once, checked against the closed form.

use s3_inventory_bill::{bill_many, prices_builtin};

#[test]
fn quality_benchmark_one_million_lines() {
    let header = b"Bucket,Key,Size,StorageClass\n";
    let row = b"bkt,p/o,1048576,STANDARD\n";
    let mut data = Vec::with_capacity(header.len() + row.len() * 1_000_000);
    data.extend_from_slice(header);
    for _ in 0..1_000_000 {
        data.extend_from_slice(row);
    }
    let bill = bill_many(&[data.as_slice()], &prices_builtin(), None, 1).unwrap();
    assert_eq!(bill.total_objects, 1_000_000);
    assert_eq!(bill.total_raw_bytes, 1_048_576_000_000);
    assert_eq!(bill.total_usd, "22.4609375");
    assert_eq!(bill.prefixes.len(), 1);
    assert_eq!(bill.prefixes[0].prefix, "p/");
    assert_eq!(bill.prefixes[0].objects, 1_000_000);
    assert_eq!(bill.prefixes[0].usd_month, "22.4609375");
    assert!(bill.lifecycle.is_none());
    assert!(bill.warnings.is_empty());
}

/// Every test name in the claim table of QUALITY.md must be a real test.
#[test]
fn quality_claim_table_names_only_existing_tests() {
    use std::fs;
    use std::path::{Path, PathBuf};
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let quality = fs::read_to_string(root.join("QUALITY.md")).unwrap();
    let table = quality
        .split("## Claim")
        .nth(1)
        .expect("QUALITY.md has a claim table");
    let mut sources = String::new();
    fn walk(dir: &Path, out: &mut String) {
        for entry in fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                walk(&path, out);
            } else if path.extension().is_some_and(|ext| ext == "rs") {
                out.push_str(&fs::read_to_string(&path).unwrap());
            }
        }
    }
    walk(&root.join("src"), &mut sources);
    walk(&root.join("tests"), &mut sources);
    let mut checked = 0;
    for row in table.lines().filter(|line| line.starts_with("| ")) {
        let cells: Vec<&str> = row.split('|').map(str::trim).collect();
        // | # | Claim | Test | Status |
        let Some(test_cell) = cells.get(3) else {
            continue;
        };
        for name in test_cell.split('`').skip(1).step_by(2) {
            if !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
                continue;
            }
            assert!(
                sources.contains(&format!("fn {name}(")),
                "QUALITY.md names a test that does not exist: {name}"
            );
            checked += 1;
        }
    }
    assert!(checked >= 50, "only {checked} test names found");
}

/// Many distinct prefixes in every price class: the time grows with the number
/// of prefixes, with a small constant. A generous bound, so a slow machine passes
/// and a return to bit-by-bit division (about 170 us per prefix in release) does not.
#[test]
fn many_distinct_prefixes_are_priced_in_a_moment() {
    let classes = [
        "STANDARD",
        "STANDARD_IA",
        "GLACIER",
        "DEEP_ARCHIVE",
        "INTELLIGENT_TIERING",
        "ONEZONE_IA",
        "GLACIER_IR",
        "EXPRESS_ONEZONE",
    ];
    let build = |count: usize| {
        let mut data = String::from("Bucket,Key,Size,StorageClass\n");
        for i in 0..count {
            data.push_str(&format!(
                "b,p{i}/o,{},{}\n",
                1_000_003 + i * 7919,
                classes[i % 8]
            ));
        }
        data.into_bytes()
    };
    let prices = prices_builtin();
    let mut seconds = Vec::new();
    for count in [10_000, 20_000, 40_000] {
        let data = build(count);
        let start = std::time::Instant::now();
        let bill = bill_many(&[data.as_slice()], &prices, None, 1).unwrap();
        seconds.push(start.elapsed().as_secs_f64());
        assert_eq!(bill.prefixes.len(), count);
    }
    assert!(seconds[2] < 4.0, "40,000 prefixes took {:.2} s", seconds[2]);
    // Twice the input must not take more than about 3 times as long.
    assert!(
        seconds[2] < 3.0 * seconds[1].max(0.05) + 0.5,
        "20,000: {:.2} s, 40,000: {:.2} s",
        seconds[1],
        seconds[2]
    );
}

fn test_names_in_claim_table(text: &str) -> Vec<String> {
    let table = text
        .split("## Claim")
        .nth(1)
        .or_else(|| text.split("## Bewering").nth(1));
    let table = table.expect("a claim table");
    let mut names = Vec::new();
    for row in table.lines().filter(|line| line.starts_with("| ")) {
        let cells: Vec<&str> = row.split('|').map(str::trim).collect();
        if let Some(cell) = cells.get(3) {
            names.extend(cell.split('`').skip(1).step_by(2).map(str::to_string));
        }
    }
    names
}

/// The Dutch page names the same tests as the English one, and both pages
/// carry a date, a commit, and a list of what was not run.
#[test]
fn quality_pages_agree_and_record_date_commit_and_what_was_not_run() {
    use std::fs;
    use std::path::PathBuf;
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let english = fs::read_to_string(root.join("QUALITY.md")).unwrap();
    let dutch = fs::read_to_string(root.join("QUALITY.nl.md")).expect("QUALITY.nl.md");
    assert_eq!(
        test_names_in_claim_table(&english),
        test_names_in_claim_table(&dutch)
    );
    for (page, text, not_run) in [
        ("EN", &english, "## Not run"),
        ("NL", &dutch, "## Niet uitgevoerd"),
    ] {
        assert!(
            text.contains(not_run),
            "{page}: no list of what was not run"
        );
        let has_date = text.as_bytes().windows(10).any(|w| {
            w.iter().enumerate().all(|(i, b)| match i {
                4 | 7 => *b == b'-',
                _ => b.is_ascii_digit(),
            })
        });
        assert!(has_date, "{page}: no date YYYY-MM-DD");
        let has_commit = text
            .split(|c: char| !c.is_ascii_alphanumeric())
            .any(|word| word.len() == 40 && word.bytes().all(|b| b.is_ascii_hexdigit()));
        assert!(has_commit, "{page}: no full commit hash");
    }
}

#[test]
fn security_policy_names_the_contact_address() {
    let text = std::fs::read_to_string(
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("SECURITY.md"),
    )
    .unwrap();
    assert!(text.contains("pep_ai@icloud.com"));
}

/// The source URL named in `tests/real_shape.rs` is also named in both
/// quality pages, so a reader can check the input shape against its source.
#[test]
fn both_quality_pages_name_the_source_url_of_the_real_input_shape() {
    use std::fs;
    use std::path::PathBuf;
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let url = "https://docs.aws.amazon.com/AmazonS3/latest/userguide/storage-inventory.html";
    let test = fs::read_to_string(root.join("tests/real_shape.rs")).unwrap();
    assert!(test.contains(url), "real_shape.rs lost its source URL");
    for page in ["QUALITY.md", "QUALITY.nl.md"] {
        let text = fs::read_to_string(root.join(page)).unwrap();
        assert!(text.contains(url), "{page} does not name {url}");
    }
}

/// The security policy must not claim the tool writes only to the standard
/// streams: `--output` writes a temporary file and renames it into place.
#[test]
fn security_policy_describes_the_output_file_write() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let text = std::fs::read_to_string(root.join("SECURITY.md")).unwrap();
    assert!(
        !text.contains("writes only to standard output and standard error"),
        "SECURITY.md still says the tool writes only to the standard streams"
    );
    assert!(
        text.contains("--output"),
        "SECURITY.md does not name --output"
    );
    assert!(
        text.contains("temporary file"),
        "SECURITY.md does not name the temporary file"
    );
    assert!(
        text.contains("symbolic link"),
        "SECURITY.md does not say what happens to a symbolic link"
    );
}
