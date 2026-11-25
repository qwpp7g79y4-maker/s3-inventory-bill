# s3-inventory-bill

Estimate S3 storage cost from an Inventory CSV. Reads files only, no network.

The tests and the measured numbers are in `QUALITY.md`.

What it reads and does: it reads the inventory CSV files (or gzip CSV) named on the command line and, with `--prices`, one price file. It makes no network call, needs no AWS account, token or paid service, and writes nothing except the report on standard output, or the file named by `--output`. It starts no other process.

## Why

An S3 Inventory export already lists each object's size and storage class. s3-inventory-bill prices that CSV on your machine, per class and per prefix, and shows what a lifecycle move of eligible Standard objects to Standard-IA or Glacier would change. It does not call AWS and it does not need an account.

The first row of every file must be a header line that names the columns `Bucket`, `Key`, `Size` and `StorageClass`, in any order; `LastModifiedDate` and `IntelligentTieringAccessTier` are optional. A raw S3 Inventory CSV has no header line, so add one that follows the column order in the inventory's `manifest.json` (`fileSchema`). A file without it is refused with exit 2 and a message that lists the expected columns. Keys are used as written: the tool does not URL-decode them.

## Installation

Version 1.0.0. This crate is not published and version 1.0.0 has no git tag, so there is no crates.io or binary-archive install yet. Build from this checkout with Rust 1.85 or newer:

```sh
cargo install --locked --path .
s3-inventory-bill --help
```

The licenses of the third-party crates in the binary are in [THIRD-PARTY-LICENSES.html](THIRD-PARTY-LICENSES.html). A release archive includes that same file. Until a release exists, `cargo install s3-inventory-bill` from crates.io does not work; install from this checkout.

Supported systems: Linux is the tested system (glibc 2.34 or newer for a glibc build). The Windows and macOS builds are only type-checked here, and a musl build has only been linted (see "Limitations" and [QUALITY.md](QUALITY.md)).

## Quick start

`examples/inventory.csv` is one Standard object of 1 GiB under `logs/app/`. Running the tool on it only reads it. To try the tool outside a clone, write the same bytes to a temporary directory and read that:

```sh
dir=$(mktemp -d)
cat > "$dir/inventory.csv" <<'CSV'
Bucket,Key,Size,LastModifiedDate,StorageClass
example-bucket,logs/app/a.bin,1073741824,2024-01-15T00:00:00.000Z,STANDARD
CSV
s3-inventory-bill --as-of 2026-11-09 "$dir/inventory.csv"
```

A test runs every console block below, from the root of a clone, and compares it to this binary. `--as-of` is a day: an object last modified on or before that day is eligible. The sample object was written on 2024-01-15, which is on or before 2026-11-09, so it is eligible. Rates are US East (N. Virginia) from <https://aws.amazon.com/s3/pricing/> as read on 2026-11-09.

```console
$ s3-inventory-bill --as-of 2026-11-09 examples/inventory.csv
s3-inventory-bill
source: https://aws.amazon.com/s3/pricing/
as of: 2026-11-09
region: us-east-1 (US East (N. Virginia))
prefix_depth: 1
Estimate from list prices, not your AWS invoice (free tier, credits, discounts not included)

storage by class
STANDARD objects=1 raw_bytes=1073741824 billable_bytes=1073741824 estimated_usd_month=0.023
total objects=1 raw_bytes=1073741824 estimated_usd_month=0.023

storage by prefix
logs/ objects=1 raw_bytes=1073741824 estimated_usd_month=0.023

lifecycle
as-of: 2026-11-09
STANDARD_IA horizon_days=30 eligible_objects=1 monthly_now_usd=0.023 monthly_after_usd=0.0125 transition_request_usd=0.00001 early_delete_usd=0.00 savings_over_horizon_usd=0.01049 Not executed
GLACIER_IR horizon_days=90 eligible_objects=1 monthly_now_usd=0.023 monthly_after_usd=0.004 transition_request_usd=0.00002 early_delete_usd=0.00 savings_over_horizon_usd=0.05698 Not executed
GLACIER horizon_days=90 eligible_objects=1 monthly_now_usd=0.023 monthly_after_usd=0.00360028533935546875 transition_request_usd=0.00003 early_delete_usd=0.00 savings_over_horizon_usd=0.05816914398193359375 Not executed
DEEP_ARCHIVE horizon_days=180 eligible_objects=1 monthly_now_usd=0.023 monthly_after_usd=0.0009902056884765625 transition_request_usd=0.00005 early_delete_usd=0.00 savings_over_horizon_usd=0.132008765869140625 Not executed
```

1 GiB of Standard is 0.023 USD per month. Moving that object to Standard-IA for 30 days saves 0.01049 USD after the 0.00001 transition request. Glacier Flexible Retrieval and Glacier Deep Archive also bill 8 KiB at Standard rates and 32 KiB at the cold rate, so `monthly_after_usd` is the object plus that metadata, not the object alone. Each target line ends with `Not executed`. Without `--as-of` the lifecycle block is `omitted: pass --as-of YYYY-MM-DD` and only storage is priced.

## Aan de slag in 5 minuten

Korte route in het Nederlands; de Nederlandse README (`README.nl.md`) heeft dezelfde koppen als deze.

1. Bouw de tool met Rust 1.85 of nieuwer: `cargo install --locked --path .`.
2. Zet in je S3 Inventory-export een kopregel boven de data, in de kolomvolgorde van `manifest.json` (`fileSchema`): `Bucket`, `Key`, `Size`, `StorageClass`, en liefst `LastModifiedDate`.
3. Draai `s3-inventory-bill --as-of 2026-11-09 inventory.csv` (vervang de datum door vandaag). Zonder `--as-of` krijg je alleen de opslagkosten, geen besparingen.
4. Lees het rapport: kosten per opslagklasse en per prefix, daarna per doelklasse wat een lifecycle-overgang zou schelen. Elk bedrag is een schatting uit lijstprijzen, geen AWS-factuur.
5. Gebruik `--json` voor een script, `--md` voor een pull request of wiki, en `--output bestand` om het rapport in een bestand te zetten.


## Options

All options and their defaults, as printed by `s3-inventory-bill --help`:

```text
Estimate S3 storage cost from an inventory CSV. Reads files only, no network.

Usage: s3-inventory-bill [OPTIONS] <FILE>...

Arguments:
  <FILE>...  S3 Inventory CSV files, or gzip-compressed CSV

Options:
      --prices <FILE>                Price file. The default is the built-in US East (N. Virginia) list
      --as-of <YYYY-MM-DD>           Day that ages are measured to, `YYYY-MM-DD`. An object last modified on or before that day is eligible; the time of day is not read. Without it, savings are omitted
      --prefix-depth <PREFIX_DEPTH>  Path segments in the prefix, 1 to 8. `logs/2024/a` at depth 1 is `logs/` [default: 1]
      --json                         Print JSON instead of the text table
      --md                           Print Markdown tables instead of the text table
      --output <FILE>                Write the report to this file instead of standard output. The file is written whole and renamed into place; an existing file is replaced, a symbolic link is replaced and not followed
      --quiet                        Print only the report and errors: no warning copies on standard error
      --verbose                      Also print progress lines on standard error
  -h, --help                         Print help
  -V, --version                      Print version

Exit codes: 0 ok, 2 input or usage error, 130 interrupted (SIGINT).
```

Input: one or more files, each a CSV with a header line, or a gzip file of such a CSV. The first row names the columns (see "Why"). The report is text by default; `--json` and `--md` choose another form, and the two cannot be combined (a usage error, exit 2). `--prefix-depth` defaults to 1.

Every JSON document starts with `"schema_version": 1`, the version of its layout; the number changes only when a field is renamed, removed or changes meaning. An error never prints JSON: a failed run with `--json` has an empty standard output and the message on standard error (exit 2), so every JSON document that is printed has the field.

Every amount is an estimate from list prices: the report says so in one line, and the JSON has `"estimate": true` at the top level. Free tier, credits and discounts are not included, so the figure can differ from your AWS invoice. The monthly cost is named `estimated_usd_month` in the text and in the JSON.

If the date of the price file (the built-in list or `--prices`) is more than 180 days before `--as-of`, or before today when there is no `--as-of`, the report carries a warning that names that date. The exit code stays 0. `SOURCE_DATE_EPOCH` (seconds since 1970-01-01 UTC), when set, stands in for the clock, so a run can be repeated.

`--json` prints the same totals. Money and byte totals are strings. With `--json`, warnings, when there are any, go to standard error; in the text report they are lines at the end of the report on standard output. This sample has none.

`--md` prints the same amounts as Markdown tables, with the estimate line, for a pull request or a wiki. A `|` or a line break in a class or prefix name is escaped, so it cannot end a cell. It cannot be combined with `--json`; that is a usage error with exit 2.

`--quiet` prints only the report and errors: with `--json` it leaves out the warning copies on standard error, and the text report is unchanged. `--verbose` adds progress lines on standard error (`s3-inventory-bill: reading FILE`, `pricing N files`, `done`) and leaves standard output as it is. Giving both is a usage error with exit 2.

`--output FILE` writes the report to a file instead of standard output (the text, `--json` and `--md` forms alike), and prints nothing on standard output. The report is written to a temporary file in the same directory and renamed over FILE, so FILE is the old report or the whole new one. An existing file is replaced. A symbolic link at FILE is replaced by the new file and is not followed. A directory, a pipe or a device is refused. A missing directory is not created. Any failure is exit 2 with `s3-inventory-bill: could not write report to FILE: REASON` and no panic. With `--json`, the warnings still go to standard error.

Ctrl-C (SIGINT) ends the run with exit 130 and the line `s3-inventory-bill: interrupted by SIGINT` on standard error, and nothing is printed on standard output. The report is written in one step at the end, and a SIGINT that arrives during that step waits until the step is complete. With `--output`, a SIGINT before the rename leaves an existing FILE as it was and creates no file. On Windows the platform's own Ctrl-C handling applies and the exit code is not 130.

## Example

`--md` on the sample prints (a test compares this block to the binary):


```markdown
# s3-inventory-bill

Estimate from list prices, not your AWS invoice (free tier, credits, discounts not included)

- Source: https:\//aws.amazon.com/s3/pricing/
- As of: 2026-11-09
- Region: us-east-1 (US East (N. Virginia))
- Prefix depth: 1

## Storage by class

| Class | Objects | Raw bytes | Billable bytes | Estimated USD per month |
| --- | ---: | ---: | ---: | ---: |
| STANDARD | 1 | 1073741824 | 1073741824 | 0.023 |
| **Total** | 1 | 1073741824 |  | 0.023 |

## Storage by prefix

| Prefix | Objects | Raw bytes | Estimated USD per month |
| --- | ---: | ---: | ---: |
| logs/ | 1 | 1073741824 | 0.023 |

## Lifecycle

As of: 2026-11-09

| Target | Horizon days | Eligible objects | Monthly now USD | Monthly after USD | Transition request USD | Early delete USD | Savings over horizon USD | Status |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | --- |
| STANDARD_IA | 30 | 1 | 0.023 | 0.0125 | 0.00001 | 0.00 | 0.01049 | Not executed |
| GLACIER_IR | 90 | 1 | 0.023 | 0.004 | 0.00002 | 0.00 | 0.05698 | Not executed |
| GLACIER | 90 | 1 | 0.023 | 0.00360028533935546875 | 0.00003 | 0.00 | 0.05816914398193359375 | Not executed |
| DEEP_ARCHIVE | 180 | 1 | 0.023 | 0.0009902056884765625 | 0.00005 | 0.00 | 0.132008765869140625 | Not executed |
```

The JSON for the sample:

```console
$ s3-inventory-bill --json --as-of 2026-11-09 examples/inventory.csv
{
  "schema_version": 1,
  "estimate": true,
  "source_url": "https://aws.amazon.com/s3/pricing/",
  "source_date": "2026-11-09",
  "region": "us-east-1",
  "region_name": "US East (N. Virginia)",
  "prefix_depth": 1,
  "objects": 1,
  "raw_bytes": "1073741824",
  "estimated_usd_month": "0.023",
  "classes": [
    {
      "class": "STANDARD",
      "objects": 1,
      "raw_bytes": "1073741824",
      "billable_bytes": "1073741824",
      "estimated_usd_month": "0.023"
    }
  ],
  "prefixes": [
    {
      "prefix": "logs/",
      "objects": 1,
      "raw_bytes": "1073741824",
      "estimated_usd_month": "0.023"
    }
  ],
  "lifecycle": {
    "as_of": "2026-11-09",
    "targets": [
      {
        "target": "STANDARD_IA",
        "horizon_days": 30,
        "eligible_objects": 1,
        "monthly_now_usd": "0.023",
        "monthly_after_usd": "0.0125",
        "transition_request_usd": "0.00001",
        "early_delete_usd": "0.00",
        "savings_over_horizon_usd": "0.01049",
        "not_executed": true
      },
      {
        "target": "GLACIER_IR",
        "horizon_days": 90,
        "eligible_objects": 1,
        "monthly_now_usd": "0.023",
        "monthly_after_usd": "0.004",
        "transition_request_usd": "0.00002",
        "early_delete_usd": "0.00",
        "savings_over_horizon_usd": "0.05698",
        "not_executed": true
      },
      {
        "target": "GLACIER",
        "horizon_days": 90,
        "eligible_objects": 1,
        "monthly_now_usd": "0.023",
        "monthly_after_usd": "0.00360028533935546875",
        "transition_request_usd": "0.00003",
        "early_delete_usd": "0.00",
        "savings_over_horizon_usd": "0.05816914398193359375",
        "not_executed": true
      },
      {
        "target": "DEEP_ARCHIVE",
        "horizon_days": 180,
        "eligible_objects": 1,
        "monthly_now_usd": "0.023",
        "monthly_after_usd": "0.0009902056884765625",
        "transition_request_usd": "0.00005",
        "early_delete_usd": "0.00",
        "savings_over_horizon_usd": "0.132008765869140625",
        "not_executed": true
      }
    ]
  },
  "warnings": []
}
```

## How it counts

Built-in rates are the US East (N. Virginia) figures on <https://aws.amazon.com/s3/pricing/> as of 2026-11-09. The page fills its tables from the public pricing map published 2026-11-09T15:43:10Z. Glacier Deep Archive storage is the page rate 0.00099 USD per GB-month; that map was published 2026-09-11T12:45:07Z. 1 GB is 1,073,741,824 bytes. 1 TB is 1,024 GB. Standard uses three tiers on the aggregate: the first 50 TB at 0.023, the next 450 TB at 0.022, and the rest at 0.021. `--prices` replaces the whole quote. Pass another file with the same fields when the region is not us-east-1.

C1. One inventory row is one object. The same key in two files is counted twice.
C2. 1 GiB is 1,073,741,824 bytes. 1 TB is 1,024 GB, which is 1,099,511,627,776 bytes.
C3. Standard storage in the first 50 TB is 0.023 USD per GB-month. The source is https://aws.amazon.com/s3/pricing/ as of 2026-11-09, region us-east-1.
C4. Standard-IA bills at least 128 KiB per object. The savings horizon is 30 days.
C5. One Zone-IA bills at least 128 KiB per object. Its storage rate is 0.01 USD per GB-month.
C6. Glacier Instant Retrieval bills at least 128 KiB per object. The savings horizon is 90 days and the rate is 0.004 USD per GB-month.
C7. Glacier Flexible Retrieval adds 8 KiB at Standard rates and 32 KiB at the Glacier rate for every object, including a 0-byte object. The horizon is 90 days and the rate is 0.0036 USD per GB-month.
C8. Glacier Deep Archive uses the same 8 KiB and 32 KiB metadata split, at the Deep Archive rate of 0.00099 USD per GB-month. The horizon is 180 days.
C9. An Intelligent-Tiering object under 128 KiB stays on Frequent Access and pays no monitoring fee. An object of 128 KiB or more is monitored at 0.0000025 USD, which is 0.0025 USD per 1,000 objects.
C10. Express One Zone is 0.11 USD per GB-month. There is no 128 KiB floor.
C11. A lifecycle transition is priced per request: Standard-IA, One Zone-IA, and Intelligent-Tiering 0.00001; Glacier Instant Retrieval 0.00002; Glacier Flexible Retrieval 0.00003; Glacier Deep Archive 0.00005. One thousand monitoring charges are 0.0025 USD.
C12. Each savings line ends with the words Not executed. Without `--as-of` the lifecycle block is omitted.
C13. Storage is priced whether or not `--as-of` is set. Savings are omitted when it is absent. The clock is not used as a default date.
C14. A missing, empty, invalid, or future LastModifiedDate stays in the storage cost and is left out of savings. The warning does not echo the date or the key.
C15. Prefix depth 1 of `logs/2024/a.bin` is `logs/`. Depth 2 is `logs/2024/`. A key with fewer segments stays the key, so `pics/c` at depth 2 is `pics/c`. Standard dollars for a prefix are that prefix's share of the tiered Standard total.
C16. REDUCED_REDUNDANCY and any other unknown class are unpriced. They still count as objects and raw bytes. They are excluded from the dollar total.
C17. Gzip CSV is inflated and then priced. Parquet and ORC, including a gzip that inflates to those magics, are rejected. The crate does not link an AWS SDK.
C18. `--prices` replaces the built-in quote. An unknown JSON field is ignored. A duplicate key at any depth is an error, and the message is `duplicate key`.
C19. The dollar total is the exact sum of the parts of all priced classes, rounded once, not the sum of the rounded class amounts. A class made of several parts (Intelligent-Tiering, Glacier, Deep Archive) is also the exact sum of its parts, rounded once. JSON money and byte totals are strings. Each amount is rounded once, half away from zero (see "Numbers" below). Each prefix is its share of the tiered Standard total, divided on its own, so the prefix amounts can differ from the total in the last decimal. The whole difference is added to the largest prefix. Added up outside the tool, the prefix amounts can still differ from the total in the last digits. With a very large total the correction is too small for the decimal precision and a last-digit gap stays on that largest prefix.
C20. A Size that is empty, signed, or not an integer is `FILE: row N: invalid Size`, where `FILE` is the path as given, also when there is only one file. `N` is the line number in the file, blank lines included. The message does not include the value or the key.
C21. Glacier metadata is stacked on the Standard tier schedule after the Standard object bytes, and those dollars sit on the cold class. One Standard object of 50 TB minus 4,096 bytes plus one 0-byte Glacier object prices 4,096 metadata bytes at 0.023 and 4,096 at 0.022.
C22. A 1,000 byte Standard object moved to Standard-IA can show a negative saving, because the 128 KiB floor costs more than the small object does today.
C23. Retrieval and GET fees are not added. 1 GiB of Standard-IA is 0.0125 USD per month.
C24. Every file is accumulated first and priced once, so Standard tiers apply to the sum. 50 TB minus 1 GiB in one file plus 2 GiB in another is 1177.622 USD.

Eligible Standard objects are priced on their own tier schedule. The early-delete remainder for a Standard source is 0.00, and that field is still printed. One Zone-IA and Intelligent-Tiering have transition rates in the quote. They are not savings targets.

## Exit codes

| Code | Meaning |
| ---: | --- |
| 0 | The report was written. Warnings (for example old price data) do not change this. `--help` and `--version` also exit 0. |
| 2 | An input or usage error: an unknown or conflicting option, a file that cannot be read or is refused, a bad price file, an amount or byte total that does not fit, or a standard output (or `--output` file) that cannot be written. The message is on standard error and nothing is printed on standard output. An error never prints JSON. |
| 130 | Interrupted by Ctrl-C (SIGINT). Nothing is printed on standard output. |

The tool does not panic on bad input; a failure is exit 2 with a one-line message that does not repeat a cell or key.

## Limitations

Scope:

- The tool reads CSV only. parquet and orc inventories are rejected, and so is a gzip of those magics.
- The lifecycle line is not executed: the tool never runs a transition. There is no AWS SDK and no network call.
- The built-in quote is us-east-1. Object retrieval is not in this storage bill. The account free tier not applied: nothing is waived. Reduced Redundancy unpriced, and so is any class outside the quote.
- Savings assume the object remains for the whole horizon. This is not a forecast.

Input rules:

- A file above 1073741824 bytes is refused, on disk and after gzip. A symlink to a file is followed. A directory is an error, and so is a pipe, a device or any other file that is not a regular file (`not a file`). `--prices` accepts only a regular file of at most 1048576 bytes (`file is larger than 1048576 bytes`); the built-in quote is about 2 KB. A gzip file has at most 100000 members (`gzip file has more than 100000 members`); every member costs time even when it is empty.
- The first row is the header line (see "Why"). A record has at most 64 columns, quoted or not. A quote that opens inside an unquoted field is an error, and so is any text after the closing quote of a quoted field other than a comma or the end of the line (`row N: text after a closing quote`), so `"1"0` is not read as 10.
- Line ends are LF or CRLF. A file whose only line ends are CR is read as one line and refused, with `missing column ...` when a required column is not found, or with `the first row is not a header line` when it is (for instance `LastModifiedDate` is the last column); both messages say that the file may use only CR line ends.
- A `LastModifiedDate` is read as `YYYY-MM-DD`, followed by the end of the field, a `T` or a space. Nothing after that is checked, so `2024-01-15T99:99:99.000Z` counts as 2024-01-15. `--as-of` compares days only; an object modified on the `--as-of` day is eligible.
- `--prefix-depth` is 1 to 8; `08` and `+3` are read as 8 and 3. Any other value exits 2.
- An empty key and a key written `(empty)` fall into the same prefix line.
- A `Size` is ASCII digits only, with any number of leading zeros, and at most 18446744073709551615. White space around it, and around `Bucket` and `StorageClass`, is ignored (space, tab, CR, LF and form feed; a vertical tab is not), so ` 5` and `5 ` are 5 bytes; a sign, an underscore, `0x`, a non-ASCII digit, a non-breaking space or an empty field is `invalid Size`. A key is used as written, spaces included.
- A storage class is matched without regard to ASCII case: `standard` is `STANDARD`. An unknown class is shown in capitals.
- A NUL byte in the first 512 bytes means `not a CSV inventory`; a NUL byte later in the file is part of the key and is shown escaped (`\0`). A file that starts with the letters `ORC` or `PAR1` is refused as that format, also when it is CSV (a header that starts with `ORC_ID` is refused). Bytes after the last gzip member, even zeros, mean `not a gzip inventory`.
- The same key counts twice when two rows list it. An Intelligent-Tiering tier named `ARCHIVE` or `DEEP_ARCHIVE` is read like `ARCHIVE_ACCESS` or `DEEP_ARCHIVE_ACCESS`.
- An error about a file starts with that file's path, also when only one file is given.
- Prices in a `--prices` file are decimal numbers of at most 28 decimals; only space, tab, CR and LF around them are trimmed.

Numbers:

- Bytes are 64-bit. The sum of all `Size` values must be at most 18446744073709551615, or the run ends with `byte total does not fit`. With `--as-of` the sum is also taken with every object counted at least at the minimum billable size, so an input at that limit that is accepted without `--as-of` can be refused with it (one object of 18446744073709551615 bytes plus one empty object).
- Money keeps at most 28 decimal places, and fewer when the digits in front of the point need the room, so a large amount has at most 28 significant digits. Each amount is rounded once, at the end, half away from zero. 3 GiB at 3.3333333333333333333333333333 per GiB is 9.9999999999999999999999999999 exactly; that value does not fit, and the one rounding shows `10.00`. An amount above 79228162514264337593543950335 is `amount does not fit`.
- In a `--prices` file the digits of a price, written without the point, are at most 79228162514264337593543950335, so `9.9999999999999999999999999999` is refused as `price file FILE: bad standard_first_50_tb`. The `*_min_days` fields are whole numbers from 1 to 36500, and `min_billable_bytes` is a whole JSON number (not `131072.0`, not a string, not negative). That message names the field, not its value.
- `--as-of` is a real day from 0000-01-01 to 9999-12-31.

Output:

- If stdout is closed or a write to it fails, the tool exits 2 with `could not write output` on stderr. A closed stdout is recognized on Linux only.

Memory and time:

- Memory follows the number of distinct prefixes and of distinct storage class names, not only the file size. A plain file is held once, and a gzip file is held packed and inflated. Measured on Linux with the release build, text report, one run each after the last code change (rows over eight storage classes, each row its own prefix, sizes from 1,000,003 bytes up): 500,000 prefixes (17.7 MB) 1.35 s and 346 MB of resident memory, 1,000,000 prefixes 2.67 s and 694 MB, 2,000,000 prefixes (72.5 MB) 5.42 s and 1.39 GB. Twice the input took 1.98 times as long and 2.00 times the memory, four times took 4.0 times and 4.0 times. With `--json` the same files took 2.9 s, 6.0 s and 11.7 s, and 2,000,000 prefixes needed 4.9 GB. The same shape with 50,000, 100,000 and 200,000 prefixes took 0.13 s, 0.26 s and 0.53 s; the version before this change took 7.4 s, 15.3 s and 33.5 s on them. 1,500,000 rows (45.4 MB) with 10 distinct prefixes took 0.31 s and 46 MB. 1,000,000 rows (29.8 MB) with one prefix and each its own class name took 1.32 s and 339 MB; with each its own prefix as well 3.21 s and 1.36 GB. A file of exactly 1073741824 bytes with two distinct prefixes took 3.9 s and 1.05 GB (measured before this change; the pricing of two prefixes is not what that time is spent on). A file at the limit with only distinct prefixes or class names would need much more; that was not run. The memory is not bounded by this tool.
- One line with 5,000,000 quoted fields (20 MB) is refused with `too many columns` in 0.05 s, using about the memory of the file; the 65th column stops the read. A gzip file of 100,000 empty members is read in 0.45 s.

System requirements:

- Rust 1.85 or newer to build. The Linux binary built with glibc needs GLIBC_2.34 or newer. Windows and macOS builds are only type-checked here; their CI jobs are the evidence once they run. No minimum Windows or macOS version has been determined, and a musl build has only been linted, not run.

The test suite and what it covers are in [QUALITY.md](QUALITY.md).

## Prices and responsibility

The built-in prices are a snapshot: US East (N. Virginia) list prices from <https://aws.amazon.com/s3/pricing/>, dated 2026-11-09 (the `source_date` in the price file). They age. Check the current prices yourself. You are responsible for any decision you take on the basis of the output; it is an estimate, not an invoice. Pass your own price file with `--prices`.

## Maintenance

Report bugs through GitHub Issues of this repository. After a release the tool is in dormant mode: a new version appears only for a reported bug, a request or a security problem, and security always comes first.

## License

MIT OR Apache-2.0. See [LICENSE-MIT](LICENSE-MIT) and [LICENSE-APACHE](LICENSE-APACHE). [THIRD-PARTY-LICENSES.html](THIRD-PARTY-LICENSES.html) lists the third-party crates in the binary and their license texts. A release archive includes that same file.

## Support

Report a bug or ask a question through GitHub Issues of this repository. For a security problem use the private route in [SECURITY.md](SECURITY.md), not a public issue. Contributions: see [CONTRIBUTING.md](CONTRIBUTING.md). Never attach a real inventory; a small synthetic CSV that shows the problem is enough.

## Contact

- E-mail: pep_ai@icloud.com
- Web: <https://www.pepai.nl>
- X: <https://x.com/pepai_nl>
- Ko-fi: <https://ko-fi.com/pepai>
