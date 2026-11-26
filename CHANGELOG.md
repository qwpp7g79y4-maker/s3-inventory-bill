# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.0.0] - 2026-11-15

Changes made after 2026-11-15; the version number did not change.

### Changed

- The tests no longer depend on the system clock: every test that runs the binary fixes the day with `SOURCE_DATE_EPOCH` (or `--as-of`), so the price-age warning cannot turn a test red as time passes. Tests only; the behaviour of the tool is unchanged.
- The monthly cost is labelled as an estimate: `usd_month` is now `estimated_usd_month` in the text report and in the JSON (total, each class, each prefix), the text report has one line `Estimate from list prices, not your AWS invoice (free tier, credits, discounts not included)`, and the JSON has `"estimate": true` at the top level.

### Added

- Documentation to the standard of the other tools: README sections Installation, Quick start (the sample is read from `examples/` or written to a temporary directory, never into the repository), Aan de slag in 5 minuten, Options, Example, Exit codes, Support and Contact; `README.nl.md`, `CONTRIBUTING.md`, `QUALITY.nl.md`, and `examples/README.md` with `examples/inventory.expected`. `SECURITY.md` names the contact address. Tests keep the English and Dutch READMEs, both quality pages and the example output in step with the binary.
- `build.rs`: a debug build prints the commit of its own checkout after the version in `--version`; a release build, a build outside a checkout and a build without git print the version only (`tests/version.rs`).
- `about.hbs` and `about.toml` for the third-party license listing, and `.github/workflows/release.yml` that builds the release archives from a version tag after CI passes and puts `THIRD-PARTY-LICENSES.html` in each. The same file is committed at the repository root so it is visible without a release.
- `THIRD-PARTY-LICENSES.html`, generated with `cargo about generate --locked about.hbs`.
- The README and the quality pages name GitHub CI run 45582606112 on commit `7d7c78befbcb71a4132cc6e174d0ba452037c0a0`. Windows and macOS tests passed there. A musl build is still only linted.
- The release workflow passes `--clobber` to `gh release upload`, so a second run can replace assets of the same name. The first release still uses `gh release create`.
- The README and the security page say the built tool starts no subprocess. The build script may run local git during `cargo build`.

- The suite has 210 tests, measured on 2026-11-20 on stable, in release, on Rust 1.85 and in the unpacked package.
- `"schema_version": 1` is the first key of every JSON document. An error prints no JSON.
- `--output FILE`: the report goes to a file, written to a temporary file in the same directory and renamed into place. A symbolic link is replaced, not followed; a directory or a special file is refused; any write failure is exit 2.
- SIGINT (Ctrl-C) ends the run with exit 130, the line `s3-inventory-bill: interrupted by SIGINT` on standard error and nothing on standard output. A SIGINT during the final write waits until the report is complete. Unix only; Windows is unchanged.
- `--md`: the report as Markdown tables with the same amounts, the estimate line and escaped `|` and line breaks. With `--json` it is a usage error.
- `--quiet` (only the report and errors; no warning copies on standard error) and `--verbose` (progress lines on standard error). Both together are a usage error with exit 2.
- `tests/real_shape.rs`: an inventory in the shape the S3 Inventory documentation describes, priced exactly.
- A warning when the date of the price data (built-in or `--prices`) is more than 180 days before `--as-of` or before today. The exit code stays 0. `SOURCE_DATE_EPOCH` replaces the clock for that check.
- `man/s3-inventory-bill.1` and `SECURITY.md`. A test checks that the man page holds the `--help` text and exactly its long options.
- A closed standard output is noticed at start-up on Linux. Together with a failed write to standard output, including `--help` and `--version`, it ends with exit 2 and `could not write output` on standard error. Standard output opened on `/dev/null` still exits 0.
- A tier name `ARCHIVE` or `DEEP_ARCHIVE` in `IntelligentTieringAccessTier` is read like `ARCHIVE_ACCESS` or `DEEP_ARCHIVE_ACCESS`.
- The library exports `escape_controls` and `shown_path`. `shown_path` gives the path as text and does not escape it; `Error::new` escapes the whole message once.
- A test checks that `README.md`, `QUALITY.md`, `CHANGELOG.md`, `SECURITY.md` and `man/` hold no local path and no internal name.
- The README lists the input rules, the limits on gzip members and on the price file, the number limits (bytes, money, price fields, `--as-of` years), the memory and time measured for many distinct prefixes and for a file at the size limit, and the system requirements. It no longer says that the total is "exact"; it says that money has 28 significant digits.
- `scripts/check-glibc.sh` and a CI job `glibc` that fails when the release binary needs a glibc symbol newer than 2.34. The CI jobs have timeouts, the checkout does not keep credentials, and `cargo test` runs with `--no-fail-fast`. `.gitattributes` also pins `*.sh` to LF.
- When a file has only CR line ends, the error `missing column ...` now adds that the first row holds a carriage return and that CR-only line ends are not read. Other missing-column errors are unchanged.
- `.github/dependabot.yml`, for cargo and GitHub Actions updates.
- The check for local paths and internal names now also reads `src/`, `tests/`, `scripts/`, `examples/`, `prices/`, `.github/`, `deny.toml` and `Cargo.toml` (without its `exclude` line), not only the documents and the man page. Another test checks that `exclude` in `Cargo.toml` lists exactly the two local work files.
- The README says which `Size` forms are read: ASCII white space around a `Size`, `Bucket` or `StorageClass` is ignored and a sign, an underscore, `0x`, a non-ASCII digit or a non-breaking space is `invalid Size`. The behavior is unchanged and a test covers it.

### Fixed

- Pricing per prefix is fast again: 200,000 distinct prefixes took 33.5 s since the exact rounding and take 0.53 s now (measured 1x, 2x and 4x in the README). The division in `money.rs` works on 64-bit limbs instead of bit by bit and the rounding no longer tries the finest scale first.
- Lifecycle amounts (`monthly_now_usd`, `monthly_after_usd`, `savings_over_horizon_usd`) are rounded once from the exact sum, so they no longer carry about 1e-26 of noise from summing rounded pieces.
- Each amount is rounded once, half away from zero. A value on a half unit rounds away from zero, so 2 bytes at 1 USD per GB is `0.0000000018626451492309570313`, and an amount of 5e-29 is `0.0000000000000000000000000001` rather than `0.00`. Metadata dollars stacked on a large Standard total are the cost of the extra bytes, not the difference of two rounded totals: 1e17 Standard bytes at 418175.32 USD per GB over 500 TB plus one empty Glacier object prices that Glacier line at `3.190424609375`. A Standard total of the bytes 2688123546345455679 and 7628359478944001404 at 1.1882, 1.1887 and 1.1831 is `11367196327.415037670449633151`.
- The test of the `exclude` entry in `Cargo.toml` failed in the unpacked package, because cargo writes that array as several lines there. It now reads the array from `exclude = [` to the first `]`, on one line or on several, and a new test covers both forms. `cargo test --offline` now passes in the unpacked package.

### Changed

- The refusal of a Parquet or ORC inventory no longer says `not read in v0.1`. It reads `parquet inventories are not read; only CSV and gzip CSV are` (and the same for `orc`). The exit code is still 2.
- A CSV line with very many quoted fields is read in linear time. A line of 200,000 quoted fields (800 KB) took more than 20 s in the unreleased state before this change and is now refused with `too many columns` in under a second. The 65th field stops the read: a line of 5,000,000 quoted fields (20 MB) is refused in 0.05 s, where it needed 18 times the file size in memory before.
- A gzip file with more than 100000 members is refused with `gzip file has more than 100000 members`. Before, a file of 5,000,000 empty members (100 MB) took 13.3 s and one of 1 GiB took minutes; it is now refused in 0.6 s, and 100,000 empty members are read in 0.45 s. Gzip files with fewer members are read as before.
- Text after the closing quote of a quoted field, other than a comma or the end of the line, is an error (`row N: text after a closing quote`). Before, `b,k,"1"0,STANDARD` was read as a size of 10 and `"a"b/x` as the key `ab/x`.
- A file whose only line ends are CR is refused also when its last column is optional (`LastModifiedDate`, `IntelligentTieringAccessTier`) and every required column was found: `the first row is not a header line` and the CR hint. Before, such a file gave exit 0 and a bill of 0.00.
- A `--prices` file larger than 1048576 bytes is refused with `file is larger than 1048576 bytes`. Before, up to 1 GiB was read, and a file of 100 MB of numbers took 5 GB of memory. The built-in quote is about 2 KB.
- A plain inventory file is held in memory once, not twice: a file of 1073741824 bytes with two distinct prefixes took 1.05 GB (before, 2.1 GB).
- A record with quoted fields has the same limit of 64 columns as one without. Before, a quoted record of 104 columns was read; now it is refused with `too many columns`.
- A `Size` with leading zeros is read whatever its length: 40 zeros and a 5 was `invalid Size`, and is now 5 bytes. The value must still be at most 18446744073709551615.
- A bill whose prefix share is too large to multiply first (a total of many PiB at a rate above about 1 USD per GB, for example two objects of 2688123546345455679 and 7628359478944001404 bytes at 1.1882, 1.1887 and 1.1831) no longer ends with `prefix amounts do not add up to the total`. The share is now multiplied and divided in wider integers and rounded once, so the prefix amounts add up to the total as for a small bill.
- A backslash in a file name is shown as two backslashes in an error (one doubling), not four. `C:\Users\me\inv.csv` is shown as `C:\\Users\\me\\inv.csv`.
- The `--as-of` help text, the man page and the README say what the tool does: an object last modified on or before that day is eligible, and the time of day in `LastModifiedDate` is not read. They no longer say `midnight UTC`. The behavior is unchanged.
- The README says that warnings go to standard error with `--json` and are lines of the report on standard output in the text report, and that a pipe, a device or any other file that is not a regular file is refused with `not a file`. The behavior is unchanged.
- The man page test compares the OPTIONS section with the long options of `--help`, so an option cannot be missing there and still be found in the help block or in an example. The closed-output tests also run the real report and `--json`, not only `--help` and `--version`. `tests/docs_clean.rs` also refuses more internal names.
- `--prefix-depth` is 1 to 8: the range is in `--help`, the man page and the README. Any other value is a usage error with exit 2, as before.
- Tests that need a file name that is not UTF-8, or a control character in a file name, run only on Linux. Tests that read the README, the man page or the built-in price file also pass in a checkout with CRLF line ends.
- A missing required column now says that the first row must be a header line naming `Bucket`, `Key`, `Size` and `StorageClass`. The README explains the header line.
- A row number in an error is the line number in the file, blank lines included. An unterminated quote is reported as `row N: unterminated quote`.
- A quote inside an unquoted field is an error (`row N: quote in an unquoted field`) instead of being read as the start of a quoted field.
- An error about a file, including a decode error, starts with that file's path, also when only one file is given.
- Error text, usage errors, file names, and the source and region lines of the text report escape line and paragraph separators, bidirectional marks, invisible and filler characters, and a backslash. JSON output writes DEL, C1 controls and those characters as `\uXXXX`. A file name that is not UTF-8 is shown with escaped bytes, and the `.parquet` and `.orc` name check also works for it.
- The prefix amounts are divided on their own. A rounding difference with the total is added to the largest prefix; a larger difference is the error `prefix amounts do not add up to the total`. With a very large total a last-digit gap can stay on that prefix.
- A prefix share and a per-GiB amount no longer fail on an intermediate overflow when the final amount fits.
- A price in a `--prices` file has at most 28 decimals, and only space, tab, CR and LF around it are trimmed.
- The duplicate-key check of the price file uses a hash set instead of a list, and reads a multi-byte character without validating the rest of the file.
- `.gitattributes` names the file types (`*.csv`, `*.json`, `*.1`, `*.rs`, `*.toml`, `*.yml`) next to `* text=auto eol=lf`. `SECURITY.md` says that memory use with many distinct prefixes is a documented limit. The README says that no minimum Windows or macOS version has been determined.
- The package includes `man/` and `SECURITY.md`. `homepage` in `Cargo.toml` is `https://www.pepai.nl`.


### Changed

- Version 1.0.0. `--prices` refuses anything that is not a regular file and reads no further than the length its metadata reported.
- CI also runs `cargo audit` and `cargo deny --locked check licenses`; `deny.toml` holds the permissive license list.
- Tests that need Unix paths, symlinks, fifos or control characters in file names run only on Unix (later narrowed: see the entry about Linux in [Unreleased]).

## [0.1.0] - 2026-11-09

### Added

- `s3-inventory-bill` reads local S3 Inventory CSV and prices storage per class and prefix.
- Lifecycle savings to Standard-IA, Glacier Instant Retrieval, Glacier Flexible Retrieval, and Glacier Deep Archive, with minimum duration and the per-object transition request. The report says the transition is not executed.
- Built-in US East (N. Virginia) rates from the Amazon S3 pricing page, read on 2026-11-09, and a `--prices` file for another quote.
- Text table and `--json`. Gzip CSV is read. Parquet and ORC are not.
