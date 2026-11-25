# Examples

`inventory.csv` is a one-object S3 Inventory file with a header line: one Standard object of 1 GiB under `logs/app/`, last modified on 2024-01-15.

Run it from the root of a clone. The command only reads the file and writes the report to standard output:

```sh
s3-inventory-bill --as-of 2026-11-09 examples/inventory.csv
```

`inventory.expected` is the exact output of that command, with the price data of 2026-11-09 and the clock fixed by `SOURCE_DATE_EPOCH=1795132800` (no warning about old prices). A test runs the command and compares the output to this file, and the README Quick start shows the same text.

The same file with `--json` or `--md` is in the README, under "Example".
