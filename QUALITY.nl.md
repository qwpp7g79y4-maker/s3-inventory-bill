# Kwaliteit

pep: 10

Deze commit wijzigt alleen documentatie; de meting hieronder geldt voor codecommit `5f02e4a`.

Suite: 217 geslaagd, 0 mislukt, 0 genegeerd.

Deze pagina legt controles vast. Het is geen garantie en geen fuzz-rapport. Deze versie is niet gepubliceerd en heeft geen git-tag. De Engelse pagina (`QUALITY.md`) heeft dezelfde inhoud en is leidend bij een verschil.

Gemeten op 2026-11-23, op Linux (x86_64), op de code-commit `5f02e4a65fb6f53625ed83678d0df820f7accd07`. Alleen de vijf commando's in de eerste tabel hieronder zijn op die commit gedraaid, vanuit de root van de crate, met `--locked`, als `cargo fmt --check`, `cargo clippy ...` en `cargo test --locked`; er was geen geheugenlimiet en geen optie `-j` of `--test-threads`. De rijen van de tweede tabel zijn op 2026-11-20 gemeten op de eerdere codecommit `6b36822` en niet opnieuw uitgevoerd. GitHub-CI op `main` commit `7d7c78befbcb71a4132cc6e174d0ba452037c0a0` is geslaagd: run 45582606112, jobs ubuntu-latest, ubuntu-latest met Rust 1.85, windows-latest, macos-latest, macos-15-intel, Dependency licenses, Dependency audit en Highest glibc symbol. Commits na `5f02e4a` wijzigden ook `src/money.rs`, `src/main.rs`, `src/json_scan.rs`, tests en `.github/workflows/release.yml`. Alleen echte meetwaarden staan hier; een controle die niet is uitgevoerd staat onder "Niet uitgevoerd".

Toolchain: rustc 1.99.0 (b940084d7 2026-09-28) and cargo 1.99.0 (5f94df478 2026-08-27). For the minimum supported version: rustc 1.85.1 (4eb161250 2025-03-15) and cargo 1.85.1 (d73d2caf9 2024-12-31). cargo-deny 0.20.2, cargo-audit 0.22.2, cargo-about 0.9.2.

## Commando's

Exitcodes van de commando's die op de hierboven genoemde commit zijn gedraaid (217 geslaagd, 0 mislukt):

| Command | Exit |
| --- | ---: |
| `cargo fmt --check` | 0 |
| `cargo clippy --offline --locked --all-targets -- -D warnings` (stable 1.99.0) | 0 |
| `cargo +1.85 clippy --offline --locked --all-targets -- -D warnings` | 0 |
| `cargo test --locked` (debug, stable): 217 passed, 0 failed | 0 |
| `cargo +1.85 test --locked` (debug): 217 passed, 0 failed | 0 |

Gemeten op de eerdere codecommit `6b36822` (2026-11-20) en niet opnieuw uitgevoerd op de commit hierboven:

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

De Windows- en macOS-testjobs draaiden op GitHub-CI-run 45582606112 en slaagden. Een musl-build is alleen gelint, niet gedraaid.

## Twee gaten, gemeten voor en na

Beide gemeten op de tip `619e7a6` met de release-binary, en dezelfde invoer na de wijziging op codecommit `5f02e4a`. Invoer: `pj_b1.json` is `prices/s3-storage.json` with `source_url` set to `https://x.example/[click](https://evil.example/p)<img/src=//evil.example/t.png>*b*`+"`c`"+`; `forge.csv` heeft de sleutels `real/data.bin`, `total objects=0 raw_bytes=0 estimated_usd_month=0.00`, `warning: prices are current` and `lifecycle`. Alleen deze commando's zijn gedraaid; er is geen Markdown-renderer gedraaid.

Voor (619e7a6):

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

Na (5f02e4a):

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

In het tekstrapport begint een sleutel die met `total` of `warning` begint nu met `\u{74}` of `\u{77}`; de echte totaalregel blijft. `--md` schrijft `source_url` door dezelfde escape als de sleutels. De JSON-uitvoer is ongewijzigd. Een klassenaam uit de kolom StorageClass is niet veranderd: met `cls.csv` (klasse `total x`) had het tekstrapport nog één `total`-regel. Niet gemeten: andere labels (`source:`, `region:`, `lifecycle`) als begin van een sleutel.

## Testresultaat per doel

Debug, stable: `cargo test --locked`, 217 geslaagd, 0 mislukt. De debug-run op Rust 1.85 (`cargo +1.85 test --locked`) gaf op dezelfde commit ook 217 geslaagd, 0 mislukt.

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

Het uitgepakte pakket: `cargo package --offline --allow-dirty` gaf 53 paden; het `.crate` is in een lege map uitgepakt en `cargo test --offline` daar gaf 210 geslaagd, 0 mislukt.

## Niet uitgevoerd

- De release-workflow en een release-archief. De CI is gedraaid: run 45582606112 op `7d7c78befbcb71a4132cc6e174d0ba452037c0a0` is geslaagd. `cargo about generate` schreef ook `THIRD-PARTY-LICENSES.html` in de repository.
- Een draaiende release: er is geen tag, geen release-archief en geen `cargo publish`.
- Een build en een testrun op musl. De Windows- en macOS-testjobs slaagden op diezelfde run.
- Een fuzz-campagne en een mutatietool.
- `cargo audit` met een vers opgehaalde advisory-database (de lokale database is gelezen).
- Een bestand op de limiet van 1073741824 bytes met alleen verschillende prefixen of klassenamen.
- `git push`, `git tag` en `cargo publish`.

## Bekende grenzen

- Alleen inventory-CSV en gzip-CSV worden gelezen. Parquet en ORC worden geweigerd.
- "Geen netwerkaanroep" is gecontroleerd via de afhankelijkhedenlijst (`cargo tree -e normal`) en een zoekactie in `src/`; geen test bekijkt systeemaanroepen.
- De detectie van gesloten standaarduitvoer werkt alleen op Linux. Een mislukkende schrijfactie (`/dev/full`, een gesloten pipe) geeft op elk systeem exit 2.
- Bij een heel groot totaal kan een verschil in het laatste cijfer blijven tussen de prefixsom en het totaal (README C19).
- De benchmark prijst een miljoen rijen in één test; het is geen geheugen- of tijdgarantie voor een bestand van 1073741824 bytes.
- Geheugen volgt het aantal verschillende prefixen en klassenamen; de gemeten waarden staan in de README onder "Beperkingen".
- De volledige lijst met grenzen en open punten staat in `QUALITY.md`.

## Bewering → test

Elke genummerde bewering van `README.md` en de andere uitspraken in de tekst, met de test die ze bewijst. De testnamen zijn dezelfde als in `QUALITY.md`; `quality_claim_table_names_only_existing_tests` controleert dat elke naam bestaat en `quality_pages_agree_and_record_date_commit_and_what_was_not_run` dat beide pagina's dezelfde namen noemen. "Leemte" betekent dat geen test het bewijst.

| # | Bewering | Test | Status |
| --- | --- | --- | --- |
| C1 | Eén rij is één object; dezelfde sleutel twee keer telt twee keer | `claim_c1_one_row_is_one_object` | getest |
| C2 | 1 GiB is 1.073.741.824 bytes; 1 TB is 1.024 GB | `claim_c2_gibibyte_and_terabyte` | getest |
| C3 | Standard-tarief eerste 50 TB, bron en datum, us-east-1 | `claim_c3_standard_rate_source_date_region` | getest |
| C4 | Standard-IA: ondergrens 128 KiB, horizon 30 dagen | `claim_c4_standard_ia_floor_and_thirty_days` | getest |
| C5 | One Zone-IA: ondergrens 128 KiB | `claim_c5_onezone_ia_floor` | getest |
| C6 | Glacier Instant Retrieval: ondergrens, 90 dagen, tarief | `claim_c6_glacier_ir_floor_and_ninety_days` | getest |
| C7 | Glacier Flexible Retrieval: metadata, 90 dagen, tarief | `claim_c7_glacier_metadata_and_ninety_day_horizon`, `zero_byte_glacier_still_bills_metadata` | getest |
| C8 | Deep Archive: metadata, 180 dagen, tarief | `claim_c8_deep_archive_metadata_and_one_hundred_eighty_days` | getest |
| C9 | Intelligent-Tiering onder 128 KiB en monitoringkosten | `claim_c9_intelligent_tiering_small_objects_and_monitoring` | getest |
| C10 | Express One Zone: tarief, geen ondergrens | `claim_c10_express_has_no_floor` | getest |
| C11 | Prijzen van overgangsverzoeken | `claim_c11_lifecycle_request_prices` | getest |
| C12 | Elke besparingsregel eindigt met Not executed | `claim_c12_savings_line_is_not_executed` | getest |
| C13 | Opslag geprijsd zonder `--as-of`; besparingen weggelaten | `claim_c13_savings_omitted_without_as_of` | getest |
| C14 | Ontbrekende, ongeldige of toekomstige datum blijft in de opslag, niet in de besparing, niet getoond | `claim_c14_future_and_bad_dates_are_skipped_without_the_value`, `absent_date_column_warns_and_prices_storage` | getest |
| C15 | Prefixdiepte en het prefixaandeel van Standard | `claim_c15_prefix_depth_and_standard_share`, `prefix_depth_groups_the_sample_key`, `prefix_depth_keeps_a_trailing_slash_only_when_more_segments_exist` | getest |
| C16 | Onbekende klasse is ongeprijsd maar geteld | `claim_c16_unknown_class_is_unpriced`, `unpriced_class_renders_the_word` | getest |
| C17 | Gzip wordt uitgepakt; Parquet en ORC geweigerd; geen AWS-SDK | `claim_c17_parquet_orc_gzip_and_no_sdk`, `parquet_and_orc_names_exit_2`, `gzip_reads_every_member` | getest |
| C18 | `--prices` vervangt de prijsopgave; onbekend veld genegeerd; dubbele sleutel is een fout | `claim_c18_prices_override_changes_the_bill`, `claim_c18_unknown_field_ignored_and_duplicate_hidden` | getest |
| C19 | Dollartotaal is de klassensom; JSON-bedragen als strings; prefixbedragen en verschil | `claim_c19_total_is_the_sum_and_json_uses_strings`, `prefix_amounts_sum_to_the_total`, `reconcile_fixes_rounding_but_rejects_a_large_gap`, `large_total_leaves_the_prefix_drift_on_the_largest_prefix` | getest |
| C20 | Ongeldige Size is `row N: invalid Size`, N telt bestandsregels, niets getoond | `claim_c20_errors_hide_file_contents`, `row_number_is_the_file_line_when_blank_lines_precede`, `bad_size_does_not_echo_the_row` | getest |
| C21 | Glacier-metadata gestapeld op de Standard-trappen | `claim_c21_metadata_crosses_the_standard_tier` | getest |
| C22 | Klein Standard-object kan een negatieve besparing tonen | `claim_c22_small_standard_object_saves_a_negative_amount` | getest |
| C23 | Geen kosten voor ophalen of GET; 1 GiB Standard-IA is 0,0125 | `claim_c23_retrieval_is_not_added` | getest |
| C24 | Bestanden delen één Standard-trappenschema | `claim_c24_files_share_one_standard_tier_schedule`, `two_files_share_one_tier_schedule` | getest |
| Q1 | Voorbeeldbestand en elk console-blok van de snelstart komen overeen met de binary | `quickstart_heredoc_matches_the_sample_file`, `console_examples_match_the_binary` | getest |
| Q2 | `--json` houdt standaardfout leeg voor het voorbeeld | `example_inventory_exits_0_and_json_keeps_stderr_empty` | getest |
| Q3 | Waarschuwingen gaan naar standaardfout als die er zijn | `warnings_go_to_stderr_with_json_and_savings_name_four_targets`, `absent_date_column_warns_and_prices_storage` | getest |
| W1 | Kopregel met Bucket, Key, Size, StorageClass is verplicht; zonder wordt het bestand geweigerd met exit 2 en de verwachte kolommen | `headerless_aws_inventory_line_is_refused_with_the_expected_columns`, `missing_column_and_short_row_and_duplicate_column` | getest |
| W2 | Sleutels worden gebruikt zoals ze er staan, niet URL-gedecodeerd | `keys_are_used_as_written_and_not_url_decoded` | getest |
| W3 | Een bestand boven 1073741824 bytes wordt geweigerd, op schijf en na gzip | `file_larger_than_1073741824_bytes`, `files_over_the_cap_are_refused_by_name_and_size`, `plain_and_gzip_over_the_cap_say_how_large` | getest |
| W4 | Een symlink wordt gevolgd; een map is een fout | `directory_is_rejected_and_symlink_is_followed`, `symlink_is_followed_and_a_directory_is_rejected` | getest |
| W5 | `--prices` en elke inventory accepteren alleen een gewoon bestand | `prices_fifo_is_refused_without_reading`, `inventory_fifo_is_refused_without_reading`, `missing_prices_file_exits_2` | getest (Unix) |
| W6 | Intelligent-Tiering-laag `ARCHIVE` en `DEEP_ARCHIVE` worden als de `_ACCESS`-namen gelezen | `it_inventory_tier_names_without_access_suffix_match_the_access_names` | getest |
| W7 | Een decodeerfout noemt het bestand | `bad_gzip_in_second_file_names_that_file`, `csv_quote_errors_and_bad_gzip_name_the_file`, `a_file_error_names_the_file_with_one_file_too` | getest |
| W8 | Gesloten of falende standaarduitvoer geeft exit 2 met `could not write output`; `/dev/null` houdt exit 0 | `closed_stdout_is_exit_2`, `dev_full_is_exit_2`, `dev_null_keeps_real_exit`, `stdout_set_to_dev_null_by_the_parent_is_not_an_error` | getest (alleen Linux) |
| W9 | Prijzen zijn decimalen van hoogstens 28 decimalen; alleen spatie, tab, CR, LF worden weggehaald | `parse_money_trims_only_space_tab_cr_lf`, `money_rejects_exponent_sign_and_empty` | getest |
| W10 | Het gratis tegoed van het account wordt niet toegepast; niets wordt kwijtgescholden | `console_examples_match_the_binary` shows 1 GiB billed in full at 0.023 | deels getest: geen test noemt het gratis tegoed |
| W11 | Installeren vanaf een lokaal pad met Rust 1.85 | `install_uses_a_local_path` checks the text only | leemte: `cargo install` en Rust 1.85 niet gedraaid |
| W12 | README-koppen en licenties komen overeen met het pakket | `readme_sections_follow_the_documentation_standard_in_order`, `licenses_and_changelog_match_the_package` | getest |
| W13 | Besparingen gaan ervan uit dat het object de hele horizon blijft; geen voorspelling | geen | leemte: een afbakening, geen gedrag om te testen |
| W14 | One Zone-IA en Intelligent-Tiering zijn geen besparingsdoelen | `warnings_go_to_stderr_with_json_and_savings_name_four_targets` lists the four targets | getest |
| M1 | De man-pagina bevat de `--help`-tekst en de OPTIONS-sectie beschrijft precies de lange opties | `help_long_options_match_the_man_page_and_unknown_option_is_not_a_panic` | getest |
| W15 | `--prefix-depth` is 1 tot 8; elke andere waarde geeft exit 2 | `prefix_depth_range_is_named_in_help_readme_and_man_and_enforced` | getest |
| W16 | Een record heeft hoogstens 64 kolommen, tussen aanhalingstekens of niet | `sixty_four_columns_pass_and_sixty_five_are_refused`, `a_quoted_record_has_the_same_column_limit_as_a_plain_one` | getest |
| W17 | Een aanhalingsteken dat midden in een veld zonder aanhalingstekens opent, is een fout | `quote_inside_an_unquoted_field_is_refused`, `quote_error_names_the_line_and_exits_2` | getest |
| W18 | Een bestand met alleen CR-regeleinden wordt geweigerd met `missing column StorageClass` en de melding noemt de CR-regeleinden | `cr_only_file_is_refused_and_empty_key_shares_the_empty_prefix`, `cr_only_file_says_the_line_ends_are_the_cause` | getest |
| W19 | Een lege sleutel en een sleutel `(empty)` delen één prefixregel | `cr_only_file_is_refused_and_empty_key_shares_the_empty_prefix` | getest |
| W20 | Eén regel van 200.000 geciteerde velden wordt in minder dan een seconde geweigerd; 5.000.000 in 0,05 s | `one_line_with_many_quoted_fields_is_read_in_linear_time` (unit level, 2 s limit) | getest (tijd); leemte: geen test meet het geheugen van de regel met 5.000.000 velden, met de hand gemeten |
| W21 | Geheugen volgt het aantal verschillende prefixen; de gemeten waarden uit de README | `many_distinct_prefixes_are_priced_in_a_moment` (time only, 40,000 prefixes in under 4 s in debug and not more than about 3 times the time of 20,000); memory measured by hand on 2026-11-20 | getest (tijd); leemte: geen test meet geheugen |
| W22 | Systeemeisen: Rust 1.85 of nieuwer, glibc 2.34 of nieuwer op Linux | the run on Rust 1.85.1 (`cargo +1.85 test`); `scripts/check-glibc.sh` ran by hand on the release binary | leemte: geen test voor de glibc-waarde; de GitHub-job Highest glibc symbol slaagde op run 45582606112 |
| W23 | Een backslash in een bestandsnaam wordt in een fout één keer verdubbeld | `backslash_in_a_path_is_escaped_exactly_once`, `shown_path_leaves_escaping_to_the_error` | getest |
| W24 | Geen lokaal pad, interne naam of werkbestandsnaam in documenten, man-pagina, bronnen, tests of CI-bestanden | `documents_code_tests_and_ci_hold_no_local_paths_or_work_file_names`, `the_scan_reaches_the_ci_file_and_the_sources`, `the_check_finds_each_pattern` | getest |
| W25 | `--as-of` is een dag: een object op of vóór die dag is geschikt; de tijd van de dag wordt niet gelezen | `as_of_day_is_the_whole_day_and_the_next_day_is_not_eligible`, `as_of_help_names_the_day_rule_in_help_readme_and_man` | getest |
| W26 | Met `--json` gaan waarschuwingen naar standaardfout; in het tekstrapport zijn het regels op standaarduitvoer | `warnings_go_to_stderr_with_json_and_savings_name_four_targets`, `text_report_keeps_warnings_on_stdout_and_stderr_empty` | getest |
| W27 | ASCII-witruimte rond Size wordt genegeerd; een teken, `_`, `0x`, niet-ASCII-cijfer, niet-afbrekende spatie of leeg veld is `invalid Size` | `size_with_ascii_space_is_read_and_a_sign_or_other_digits_are_refused` | getest |
| W28 | Gebruiksfouten tonen stuurtekens ge-escaped op één regel | `usage_error_escapes_raw_control_characters`, `usage_errors_exit_2` | getest |
| W29 | Een Parquet- of ORC-inventory (op naam of magic, ook in gzip) wordt geweigerd met exit 2 en een tekst dat alleen CSV en gzip-CSV worden gelezen | `parquet_and_orc_names_exit_2`, `claim_c17_parquet_orc_gzip_and_no_sdk`, `extension_check_ignores_case_and_covers_every_listed_suffix` | getest |
| W30 | Geen minimale Windows- of macOS-versie vastgesteld; een musl-build is alleen gelint | geen; als limiet vermeld | leemte: een vermelding van wat niet gedaan is |
| W31 | Een `Size` mag willekeurig veel voorloopnullen hebben en is hoogstens 18446744073709551615 | `size_with_leading_zeros_is_read_whatever_its_length`, `parse_size_accepts_the_limit_and_refuses_one_more` | getest |
| W32 | Een heel groot geldig totaal met een hoog tarief wordt geprijsd, niet geweigerd om een tussentijds verlies | `large_valid_totals_with_a_high_rate_are_not_refused_for_an_intermediate_loss`, `wide_share_rounds_once_and_keeps_the_sum`, `share_survives_intermediate_overflow_when_total_fits` | getest |
| W33 | Witruimte-, hoofdletter-, NUL-, ORC-kopregel-, gzip-staart- en prefix-depth-regels uit de README | `readme_input_rules_hold_for_white_space_class_names_nul_and_sniffing` | getest |
| W34 | Getallimieten: 64-bit bytesom (ook met `--as-of`), prijscijfers, `*_min_days`, `min_billable_bytes`, `--as-of`-jaren, 28 significante cijfers | `readme_number_limits_hold_for_sizes_prices_days_and_dates`, `extreme_price_exits_2_without_panic` | getest |
| W35 | `exclude` in `Cargo.toml` noemt precies de twee lokale werkbestanden en het manifest heeft geen interne naam | `the_manifest_holds_no_internal_name_and_excludes_exactly_the_work_files` | getest |
| W36 | Het pakket bevat `man/` en `SECURITY.md` en geen lokaal werkbestand | geen; met de hand uit `cargo package --list` gelezen | leemte: geen test |
| W37 | Een gzip-bestand heeft hoogstens 100000 leden | `gzip_with_very_many_members_is_refused_in_a_moment` | getest |
| W38 | Tekst na een sluitend aanhalingsteken is een fout; geciteerde velden lezen nog | `text_after_a_closing_quote_is_refused_and_a_quoted_field_still_reads` | getest |
| W39 | Een CR-only-bestand wordt ook geweigerd als de laatste kolom optioneel is | `cr_only_file_is_refused_also_when_the_last_column_is_optional` | getest |
| W40 | Een `--prices`-bestand is hoogstens 1048576 bytes | `a_price_file_over_one_mebibyte_is_refused_and_one_at_the_limit_loads` | getest |
| W41 | Geheugen en tijd van verschillende klassenamen, van een gewoon bestand op de limiet en van een bestand van 5.000.000 geciteerde velden | `many_distinct_prefixes_are_priced_in_a_moment` (time only, 40,000 prefixes in under 4 s in debug and not more than about 3 times the time of 20,000); memory measured by hand on 2026-11-20 | getest (tijd); leemte: geen test meet geheugen |
| W42 | De maandkosten heten `estimated_usd_month`, het tekstrapport heeft de schattingsregel en de JSON heeft `"estimate": true` | `monthly_cost_is_labelled_as_an_estimate` | getest |
| W43 | Prijsdata ouder dan 180 dagen geeft een waarschuwing met de datum, exit 0; precies 180 dagen geeft er geen | `old_price_data_warns_after_180_days_and_still_exits_0` | getest |
| W44 | Een inventory in de gedocumenteerde vorm (fileSchema-kopregel, geciteerde velden, URL-gecodeerde sleutels, ontbrekende velden, lege lijst, meer bestanden) wordt gelezen | `full_schema_with_quoted_fields_and_url_encoded_keys`, `optional_fields_missing_and_columns_in_another_order`, `empty_lists_give_a_zero_report`, `several_files_and_a_gzip_file_add_up_exactly`, `keys_with_special_characters_stay_on_one_line_and_are_counted`, `intelligent_tiering_access_tier_column_is_read` | getest tegen de vorm uit <https://docs.aws.amazon.com/AmazonS3/latest/userguide/storage-inventory.html> (het S3 Inventory-hoofdstuk van de Amazon S3 User Guide; geen netwerk); leemte: de laagnamen en delete-marker-rijen uit de documentatie zijn niet tegen de live pagina gecontroleerd |
| W45 | `--quiet` laat rapport en fouten staan en laat de waarschuwingskopieën weg; `--verbose` voegt voortgang toe; samen geeft exit 2 | `quiet_keeps_the_report_and_drops_the_stderr_warnings`, `quiet_still_shows_errors_with_exit_2`, `verbose_adds_progress_on_stderr_only`, `verbose_escapes_a_control_character_in_a_path`, `quiet_with_verbose_is_a_usage_error` | getest |
| W46 | `--md` drukt dezelfde bedragen af als tekst en JSON, met één schattingsregel en ge-escapete scheidingstekens en regeleinden; met `--json` exit 2 | `markdown_amounts_equal_text_and_json`, `pipes_and_line_breaks_in_cells_are_escaped`, `markdown_without_as_of_says_the_lifecycle_is_omitted`, `md_and_json_cannot_be_combined_and_help_lists_md`, `markdown_has_the_same_report_with_quiet_and_nothing_on_stderr`, `readme_markdown_example_is_what_the_binary_prints` | getest |
| W47 | Een echte SIGINT tijdens het lezen geeft exit 130 met één regel op standaardfout en niets op standaarduitvoer; tijdens het schrijven wacht hij | `sigint_while_reading_exits_130_with_one_line_and_no_output` | getest (Linux, leesfase); leemte: de vastgehouden schrijffase heeft geen test, een blokkerende standaarduitvoer is niet op een vast punt te laten blokkeren |
| W48 | `--output` schrijft het hele rapport per hernoeming, vervangt een bestand of symlink, weigert een map of speciaal bestand, faalt met exit 2; SIGINT laat het oude bestand staan | `the_report_goes_to_the_file_and_not_to_standard_output`, `an_existing_file_is_replaced_whole`, `a_symlink_is_replaced_and_never_followed`, `write_failures_are_exit_2_with_a_message_and_nothing_is_created`, `a_special_file_is_refused_and_a_read_only_directory_is_exit_2`, `a_failed_run_leaves_an_existing_output_file_as_it_was`, `json_warnings_stay_on_stderr_and_a_control_character_in_the_path_is_escaped`, `sigint_leaves_an_existing_output_file_as_it_was_and_creates_none` | getest; leemte: een volle schijf is niet getest |
| W49 | Elk JSON-document heeft `"schema_version": 1` als eerste sleutel; een fout met `--json` drukt geen JSON af, exit 2 | `every_json_path_has_schema_version_first`, `schema_version_is_documented_in_the_readme_and_quality` | getest |
