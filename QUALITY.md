# Quality

This commit changes documentation only; the measurements below apply to code commit `5f02e4a`.

Suite: 217 passed, 0 failed, 0 ignored.

This page records checks. It is not a guarantee and it is not a fuzz report. This version is not published and has no git tag.

Measured on 2026-11-23, on Linux (x86_64), on the code commit `5f02e4a65fb6f53625ed83678d0df820f7accd07`. That commit holds the last change to code, tests, build files and workflows; every later commit changes documents only. Only the five commands in the first table below were run on that commit, from the crate root, with `--locked`, as `cargo fmt --check`, `cargo clippy ...` and `cargo test --locked`; no memory limit and no `-j` or `--test-threads` option was set. The rows of the second table were measured on 2026-11-20 on the earlier code commit `6b36822` and were not run again. GitHub CI on `main` commit `2d43ce8f7c4a070f72fc4e7dae189c3841d88459` passed: run 45577649641, jobs ubuntu-latest, ubuntu-latest with Rust 1.85, windows-latest, macos-latest, macos-15-intel, Dependency licenses, Dependency audit, and Highest glibc symbol. Commits after `5f02e4a` also changed `src/money.rs`, `src/main.rs`, `src/json_scan.rs` and tests. Only measured results are stated.

Toolchain: rustc 1.99.0 (b940084d7 2026-09-28) and cargo 1.99.0 (5f94df478 2026-08-27). For the minimum supported version: rustc 1.85.1 (4eb161250 2025-03-15) and cargo 1.85.1 (d73d2caf9 2024-12-31). cargo-deny 0.20.2, cargo-audit 0.22.2, cargo-about 0.9.2.

## Commands

Exit codes of the commands run on the commit named above (217 passed, 0 failed):

| Command | Exit |
| --- | ---: |
| `cargo fmt --check` | 0 |
| `cargo clippy --offline --locked --all-targets -- -D warnings` (stable 1.99.0) | 0 |
| `cargo +1.85 clippy --offline --locked --all-targets -- -D warnings` | 0 |
| `cargo test --locked` (debug, stable): 217 passed, 0 failed | 0 |
| `cargo +1.85 test --locked` (debug): 217 passed, 0 failed | 0 |

Measured on the earlier code commit `6b36822` (2026-11-20) and not run again on the commit above:

| Command | Exit |
| --- | ---: |
| `cargo test --offline --locked --release`: 210 passed, 0 failed | 0 |
| `cargo check --offline --tests --target x86_64-pc-windows-gnu` | 0 |
| `cargo clippy --offline --all-targets --target x86_64-pc-windows-gnu -- -D warnings` | 0 |
| `cargo check --offline --tests --target x86_64-apple-darwin` | 0 |
| `cargo clippy --offline --all-targets --target x86_64-unknown-linux-musl -- -D warnings` | 0 |
| `cargo build --offline --locked --release` | 0 |
| `cargo deny --locked check licenses` | 0 |
| `cargo audit --no-fetch` (local advisory database, not refreshed) | 0 |
| `cargo about generate --locked --offline about.hbs` | 0 |
| `cargo package --offline --allow-dirty` (with its verify build) | 0 |
| `cargo test --offline` in the unpacked `.crate`, in an empty directory: 210 passed, 0 failed | 0 |
| `actionlint .github/workflows/ci.yml .github/workflows/release.yml` | 0 |
| `bash scripts/check-glibc.sh target/release/s3-inventory-bill 2.34` | 0 |

The Windows and macOS test jobs ran on GitHub CI run 45577649641 and passed. A musl build has only been linted, not run. `cargo audit --no-fetch` read the local database (see "Not run") and reported none.

`cargo package --list` has 53 paths: the manifest files, `.cargo_vcs_info.json`, `.gitattributes`, `.gitignore`, both workflows (`ci.yml`, `release.yml`) and `dependabot.yml`, `CHANGELOG.md`, `CONTRIBUTING.md`, both licenses, `QUALITY.md`, `QUALITY.nl.md`, `README.md`, `README.nl.md`, `SECURITY.md`, `about.hbs`, `about.toml`, `build.rs`, `deny.toml`, `examples/README.md`, `examples/inventory.csv`, `examples/inventory.expected`, `man/s3-inventory-bill.1`, `prices/s3-storage.json`, `scripts/check-glibc.sh`, `src/*.rs` and `tests/*.rs`. It has no file that is only for local work. The suite also passes in the unpacked package: the `.crate` was unpacked in an empty directory and `cargo test --offline` there gave 210 passed, 0 failed (no git checkout: the version test then expects no commit in `--version`). `cargo package` read `exclude` as a block of several lines; `tests/docs_clean.rs` reads the array on one line and on several lines.

## Two gaps, measured before and after

Both were measured on the tip `619e7a6` with the release binary and the same input after the change, on code commit `5f02e4a`. Inputs: `pj_b1.json` is `prices/s3-storage.json` with `source_url` set to `https://x.example/[click](https://evil.example/p)<img/src=//evil.example/t.png>*b*`+"`c`"+`; `forge.csv` has the keys `real/data.bin`, `total objects=0 raw_bytes=0 estimated_usd_month=0.00`, `warning: prices are current` and `lifecycle`. Only these commands were run; no Markdown renderer was run.

Before (619e7a6):

```
$ s3-inventory-bill --md --quiet --prices pj_b1.json g1.csv | grep Source
5:- Source: https://x.example/[click](https://evil.example/p)<img/src=//evil.example/t.png>*b*`c`
$ s3-inventory-bill forge.csv | grep -n -E "^(total|warning)"
10:total objects=4 raw_bytes=1073741827 estimated_usd_month=0.0230000000642612576484680176
15:total objects=0 raw_bytes=0 estimated_usd_month=0.00 objects=1 raw_bytes=1 estimated_usd_month=0.0000000000214204192161560059
16:warning: prices are current objects=1 raw_bytes=1 estimated_usd_month=0.0000000000214204192161560059
$ s3-inventory-bill --md forge.csv | grep -n -E "^(total|warning)|real|lifecycle"
12:| Class | Objects | Raw bytes | Billable bytes | Estimated USD per month |
13:| --- | ---: | ---: | ---: | ---: |
14:| STANDARD | 4 | 1073741827 | 1073741827 | 0.0230000000642612576484680176 |
15:| **Total** | 4 | 1073741827 |  | 0.0230000000642612576484680176 |
19:| Prefix | Objects | Raw bytes | Estimated USD per month |
20:| --- | ---: | ---: | ---: |
21:| lifecycle | 1 | 1 | 0.0000000000214204192161560059 |
22:| real/ | 1 | 1073741824 | 0.0229999999999999999999999999 |
23:| total objects=0 raw_bytes=0 estimated_usd_month=0.00 | 1 | 1 | 0.0000000000214204192161560059 |
24:| warning: prices are current | 1 | 1 | 0.0000000000214204192161560059 |
$ s3-inventory-bill --json forge.csv | python3 -c prefixes
['lifecycle', 'real/', 'total objects=0 raw_bytes=0 estimated_usd_month=0.00', 'warning: prices are current']
$ s3-inventory-bill --json --prices pj_b1.json ... source_url
https://x.example/[click](https://evil.example/p)<img/src=//evil.example/t.png>*b*`c`
$ s3-inventory-bill cls.csv | grep -n -E "^(total|warning)"
11:total objects=2 raw_bytes=1073741825 estimated_usd_month=0.023
19:warning: objects without a built-in rate: 1
```

After (5f02e4a):

```
$ s3-inventory-bill --md --quiet --prices pj_b1.json g1.csv | grep Source
5:- Source: https:\//x.example/\[click\](https:\//evil.example/p)\<img/src=//evil.example/t.png\>\*b\*\`c\`
$ s3-inventory-bill forge.csv | grep -n -E "^(total|warning)"
10:total objects=4 raw_bytes=1073741827 estimated_usd_month=0.0230000000642612576484680176
$ s3-inventory-bill --md forge.csv | grep -n -E "^(total|warning)|real|lifecycle"
12:| Class | Objects | Raw bytes | Billable bytes | Estimated USD per month |
13:| --- | ---: | ---: | ---: | ---: |
14:| STANDARD | 4 | 1073741827 | 1073741827 | 0.0230000000642612576484680176 |
15:| **Total** | 4 | 1073741827 |  | 0.0230000000642612576484680176 |
19:| Prefix | Objects | Raw bytes | Estimated USD per month |
20:| --- | ---: | ---: | ---: |
21:| lifecycle | 1 | 1 | 0.0000000000214204192161560059 |
22:| real/ | 1 | 1073741824 | 0.0229999999999999999999999999 |
23:| total objects=0 raw_bytes=0 estimated_usd_month=0.00 | 1 | 1 | 0.0000000000214204192161560059 |
24:| warning: prices are current | 1 | 1 | 0.0000000000214204192161560059 |
$ s3-inventory-bill --json forge.csv | python3 -c prefixes
['lifecycle', 'real/', 'total objects=0 raw_bytes=0 estimated_usd_month=0.00', 'warning: prices are current']
$ s3-inventory-bill --json --prices pj_b1.json ... source_url
https://x.example/[click](https://evil.example/p)<img/src=//evil.example/t.png>*b*`c`
$ s3-inventory-bill cls.csv | grep -n -E "^(total|warning)"
11:total objects=2 raw_bytes=1073741825 estimated_usd_month=0.023
19:warning: objects without a built-in rate: 1
```

In the text report a key that starts with `total` or `warning` now starts with `\u{74}` or `\u{77}`; the real total line stays. `--md` writes `source_url` through the same escape as the keys. The JSON output is unchanged. A class name from the StorageClass column was not changed: with `cls.csv` (class `total x`) the text report still had one `total` line. Not measured: other labels (`source:`, `region:`, `lifecycle`) as a key start.

## Test result per target

Debug: `cargo test --locked`, measured on the commit named above: 217 passed, 0 failed, 0 ignored. The Rust 1.85 debug run (`cargo +1.85 test --locked`) on the same commit also gave 217 passed, 0 failed. The release run gave 210 passed, 0 failed on the earlier code commit `6b36822` and was not run again.

| Target | Passed | Failed | Ignored |
| --- | ---: | ---: | ---: |
| unittests `src/lib.rs` | 98 | 0 | 0 |
| unittests `src/main.rs` | 0 | 0 | 0 |
| `tests/cli.rs` | 57 | 0 | 0 |
| `tests/docs_clean.rs` | 7 | 0 | 0 |
| `tests/handbook_help.rs` | 1 | 0 | 0 |
| `tests/markdown.rs` | 8 | 0 | 0 |
| `tests/output.rs` | 8 | 0 | 0 |
| `tests/quality.rs` | 7 | 0 | 0 |
| `tests/readme.rs` | 13 | 0 | 0 |
| `tests/real_shape.rs` | 6 | 0 | 0 |
| `tests/schema.rs` | 2 | 0 | 0 |
| `tests/sigint.rs` | 2 | 0 | 0 |
| `tests/verbosity.rs` | 7 | 0 | 0 |
| `tests/version.rs` | 1 | 0 | 0 |
| doc-tests | 0 | 0 | 0 |
| total | 217 | 0 | 0 |

Release: `cargo test --release` ran on the earlier code commit `6b36822` and gave 210 passed, 0 failed; it was not run again on the commit named above.

Changes since commit `2707ced`, with the test result on the old and on the new `src/`:

- A valid input with a very large total and a rate above about 1 USD per GB no longer ends with `prefix amounts do not add up to the total` (`large_valid_totals_with_a_high_rate_are_not_refused_for_an_intermediate_loss`, two inputs of 2688123546345455679 and 7628359478944001404 bytes at 1.1882, 1.1887 and 1.1831, and of 1000000000000000007 and 1000000000000000013 bytes at 200, 100 and 50). It failed on the old code with exit 2 and that message, and passes now. `wide_share_rounds_once_and_keeps_the_sum` checks the new multiplication in the unit tests. A run of 300 random inputs of 1e16 to u64::MAX bytes in 2 to 5 prefixes at rates from 0.3 to 123456789.4 USD per GB gave no exit 2 with the debug binary.
- A gzip file with more than 100000 members is refused (`gzip_with_very_many_members_is_refused_in_a_moment`, 200,000 empty members). Red on the old code (exit 0, the file was read), green now. Measured with the release binary: 5,000,000 empty members (100 MB) took 13.3 s before and are refused in 0.6 s now; 100,000 members take 0.45 s.
- Text after the closing quote of a quoted field is an error (`text_after_a_closing_quote_is_refused_and_a_quoted_field_still_reads`): `b,k,"1"0,STANDARD` was read as a size of 10 (exit 0). Red on the old code, green now; the same test checks that quoted fields, doubled quotes, CRLF and a quoted last field still read.
- A CR-only file whose last column is optional is refused (`cr_only_file_is_refused_also_when_the_last_column_is_optional`): it gave exit 0 and `estimated_usd_month=0.00`. Red on the old code, green now.
- A `--prices` file over 1048576 bytes is refused and one of exactly 1048576 bytes loads (`a_price_file_over_one_mebibyte_is_refused_and_one_at_the_limit_loads`). Red on the old code (exit 0). The older test `files_over_the_cap_are_refused_by_name_and_size` now expects the price limit, 1048576, for the `--prices` case; the inventory limit is unchanged. Measured: a price file of 100 MB of numbers took 5.2 GB and 7.6 s before and is refused at once now.
- Pricing per prefix is fast again, and lifecycle amounts are rounded once. Red on the previous commit: `many_distinct_prefixes_are_priced_in_a_moment` failed (40,000 prefixes, debug: about 35 s against a 4 s limit) and `lifecycle_amounts_round_once_from_exact_values` failed (`savings_over_horizon_usd` ended in ...593 and ...404 where the exact values, from Python `fractions` with half away from zero at 28 digits, are ...594 and ...406). Green now. `Big::divmod` is Knuth algorithm D on 64-bit limbs and `fast_division_equals_bit_by_bit_division` compares it with the old bit-by-bit loop on 3000 operands; `round_ratio` starts at the scale the size of the value allows; a sum of two amounts is done in 128-bit integers when it fits. Prefix amounts, the total and the classes are unchanged (a text and a JSON report of 50,000 prefixes are byte-identical to the previous build). The lifecycle `monthly_now_usd`, `monthly_after_usd` and `savings_over_horizon_usd` are each one rounding of the exact sum of `bytes / 1024^3 * rate` terms, no longer a sum of rounded pieces; the transition request is still the one product `objects * rate`.
- A plain file is no longer copied after it is read, and a quoted record stops at the 65th field. These two change memory only, and no test measures memory: a file of exactly 1073741824 bytes with two distinct prefixes took 1.05 GB (before, 2.1 GB), and a line of 5,000,000 quoted fields (20 MB) took 22 MB (before, 374 MB).
- A quoted record has the same limit of 64 columns as an unquoted one (`a_quoted_record_has_the_same_column_limit_as_a_plain_one`). It failed on the old code (the quoted record of 104 columns was read) and passes now. The unit test `one_line_with_many_quoted_fields_is_read_in_linear_time` now expects `too many columns` and keeps its time limit.
- A `Size` with leading zeros is read whatever its length (`size_with_leading_zeros_is_read_whatever_its_length`). It failed on the old code with `invalid Size` and passes now.
- `readme_input_rules_hold_for_white_space_class_names_nul_and_sniffing` and `readme_number_limits_hold_for_sizes_prices_days_and_dates` (new) pin input rules and number limits that already held; they are green on the old code too. They are there so that the README sentences have a test.
- `tests/docs_clean.rs` now also reads `scripts/` and `Cargo.toml` (without its `exclude` line), refuses four more internal names, and a new test checks that `exclude` lists exactly the two local work files. On a copy of the tree with a home-directory path in `README.md`, an internal option name in `.github/workflows/ci.yml`, a work file name in `src/lib.rs`, an internal name in `Cargo.toml` and another in `scripts/check-glibc.sh` it failed (two tests red, five lines found); on the real tree it passes. The copy is removed.
- `tests/handbook_help.rs`: on a copy of the man page with the whole `--prefix-depth` entry taken out of the OPTIONS section the test failed (`--prefix-depth` missing from the OPTIONS section); on the real man page it passes. The copy is removed.
- Not caught by any test: changing the upper bound of `--prefix-depth` in the argument parser from 8 to 9 changes nothing, because the pricing code checks the range a second time (value 9 still exits 2).

A copy with CRLF line ends in every `.rs`, `.md`, `.json`, `.csv`, `.toml`, `.yml` and `.1` file was measured on the earlier record (163 passed, 0 failed). That copy was not rebuilt for this change. `.gitattributes` (`* text=auto eol=lf`) keeps a checkout on LF.

## Closed and full standard output

The binary has two modules with `unsafe` code, `stdout_probe` and `interrupt`; the library has none and forbids `unsafe`. `src/stdout_probe.rs` holds the `fcntl` call described here; `src/interrupt.rs` holds the SIGINT handler. Rust replaces a closed file descriptor 1 with `/dev/null` before `main` runs, so afterwards a closed standard output looks like `1<>/dev/null`. The probe runs a constructor before that replacement and records whether descriptor 1 was already closed, using a read-only `fcntl(1, F_GETFD)`. `main` then exits 2 with `could not write output`. The probe works on Linux only.

Measured on 2026-11-19 with the release binary built for this record and, with the same results, the debug binary, for `--as-of 2026-11-09 examples/inventory.csv`, `--help`, `--version` and `--json examples/inventory.csv` (shown as ARGS):

| Invocation | Exit | Standard error |
| --- | ---: | --- |
| `s3-inventory-bill ARGS >&-` | 2 | `s3-inventory-bill: could not write output` |
| `s3-inventory-bill ARGS >/dev/full` | 2 | `s3-inventory-bill: could not write output` |
| `s3-inventory-bill ARGS >/dev/null` | 0 | empty |
| `bash -c 's3-inventory-bill ARGS 1<>/dev/null'` | 0 | empty |
| `python3 -I`, `subprocess.run(..., stdout=subprocess.DEVNULL)` | 0 | empty |
| `s3-inventory-bill ARGS \| true` | 2 | `s3-inventory-bill: could not write output` |
| `unshare -Urm sh -c 'mount -t tmpfs none /proc && s3-inventory-bill ARGS > out.txt'` | 0 | empty |

In the last row the output file held 1218 bytes for the report, 851 for `--help`, 24 for `--version` and 599 for `--json`. No run panicked. The `| true` case depends on `true` closing the pipe before the report is written. The tests `closed_stdout_is_exit_2`, `dev_full_is_exit_2`, `dev_null_keeps_real_exit` and `stdout_set_to_dev_null_by_the_parent_is_not_an_error` in `tests/cli.rs` cover the report, `--json`, `--help` and `--version` with `>&-`, `/dev/full`, `/dev/null`, `1<>/dev/null` and a parent that passes `/dev/null`.

## Not run

- A fuzz campaign.
- A mutation tool. Edits of the code were tried by hand on a copy, as named above.
- The release workflow and a release archive. CI has run: run 45577649641 on `2d43ce8f7c4a070f72fc4e7dae189c3841d88459` passed. `cargo about generate` also wrote `THIRD-PARTY-LICENSES.html` in the repository.
- A build and a test run on musl. The Windows and macOS test jobs passed on that same run. Tests that need symlinks or fifos run on Unix only; tests that need a file name that is not UTF-8 or a control character in a file name, and the closed-stdout tests, run on Linux only.
- `cargo audit` with a freshly fetched advisory database.
- `cargo install` from the checkout. (`cargo package` with its verify build ran, see above.)
- A file at the 1073741824 byte limit with only distinct prefixes or only distinct storage class names.
- `git commit`, `git push`, `git tag`, and `cargo publish`.

## Known limits

- This page records checks. It is not a guarantee and it is not a fuzz report.
- Only inventory CSV and CSV gzip are read. Parquet and ORC are rejected.
- "No network call" is checked through the dependency list (`cargo tree -e normal`: `clap`, `flate2`, `rust_decimal`, `serde_json`) and a search of `src/` for network, process and write calls. No test watches system calls. `claim_c17_parquet_orc_gzip_and_no_sdk` checks `src/lib.rs` and `Cargo.toml` for the strings `aws-sdk` and `reqwest`.
- Closed-stdout detection (`src/stdout_probe.rs`) works on Linux only. On other systems a closed standard output is not told apart from `/dev/null`. A failing write (`/dev/full`, a closed pipe) exits 2 on every system.
- With a very large total, a last-digit gap can stay between the sum of the prefix amounts and the total (README C19, `large_total_leaves_the_prefix_drift_on_the_largest_prefix`). Each amount is rounded once, half away from zero, at the finest scale that fits. 3 GiB at 3.3333333333333333333333333333 per GiB is 9.9999999999999999999999999999 exactly and does not fit, so the tool shows `10.00` (`readme_number_limits_hold_for_sizes_prices_days_and_dates`). On the rate `777777777.7777777`, three prefixes of 1073741827, 3221225473 and 1073741831 bytes still leave a gap below 1e-18, and the correction is too small to change the largest prefix.
- The quality benchmark prices one million rows in one test; it is not a memory or time guarantee for a 1073741824 byte file.
- System requirements: the release binary built with glibc needs GLIBC_2.34 or newer (highest symbol, read with `objdump -T`). No minimum Windows or macOS version has been determined, and a musl build has only been linted.
- A `Size` is read with ASCII white space around it ignored; a key is not trimmed (`size_with_ascii_space_is_read_and_a_sign_or_other_digits_are_refused`).
- `LastModifiedDate` is read as `YYYY-MM-DD` followed by the end of the field, a `T` or a space; nothing after that is checked. `2024-01-15T99:99:99.000Z` with `--as-of 2024-01-15` is eligible, `2024-01-15t00:00:00.000z` (lower case) is not read as a date; the object is skipped for savings and counted in the warning `standard objects skipped for lifecycle savings`. `--as-of` compares days (`as_of_day_is_the_whole_day_and_the_next_day_is_not_eligible`).
- The refusal text for Parquet and ORC reads `parquet inventories are not read; only CSV and gzip CSV are` (and the same for `orc`).
- In the text report the warnings are lines at the end of the report on standard output; with `--json` they go to standard error.
- Pipes, devices and other files that are not regular files are refused as inventories (`not a file`); a process substitution such as `<(...)` is refused too.
- The whole input file is read into memory first and held once (a gzip file packed and inflated); a 52.5 MB file with 10 distinct prefixes took 105 MB (a 45.4 MB file with 1,500,000 rows and 10 prefixes took 46 MB on the last run), and a file of exactly 1073741824 bytes with two distinct prefixes took 3.9 s and 1.05 GB. The sum over several files has no limit: all of them stay in memory.
- A gzip file has at most 100000 members, a `--prices` file at most 1048576 bytes. The limits are in the README and tested.
- A quoted field that is followed by text other than a comma or the end of the line is an error (`text after a closing quote`). This is stricter than a general CSV reader; AWS does not write it.
- The sum of all sizes must fit in 64 bits. With `--as-of` the empty objects count at the minimum billable size, so one object of 18446744073709551615 bytes plus one empty object is accepted without `--as-of` and refused with it (`byte total does not fit`). A fix would compute these sums in a wider type; that is a product decision and is not made here.
- Price fields have bounds that the message does not name: the digits of a price are at most 79228162514264337593543950335, `*_min_days` is 1 to 36500, `min_billable_bytes` is a whole JSON number. They are in the README and tested.
- A CSV whose first bytes read `ORC` or `PAR1` (a header `ORC_ID,Bucket,...`) is refused as that format. Bytes after the last gzip member, even zeros, make a gzip inventory `not a gzip inventory`, which is stricter than `gzip -d`. A NUL byte is refused only in the first 512 bytes.
- A storage class is matched without regard to ASCII case and an unknown class is shown in capitals. `--prefix-depth` accepts `08` and `+3`.
- `--as-of` and `LastModifiedDate` are days; `--as-of` accepts 0000-01-01 to 9999-12-31.
- Clap error text shows a newline, a carriage return and other control characters as `\n`, `\r` and `\u{..}` on one line; a backslash is doubled.

## Open points

Known limits that are not fixed. Measured with the release binary on 2026-11-19 on synthetic files.

- Memory with many distinct prefixes or storage class names. The state is a map of prefixes, and every prefix holds its class names. Release build, text report, after the last code change: 1,000,000 rows (29.8 MB), one prefix, each row its own class: 1.32 s and 339 MB; the same with each row its own prefix: 3.21 s and 1.36 GB. Each row its own prefix over eight storage classes: 500,000 prefixes 1.35 s and 346 MB, 1,000,000 prefixes 2.67 s and 694 MB, 2,000,000 prefixes (72.5 MB) 5.42 s and 1.39 GB; with `--json` 2.9 s, 6.0 s and 11.7 s, and 4.9 GB at 2,000,000. Time and memory double with the input (factors 1.98 and 2.00 for 2x, 4.0 and 4.0 for 4x). Before this change 50,000, 100,000 and 200,000 such prefixes took 7.4 s, 15.3 s and 33.5 s (about 170 us per prefix, from bit-by-bit division); now 0.13 s, 0.26 s and 0.53 s. A file at the 1073741824 byte limit with only distinct prefixes would need much more; not run. The size limit is on the input, not on this state.
- A file whose only line ends are CR (`Bucket,Key,Size,StorageClass\rb,k,5,STANDARD\r`) is not read; it is refused with `missing column StorageClass: ...` (or `the first row is not a header line` when only an optional column is last) and the message says that the row holds a carriage return and that CR-only line ends are not read. Such a file is not converted.
- An empty key and a key written `(empty)` fall into one prefix line (`b,,1,STANDARD` plus `b,(empty),2,STANDARD` gives `(empty) objects=2`).
- Keys are not URL-decoded. A raw inventory file that has URL-encoded keys is priced under the encoded prefixes (README, `keys_are_used_as_written_and_not_url_decoded`).
- The report prints `as of:` (the price date) and, in the lifecycle block, `as-of:` (the `--as-of` date). Both labels are kept.
- The release workflow has not run. CI has: run 45577649641. See "Not run".

## Claim → test

Every numbered claim of `README.md` and the other statements of its text, with the test that proves it. Test names are from the run recorded above, and `quality_claim_table_names_only_existing_tests` checks that each one exists. "Gap" means no test proves it, or only part of it.

| # | Claim | Test | Status |
| --- | --- | --- | --- |
| C1 | One row is one object; the same key twice counts twice | `claim_c1_one_row_is_one_object` | tested |
| C2 | 1 GiB is 1,073,741,824 bytes; 1 TB is 1,024 GB | `claim_c2_gibibyte_and_terabyte` | tested |
| C3 | Standard first 50 TB rate, source and date, us-east-1 | `claim_c3_standard_rate_source_date_region` | tested |
| C4 | Standard-IA 128 KiB floor, 30 day horizon | `claim_c4_standard_ia_floor_and_thirty_days` | tested |
| C5 | One Zone-IA 128 KiB floor | `claim_c5_onezone_ia_floor` | tested |
| C6 | Glacier Instant Retrieval floor, 90 days, rate | `claim_c6_glacier_ir_floor_and_ninety_days` | tested |
| C7 | Glacier Flexible Retrieval metadata, 90 days, rate | `claim_c7_glacier_metadata_and_ninety_day_horizon`, `zero_byte_glacier_still_bills_metadata` | tested |
| C8 | Deep Archive metadata, 180 days, rate | `claim_c8_deep_archive_metadata_and_one_hundred_eighty_days` | tested |
| C9 | Intelligent-Tiering under 128 KiB and monitoring fee | `claim_c9_intelligent_tiering_small_objects_and_monitoring` | tested |
| C10 | Express One Zone rate, no floor | `claim_c10_express_has_no_floor` | tested |
| C11 | Transition request prices | `claim_c11_lifecycle_request_prices` | tested |
| C12 | Each savings line ends with Not executed | `claim_c12_savings_line_is_not_executed` | tested |
| C13 | Storage priced without `--as-of`; savings omitted | `claim_c13_savings_omitted_without_as_of` | tested |
| C14 | Missing, invalid or future date stays in storage, out of savings, not echoed | `claim_c14_future_and_bad_dates_are_skipped_without_the_value`, `absent_date_column_warns_and_prices_storage` | tested |
| C15 | Prefix depth and the prefix share of Standard | `claim_c15_prefix_depth_and_standard_share`, `prefix_depth_groups_the_sample_key`, `prefix_depth_keeps_a_trailing_slash_only_when_more_segments_exist` | tested |
| C16 | Unknown class is unpriced but counted | `claim_c16_unknown_class_is_unpriced`, `unpriced_class_renders_the_word` | tested |
| C17 | Gzip is inflated; Parquet and ORC rejected; no AWS SDK | `claim_c17_parquet_orc_gzip_and_no_sdk`, `parquet_and_orc_names_exit_2`, `gzip_reads_every_member` | tested |
| C18 | `--prices` replaces the quote; unknown field ignored; duplicate key is an error | `claim_c18_prices_override_changes_the_bill`, `claim_c18_unknown_field_ignored_and_duplicate_hidden` | tested |
| C19 | Dollar total is the class sum; JSON money as strings; prefix amounts and drift | `claim_c19_total_is_the_sum_and_json_uses_strings`, `prefix_amounts_sum_to_the_total`, `reconcile_fixes_rounding_but_rejects_a_large_gap`, `large_total_leaves_the_prefix_drift_on_the_largest_prefix` | tested |
| C20 | Invalid Size is `row N: invalid Size`, N counts file lines, nothing echoed | `claim_c20_errors_hide_file_contents`, `row_number_is_the_file_line_when_blank_lines_precede`, `bad_size_does_not_echo_the_row` | tested |
| C21 | Glacier metadata stacked on the Standard tiers | `claim_c21_metadata_crosses_the_standard_tier` | tested |
| C22 | Small Standard object can show a negative saving | `claim_c22_small_standard_object_saves_a_negative_amount` | tested |
| C23 | No retrieval or GET fees; 1 GiB Standard-IA is 0.0125 | `claim_c23_retrieval_is_not_added` | tested |
| C24 | Files share one Standard tier schedule | `claim_c24_files_share_one_standard_tier_schedule`, `two_files_share_one_tier_schedule` | tested |
| Q1 | Quick start sample file and every console block match the binary | `quickstart_heredoc_matches_the_sample_file`, `console_examples_match_the_binary` | tested |
| Q2 | `--json` keeps standard error empty for the sample | `example_inventory_exits_0_and_json_keeps_stderr_empty` | tested |
| Q3 | Warnings go to standard error when there are any | `warnings_go_to_stderr_with_json_and_savings_name_four_targets`, `absent_date_column_warns_and_prices_storage` | tested |
| W1 | Header line naming Bucket, Key, Size, StorageClass is required; a file without it is refused with exit 2 and the expected columns | `headerless_aws_inventory_line_is_refused_with_the_expected_columns`, `missing_column_and_short_row_and_duplicate_column` | tested |
| W2 | Keys are used as written, not URL-decoded | `keys_are_used_as_written_and_not_url_decoded` | tested |
| W3 | A file above 1073741824 bytes is refused, on disk and after gzip | `file_larger_than_1073741824_bytes`, `files_over_the_cap_are_refused_by_name_and_size`, `plain_and_gzip_over_the_cap_say_how_large` | tested |
| W4 | A symlink is followed; a directory is an error | `directory_is_rejected_and_symlink_is_followed`, `symlink_is_followed_and_a_directory_is_rejected` | tested |
| W5 | `--prices` and every inventory accept only a regular file | `prices_fifo_is_refused_without_reading`, `inventory_fifo_is_refused_without_reading`, `missing_prices_file_exits_2` | tested (Unix) |
| W6 | Intelligent-Tiering tier `ARCHIVE` and `DEEP_ARCHIVE` read like the `_ACCESS` names | `it_inventory_tier_names_without_access_suffix_match_the_access_names` | tested |
| W7 | A decode error names the file | `bad_gzip_in_second_file_names_that_file`, `csv_quote_errors_and_bad_gzip_name_the_file`, `a_file_error_names_the_file_with_one_file_too` | tested |
| W8 | Closed or failing standard output exits 2 with `could not write output`; `/dev/null` keeps exit 0 | `closed_stdout_is_exit_2`, `dev_full_is_exit_2`, `dev_null_keeps_real_exit`, `stdout_set_to_dev_null_by_the_parent_is_not_an_error` | tested (Linux only) |
| W9 | Prices are decimals of at most 28 decimals; only space, tab, CR, LF are trimmed | `parse_money_trims_only_space_tab_cr_lf`, `money_rejects_exponent_sign_and_empty` | tested |
| W10 | The account free tier is not applied; nothing is waived | `console_examples_match_the_binary` shows 1 GiB billed in full at 0.023 | partly tested: no test names the free tier |
| W11 | Install from a local path with Rust 1.85 | `install_uses_a_local_path` checks the text only | gap: `cargo install` and Rust 1.85 not run |
| W12 | README sections and licenses match the package | `readme_sections_follow_the_documentation_standard_in_order`, `licenses_and_changelog_match_the_package` | tested |
| W13 | Savings assume the object stays for the whole horizon; not a forecast | none | gap: a statement of scope, no behavior to test |
| W14 | One Zone-IA and Intelligent-Tiering are not savings targets | `warnings_go_to_stderr_with_json_and_savings_name_four_targets` lists the four targets | tested |
| M1 | The man page holds the `--help` text, and its OPTIONS section describes exactly the long options of `--help` | `help_long_options_match_the_man_page_and_unknown_option_is_not_a_panic` | tested |
| W15 | `--prefix-depth` is 1 to 8; any other value exits 2 | `prefix_depth_range_is_named_in_help_readme_and_man_and_enforced` | tested |
| W16 | A record has at most 64 columns, quoted or not | `sixty_four_columns_pass_and_sixty_five_are_refused`, `a_quoted_record_has_the_same_column_limit_as_a_plain_one` | tested |
| W17 | A quote that opens inside an unquoted field is an error | `quote_inside_an_unquoted_field_is_refused`, `quote_error_names_the_line_and_exits_2` | tested |
| W18 | A file with only CR line ends is refused with `missing column StorageClass` and the message names the CR line ends | `cr_only_file_is_refused_and_empty_key_shares_the_empty_prefix`, `cr_only_file_says_the_line_ends_are_the_cause` | tested |
| W19 | An empty key and a key `(empty)` share one prefix line | `cr_only_file_is_refused_and_empty_key_shares_the_empty_prefix` | tested |
| W20 | One line of 200,000 quoted fields is refused in under a second; a line of 5,000,000 is refused in 0.05 s | `one_line_with_many_quoted_fields_is_read_in_linear_time` (unit level, 2 s limit) | tested (time); gap: no test measures the memory of the 5,000,000 field line, measured by hand |
| W21 | Memory follows the number of distinct prefixes; the measured figures (2,000,000 prefixes, 10 prefixes, the 1073741824 byte file) | `many_distinct_prefixes_are_priced_in_a_moment` (time only, 40,000 prefixes in under 4 s in debug and not more than about 3 times the time of 20,000); memory measured by hand on 2026-11-20 | tested (time); gap: no test measures memory |
| W22 | System requirements: Rust 1.85 or newer, glibc 2.34 or newer on Linux | the run on Rust 1.85.1 (`cargo +1.85 test`); `scripts/check-glibc.sh` ran by hand on the release binary | gap: no test for the glibc figure; the GitHub job Highest glibc symbol passed on run 45577649641 |
| W23 | A backslash in a file name is doubled once in an error | `backslash_in_a_path_is_escaped_exactly_once`, `shown_path_leaves_escaping_to_the_error` | tested |
| W24 | No local path, internal name or work file name in the documents, the man page, the sources, the tests or the CI files | `documents_code_tests_and_ci_hold_no_local_paths_or_work_file_names`, `the_scan_reaches_the_ci_file_and_the_sources`, `the_check_finds_each_pattern` | tested |
| W25 | `--as-of` is a day: an object last modified on or before it is eligible; the time of day is not read; help, README and man agree | `as_of_day_is_the_whole_day_and_the_next_day_is_not_eligible`, `as_of_help_names_the_day_rule_in_help_readme_and_man` | tested |
| W26 | With `--json` warnings go to standard error; in the text report they are report lines on standard output | `warnings_go_to_stderr_with_json_and_savings_name_four_targets`, `text_report_keeps_warnings_on_stdout_and_stderr_empty` | tested |
| W27 | ASCII white space around Size is ignored; a sign, `_`, `0x`, a non-ASCII digit, a non-breaking space or an empty field is `invalid Size` | `size_with_ascii_space_is_read_and_a_sign_or_other_digits_are_refused` | tested |
| W28 | Usage errors show control characters escaped on one line | `usage_error_escapes_raw_control_characters`, `usage_errors_exit_2` | tested |
| W29 | A Parquet or ORC inventory (by name or by magic, also inside gzip) is refused with exit 2 and a text that says only CSV and gzip CSV are read | `parquet_and_orc_names_exit_2`, `claim_c17_parquet_orc_gzip_and_no_sdk`, `extension_check_ignores_case_and_covers_every_listed_suffix` | tested |
| W30 | No minimum Windows or macOS version has been determined; a musl build is only linted | none; stated as a limit | gap: a statement of what was not done |
| W31 | A `Size` is any number of leading zeros and at most 18446744073709551615 | `size_with_leading_zeros_is_read_whatever_its_length`, `parse_size_accepts_the_limit_and_refuses_one_more` | tested |
| W32 | A very large valid total with a high rate is priced, not refused for an intermediate loss | `large_valid_totals_with_a_high_rate_are_not_refused_for_an_intermediate_loss`, `wide_share_rounds_once_and_keeps_the_sum`, `share_survives_intermediate_overflow_when_total_fits` | tested |
| W33 | White space is space, tab, CR, LF and form feed; class names ignore ASCII case; a NUL after 512 bytes is part of the key; a header that starts with `ORC` is refused; bytes after the last gzip member are refused; `--prefix-depth` reads `08` and `+3` | `readme_input_rules_hold_for_white_space_class_names_nul_and_sniffing` | tested |
| W34 | Number limits: 64-bit byte sum (also with `--as-of`), price digits, `*_min_days`, `min_billable_bytes`, `--as-of` years, 28 significant digits | `readme_number_limits_hold_for_sizes_prices_days_and_dates`, `extreme_price_exits_2_without_panic` | tested |
| W35 | `exclude` in `Cargo.toml` lists exactly the two local work files, and the manifest holds no internal name | `the_manifest_holds_no_internal_name_and_excludes_exactly_the_work_files` | tested |
| W36 | The package holds `man/` and `SECURITY.md` and no local work file | none; read from `cargo package --list` by hand | gap: no test |
| W37 | A gzip file has at most 100000 members | `gzip_with_very_many_members_is_refused_in_a_moment` | tested |
| W38 | Text after a closing quote is an error; quoted fields still read | `text_after_a_closing_quote_is_refused_and_a_quoted_field_still_reads` | tested |
| W39 | A CR-only file is refused also when its last column is optional | `cr_only_file_is_refused_also_when_the_last_column_is_optional` | tested |
| W40 | A `--prices` file is at most 1048576 bytes | `a_price_file_over_one_mebibyte_is_refused_and_one_at_the_limit_loads` | tested |
| W41 | Memory and time of distinct class names, of a plain file at the limit and of a file of 5,000,000 quoted fields (the figures in the README) | `many_distinct_prefixes_are_priced_in_a_moment` (time only, 40,000 prefixes in under 4 s in debug and not more than about 3 times the time of 20,000); memory measured by hand on 2026-11-20 | tested (time); gap: no test measures memory |
| W42 | The monthly cost is labelled `estimated_usd_month` in the text and the JSON, the text report has the one line that says it is a list-price estimate, and the JSON has `"estimate": true` | `monthly_cost_is_labelled_as_an_estimate` | tested |
| W43 | Price data more than 180 days before `--as-of` (or before today, which `SOURCE_DATE_EPOCH` can fix) gives a warning that names its date, with exit 0; exactly 180 days gives none | `old_price_data_warns_after_180_days_and_still_exits_0` | tested |
| W44 | An inventory in the documented shape (the `fileSchema` names as header, every field quoted, URL-encoded keys, optional fields missing, an empty list, several files with one gzip, special characters in keys, an access tier column) is read without error and priced exactly | `full_schema_with_quoted_fields_and_url_encoded_keys`, `optional_fields_missing_and_columns_in_another_order`, `empty_lists_give_a_zero_report`, `several_files_and_a_gzip_file_add_up_exactly`, `keys_with_special_characters_stay_on_one_line_and_are_counted`, `intelligent_tiering_access_tier_column_is_read` | tested against the shape documented at <https://docs.aws.amazon.com/AmazonS3/latest/userguide/storage-inventory.html> (the S3 Inventory chapter of the Amazon S3 User Guide; no network); gap: the tier names and the delete-marker rows of the docs were not checked against the live page |
| W45 | `--quiet` leaves the report and the errors and drops the warning copies on standard error; `--verbose` adds progress lines on standard error and leaves standard output; both together exit 2 | `quiet_keeps_the_report_and_drops_the_stderr_warnings`, `quiet_still_shows_errors_with_exit_2`, `verbose_adds_progress_on_stderr_only`, `verbose_escapes_a_control_character_in_a_path`, `quiet_with_verbose_is_a_usage_error` | tested |
| W46 | `--md` prints the same amounts as the text report and the JSON (class, total, prefix and lifecycle cells), one estimate line, escaped pipe characters and line breaks; with `--json` it exits 2 | `markdown_amounts_equal_text_and_json`, `pipes_and_line_breaks_in_cells_are_escaped`, `markdown_without_as_of_says_the_lifecycle_is_omitted`, `md_and_json_cannot_be_combined_and_help_lists_md`, `markdown_has_the_same_report_with_quiet_and_nothing_on_stderr`, `readme_markdown_example_is_what_the_binary_prints` | tested |
| W47 | A real SIGINT while a large input is read exits 130 with one line on standard error and nothing on standard output; a SIGINT during the final write waits for it | `sigint_while_reading_exits_130_with_one_line_and_no_output` | tested (Linux, reading phase); gap: the held-back write phase has no test, a stdout that blocks cannot be made to block on a fixed point
| W48 | `--output` writes the whole report to a file by rename, replaces an existing file, replaces and does not follow a symbolic link, refuses a directory or special file, and fails with exit 2; SIGINT keeps the old file | `the_report_goes_to_the_file_and_not_to_standard_output`, `an_existing_file_is_replaced_whole`, `a_symlink_is_replaced_and_never_followed`, `write_failures_are_exit_2_with_a_message_and_nothing_is_created`, `a_special_file_is_refused_and_a_read_only_directory_is_exit_2`, `a_failed_run_leaves_an_existing_output_file_as_it_was`, `json_warnings_stay_on_stderr_and_a_control_character_in_the_path_is_escaped`, `sigint_leaves_an_existing_output_file_as_it_was_and_creates_none` | tested; gap: a full disk was not tested |
| W49 | Every JSON document has `"schema_version": 1` as its first key, also with `--output`, `--quiet`, `--verbose`, `--prices` and in the library; an error with `--json` prints no JSON, exit 2 | `every_json_path_has_schema_version_first`, `schema_version_is_documented_in_the_readme_and_quality` | tested |
