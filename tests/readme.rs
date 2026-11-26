//! README.md is executable: console blocks match the binary, and every
//! numbered claim has a test.

use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn read(rel: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(rel);
    fs::read_to_string(&path)
        .unwrap_or_else(|err| panic!("read {rel}: {err}"))
        .replace("\r\n", "\n")
}

fn section<'a>(text: &'a str, heading: &str) -> &'a str {
    let marker = format!("{heading}\n");
    let start = text
        .find(&marker)
        .unwrap_or_else(|| panic!("missing heading {heading}"))
        + marker.len();
    let rest = &text[start..];
    match rest.find("\n## ") {
        Some(end) => &rest[..end],
        None => rest,
    }
}

fn rust_sources() -> String {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut out = String::new();
    for dir in ["src", "tests"] {
        for entry in fs::read_dir(root.join(dir)).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().and_then(|ext| ext.to_str()) == Some("rs") {
                out.push_str(&fs::read_to_string(&path).unwrap().replace("\r\n", "\n"));
                out.push('\n');
            }
        }
    }
    out
}

/// Every ```console block: the first line is `$ s3-inventory-bill ARGS`, the rest is stdout.
fn console_blocks(doc: &str) -> Vec<(String, Vec<String>, String)> {
    let mut out = Vec::new();
    let mut lines = doc.lines();
    while let Some(line) = lines.next() {
        if line != "```console" {
            continue;
        }
        let call = lines.next().expect("empty console block");
        let args: Vec<String> = call
            .strip_prefix("$ s3-inventory-bill ")
            .unwrap_or_else(|| {
                panic!("console block does not start with `$ s3-inventory-bill `: {call}")
            })
            .split_whitespace()
            .map(str::to_string)
            .collect();
        let mut body = String::new();
        let mut closed = false;
        for line in lines.by_ref() {
            if line == "```" {
                closed = true;
                break;
            }
            body.push_str(line);
            body.push('\n');
        }
        assert!(closed, "console block not closed: {call}");
        out.push((call.to_string(), args, body));
    }
    assert!(!out.is_empty(), "README has no console block");
    out
}

#[test]
fn readme_sections_follow_the_documentation_standard_in_order() {
    let text = read("README.md");
    // A `## ` line inside a fenced block is sample output, not a heading.
    let mut fenced = false;
    let mut headings = Vec::new();
    for line in text.lines() {
        if line.starts_with("```") {
            fenced = !fenced;
        } else if !fenced && line.starts_with("## ") {
            headings.push(line.trim_start_matches("## "));
        }
    }
    assert_eq!(
        headings,
        vec![
            "Why",
            "Installation",
            "Quick start",
            "Quickstart in 5 minutes",
            "Options",
            "Example",
            "How it counts",
            "Exit codes",
            "Limitations",
            "Prices and responsibility",
            "Maintenance",
            "License",
            "Support",
            "Contact"
        ]
    );
}

#[test]
fn install_uses_a_local_path() {
    let readme = read("README.md");
    let install = section(&readme, "## Installation");
    assert!(install.contains("cargo install --locked --path ."));
    assert!(install.contains("1.85"));
    assert!(install.contains("not published"));
}

#[test]
fn quickstart_heredoc_matches_the_sample_file() {
    let readme = read("README.md");
    let mut lines = readme.lines();
    let mut found = false;
    for line in lines.by_ref() {
        if line == "cat > \"$dir/inventory.csv\" <<'CSV'" {
            found = true;
            break;
        }
    }
    assert!(found, "quick start heredoc is missing");
    let mut body = String::new();
    for line in lines {
        if line == "CSV" {
            break;
        }
        body.push_str(line);
        body.push('\n');
    }
    assert_eq!(body, read("examples/inventory.csv"));
}

#[test]
fn console_examples_match_the_binary() {
    for file in ["README.md", "README.nl.md"] {
        check_console_blocks(&read(file));
    }
}

fn check_console_blocks(readme: &str) {
    for (call, args, expected) in console_blocks(readme) {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_s3-inventory-bill"));
        cmd.current_dir(env!("CARGO_MANIFEST_DIR"));
        cmd.env("NO_COLOR", "1");
        // 2026-11-20T00:00:00Z: the examples never read the real clock.
        cmd.env("SOURCE_DATE_EPOCH", "1795132800");
        cmd.args(&args);
        let out = cmd.output().unwrap();
        let stdout = String::from_utf8(out.stdout).unwrap();
        let stderr = String::from_utf8(out.stderr).unwrap();
        assert!(out.status.success(), "{call} failed: {stderr}");
        assert!(stderr.is_empty(), "{call} wrote to stderr: {stderr}");
        assert_eq!(stdout, expected, "console mismatch for {call}");
    }
}

#[test]
fn every_how_it_counts_claim_has_a_test() {
    let readme = read("README.md");
    let how = section(&readme, "## How it counts");
    let mut ids = Vec::new();
    for line in how.lines() {
        let Some(rest) = line.trim().strip_prefix('C') else {
            continue;
        };
        let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
        if digits.is_empty() || !rest[digits.len()..].starts_with('.') {
            continue;
        }
        ids.push(digits.parse::<u32>().unwrap());
    }
    assert_eq!(ids, (1..=24).collect::<Vec<_>>());
    let sources = rust_sources();
    for id in 1..=24 {
        let needle = format!("fn claim_c{id}_");
        assert!(sources.contains(&needle), "missing {needle}");
    }
    assert!(sources.contains("fn quality_benchmark_one_million_lines"));
}

#[test]
fn limitations_name_behaviors_the_tests_cover() {
    let readme = read("README.md");
    let limits = section(&readme, "## Limitations");
    for phrase in [
        "parquet",
        "orc",
        "not executed",
        "AWS SDK",
        "no network",
        "1073741824",
        "symlink",
        "directory",
        "us-east-1",
        "retrieval",
        "free tier not applied",
        "Reduced Redundancy unpriced",
        "same key counts twice",
        "not a forecast",
    ] {
        assert!(limits.contains(phrase), "limitations missing {phrase}");
    }
    let sources = rust_sources();
    for needle in [
        "fn file_larger_than_1073741824_bytes",
        "fn directory_is_rejected_and_symlink_is_followed",
        "fn claim_c16_",
        "fn claim_c17_",
        "fn claim_c22_",
        "fn claim_c23_",
        "fn quality_benchmark_one_million_lines",
    ] {
        assert!(sources.contains(needle), "missing {needle}");
    }
}

#[test]
fn licenses_and_changelog_match_the_package() {
    let mit = read("LICENSE-MIT");
    assert!(mit.contains("Copyright (c) 2026 PEPAI"));
    let apache = read("LICENSE-APACHE");
    assert!(apache.contains("Copyright 2026 PEPAI"));
    let changelog = read("CHANGELOG.md");
    assert!(changelog.contains("## [0.1.0] - 2026-11-09"));
    assert!(changelog.contains("not published"));
    let cargo = read("Cargo.toml");
    assert!(cargo.contains("MIT OR Apache-2.0"));
    assert!(cargo.contains("rust-version = \"1.85\""));
    assert!(!cargo.contains("[dev-dependencies]"));
    let readme = read("README.md");
    let license = section(&readme, "## License");
    assert!(license.contains("MIT OR Apache-2.0"));
    assert!(license.contains("LICENSE-MIT"));
    assert!(license.contains("LICENSE-APACHE"));
    assert!(readme.contains("https://aws.amazon.com/s3/pricing/"));
    assert!(readme.contains("2026-11-09"));
}

#[test]
fn quick_start_writes_nothing_into_the_repository() {
    let readme = read("README.md");
    let quick = section(&readme, "## Quick start");
    assert!(
        quick.contains("mktemp -d"),
        "the sample goes to a temporary directory"
    );
    for line in readme.lines() {
        let writes = line.contains("> examples/")
            || line.contains(">examples/")
            || line.contains("tee examples/")
            || line.contains("tee -a examples/");
        assert!(!writes, "README writes into examples/: {line}");
    }
}

#[test]
fn options_block_is_the_help_text_of_the_binary() {
    let readme = read("README.md");
    let options = section(&readme, "## Options");
    let out = Command::new(env!("CARGO_BIN_EXE_s3-inventory-bill"))
        .arg("--help")
        .output()
        .unwrap();
    let help = String::from_utf8(out.stdout).unwrap().replace("\r\n", "\n");
    assert!(
        options.contains(help.trim_end()),
        "the Options section must hold the --help text"
    );
}

#[test]
fn exit_codes_and_contact_are_documented() {
    let readme = read("README.md");
    let codes = section(&readme, "## Exit codes");
    for code in ["| 0 |", "| 2 |", "| 130 |"] {
        assert!(codes.contains(code), "missing {code}");
    }
    let contact = section(&readme, "## Contact");
    for needle in [
        "pep_ai@icloud.com",
        "https://www.pepai.nl",
        "https://x.com/pepai_nl",
        "https://ko-fi.com/pepai",
    ] {
        assert!(contact.contains(needle), "missing {needle}");
    }
    assert!(!readme.contains("## PEPAI"));
    assert!(readme.contains("GitHub Issues"));
}

/// The `## ` headings outside fenced blocks, and every fenced block.
fn headings_and_blocks(text: &str) -> (Vec<String>, Vec<String>) {
    let mut headings = Vec::new();
    let mut blocks = Vec::new();
    let mut current: Option<String> = None;
    for line in text.lines() {
        if line.starts_with("```") {
            match current.take() {
                Some(mut block) => {
                    block.push_str(line);
                    blocks.push(block);
                }
                None => current = Some(format!("{line}\n")),
            }
        } else if let Some(block) = current.as_mut() {
            block.push_str(line);
            block.push('\n');
        } else if let Some(title) = line.strip_prefix("## ") {
            headings.push(title.to_string());
        }
    }
    (headings, blocks)
}

#[test]
fn the_dutch_readme_has_the_same_sections_blocks_and_claims() {
    let english = read("README.md");
    let dutch = read("README.nl.md");
    let (en_headings, en_blocks) = headings_and_blocks(&english);
    let (nl_headings, nl_blocks) = headings_and_blocks(&dutch);
    assert_eq!(
        nl_headings,
        vec![
            "Waarom",
            "Installatie",
            "Snelstart",
            "Aan de slag in 5 minuten",
            "Opties",
            "Voorbeeld",
            "Hoe het telt",
            "Afsluitcodes",
            "Beperkingen",
            "Prijzen en verantwoordelijkheid",
            "Onderhoud",
            "Licentie",
            "Ondersteuning",
            "Contact"
        ]
    );
    assert_eq!(en_headings.len(), nl_headings.len());
    assert_eq!(en_blocks, nl_blocks, "code blocks differ between EN and NL");
    let claims = |text: &str| -> Vec<String> {
        text.lines()
            .filter_map(|line| {
                let rest = line.strip_prefix('C')?;
                let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
                (!digits.is_empty() && rest[digits.len()..].starts_with(". ")).then_some(digits)
            })
            .collect()
    };
    assert_eq!(claims(&english), claims(&dutch));
    assert_eq!(claims(&dutch).len(), 24);
    let contact = section(&dutch, "## Contact");
    for needle in [
        "pep_ai@icloud.com",
        "https://www.pepai.nl",
        "https://x.com/pepai_nl",
        "https://ko-fi.com/pepai",
    ] {
        assert!(contact.contains(needle), "missing {needle}");
    }
}

#[test]
fn the_example_directory_documents_its_command_and_expected_output() {
    let readme = read("examples/README.md");
    let command = "s3-inventory-bill --as-of 2026-11-09 examples/inventory.csv";
    assert!(
        readme.contains(command),
        "examples/README.md names the command"
    );
    assert!(readme.contains("inventory.expected"));
    let out = Command::new(env!("CARGO_BIN_EXE_s3-inventory-bill"))
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .env("NO_COLOR", "1")
        .env("SOURCE_DATE_EPOCH", "1795132800")
        .args(["--as-of", "2026-11-09", "examples/inventory.csv"])
        .output()
        .unwrap();
    assert!(out.status.success());
    assert_eq!(
        String::from_utf8(out.stdout).unwrap(),
        read("examples/inventory.expected")
    );
    // The same output is the first console block of the README.
    let blocks = console_blocks(&read("README.md"));
    assert_eq!(blocks[0].0, format!("$ {command}"));
    assert_eq!(blocks[0].2, read("examples/inventory.expected"));
}

#[test]
fn prices_and_maintenance_sections_follow_limitations() {
    let prices = read("prices/s3-storage.json");
    let date = prices
        .lines()
        .find_map(|l| l.trim().strip_prefix("\"source_date\": \""))
        .and_then(|r| r.split('"').next())
        .unwrap()
        .to_string();
    let cases = [
        (
            "README.md",
            "## Limitations",
            "## Prices and responsibility",
            "## Maintenance",
        ),
        (
            "README.nl.md",
            "## Beperkingen",
            "## Prijzen en verantwoordelijkheid",
            "## Onderhoud",
        ),
    ];
    for (file, limits, price_head, maint_head) in cases {
        let text = read(file);
        let heads: Vec<&str> = text.lines().filter(|l| l.starts_with("## ")).collect();
        let at = heads.iter().position(|h| *h == limits).unwrap();
        assert_eq!(heads[at + 1], price_head, "{file}");
        assert_eq!(heads[at + 2], maint_head, "{file}");
        let price_body = section(&text, price_head);
        assert!(price_body.contains(&date), "{file}: price date");
        assert!(price_body.contains("https://aws.amazon.com/s3/pricing/"));
        assert!(price_body.contains("--prices"));
        assert!(section(&text, maint_head).contains("GitHub Issues"));
    }
}
