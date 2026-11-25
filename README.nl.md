# s3-inventory-bill

Schat de S3-opslagkosten uit een Inventory-CSV. Leest alleen bestanden, geen netwerk.

De tests en de gemeten getallen staan in `QUALITY.md`.

Wat hij leest en doet: hij leest de inventory-CSV-bestanden (of gzip-CSV) die op de opdrachtregel staan en, met `--prices`, één prijsbestand. Hij doet geen netwerkaanroep, heeft geen AWS-account, token of betaalde dienst nodig en schrijft alleen het rapport naar standaarduitvoer, of naar het bestand van `--output`. Hij start geen ander proces.

## Waarom

Een S3 Inventory-export noemt van elk object de grootte en de opslagklasse. s3-inventory-bill rekent die CSV op je eigen machine door, per klasse en per prefix, en toont wat een lifecycle-overgang van geschikte Standard-objecten naar Standard-IA of Glacier zou veranderen. Hij roept AWS niet aan en heeft geen account nodig.

De eerste rij van elk bestand moet een kopregel zijn met de kolommen `Bucket`, `Key`, `Size` en `StorageClass`, in willekeurige volgorde; `LastModifiedDate` en `IntelligentTieringAccessTier` zijn optioneel. Een ruwe S3 Inventory-CSV heeft geen kopregel, dus voeg er een toe in de kolomvolgorde van `manifest.json` van de inventory (`fileSchema`). Een bestand zonder kopregel wordt geweigerd met exitcode 2 en een melding die de verwachte kolommen noemt. Sleutels worden gebruikt zoals ze er staan: de tool decodeert ze niet als URL.

## Installatie

Versie 1.0.0. Deze crate is niet gepubliceerd en versie 1.0.0 heeft geen git-tag, dus er is nog geen installatie via crates.io of een binair archief. Bouw vanuit deze checkout met Rust 1.85 of nieuwer:

```sh
cargo install --locked --path .
s3-inventory-bill --help
```

De licenties van de derde-partij-crates in de binary staan in [THIRD-PARTY-LICENSES.html](THIRD-PARTY-LICENSES.html). Een release-archief bevat datzelfde bestand. Zolang er geen release is, werkt `cargo install s3-inventory-bill` vanaf crates.io niet; installeer vanuit deze checkout.

Ondersteunde systemen: Linux is het geteste systeem (glibc 2.34 of nieuwer voor een glibc-build). De Windows- en macOS-builds zijn hier alleen typegecontroleerd en een musl-build is alleen gelint (zie "Beperkingen" en [QUALITY.nl.md](QUALITY.nl.md)).

## Snelstart

`examples/inventory.csv` is één Standard-object van 1 GiB onder `logs/app/`. De tool leest dat bestand alleen. Wil je het buiten een clone proberen, schrijf dan dezelfde bytes naar een tijdelijke map en lees die:

```sh
dir=$(mktemp -d)
cat > "$dir/inventory.csv" <<'CSV'
Bucket,Key,Size,LastModifiedDate,StorageClass
example-bucket,logs/app/a.bin,1073741824,2024-01-15T00:00:00.000Z,STANDARD
CSV
s3-inventory-bill --as-of 2026-11-09 "$dir/inventory.csv"
```

Een test draait elk console-blok hieronder, vanuit de root van een clone, en vergelijkt het met deze binary. `--as-of` is een dag: een object dat op of vóór die dag voor het laatst is gewijzigd, is geschikt. Het voorbeeldobject is op 2024-01-15 geschreven, dat is op of vóór 2026-11-09, dus het is geschikt. De tarieven zijn US East (N. Virginia) van <https://aws.amazon.com/s3/pricing/>, gelezen op 2026-11-09.

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

1 GiB Standard kost 0,023 USD per maand. Dat object 30 dagen naar Standard-IA verplaatsen bespaart 0,01049 USD na het overgangsverzoek van 0,00001. Glacier Flexible Retrieval en Glacier Deep Archive rekenen daarbovenop 8 KiB tegen Standard-tarief en 32 KiB tegen het koude tarief, dus `monthly_after_usd` is het object plus die metadata, niet het object alleen. Elke doelregel eindigt met `Not executed`. Zonder `--as-of` is het lifecycle-blok `omitted: pass --as-of YYYY-MM-DD` en wordt alleen opslag geprijsd.

## Aan de slag in 5 minuten

Korte route in het Nederlands; deze README heeft dezelfde koppen als de Engelse (`README.md`).

1. Bouw de tool met Rust 1.85 of nieuwer: `cargo install --locked --path .`.
2. Zet in je S3 Inventory-export een kopregel boven de data, in de kolomvolgorde van `manifest.json` (`fileSchema`): `Bucket`, `Key`, `Size`, `StorageClass`, en liefst `LastModifiedDate`.
3. Draai `s3-inventory-bill --as-of 2026-11-09 inventory.csv` (vervang de datum door vandaag). Zonder `--as-of` krijg je alleen de opslagkosten, geen besparingen.
4. Lees het rapport: kosten per opslagklasse en per prefix, daarna per doelklasse wat een lifecycle-overgang zou schelen. Elk bedrag is een schatting uit lijstprijzen, geen AWS-factuur.
5. Gebruik `--json` voor een script, `--md` voor een pull request of wiki, en `--output bestand` om het rapport in een bestand te zetten.

## Opties

Alle opties met hun standaardwaarden, zoals `s3-inventory-bill --help` ze toont (de helptekst is Engels):

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

Invoer: een of meer bestanden, elk een CSV met kopregel, of een gzip-bestand van zo'n CSV. De eerste rij noemt de kolommen (zie "Waarom"). Het rapport is standaard tekst; `--json` en `--md` kiezen een andere vorm, en die twee zijn niet te combineren (een gebruiksfout, exit 2). `--prefix-depth` is standaard 1.

Elk JSON-document begint met `"schema_version": 1`, de versie van de opbouw; dat getal verandert alleen als een veld een andere naam krijgt, verdwijnt of een andere betekenis krijgt. Een fout drukt nooit JSON af: een mislukte run met `--json` heeft lege standaarduitvoer en de melding op standaardfout (exit 2), dus elk JSON-document dat wordt afgedrukt, heeft dat veld.

Elk bedrag is een schatting uit lijstprijzen: het rapport zegt dat in één regel en de JSON heeft `"estimate": true` op het hoogste niveau. Gratis tegoed, kredieten en kortingen zijn niet meegeteld, dus het bedrag kan afwijken van je AWS-factuur. De maandkosten heten `estimated_usd_month` in de tekst en in de JSON.

Als de datum van het prijsbestand (de ingebouwde lijst of `--prices`) meer dan 180 dagen vóór `--as-of` ligt, of vóór vandaag als er geen `--as-of` is, bevat het rapport een waarschuwing die die datum noemt. De exitcode blijft 0. `SOURCE_DATE_EPOCH` (seconden sinds 1970-01-01 UTC) vervangt, als hij gezet is, de klok, zodat een run herhaalbaar is.

`--json` drukt dezelfde totalen af. Bedragen en byte-totalen zijn strings. Met `--json` gaan waarschuwingen, als die er zijn, naar standaardfout; in het tekstrapport zijn het regels aan het eind van het rapport op standaarduitvoer. Dit voorbeeld heeft er geen.

`--md` drukt dezelfde bedragen af als Markdown-tabellen, met de schattingsregel, voor een pull request of wiki. Een `|` of regeleinde in een klasse- of prefixnaam wordt ge-escaped, zodat het geen cel kan afsluiten. Het is niet te combineren met `--json`; dat is een gebruiksfout met exit 2.

`--quiet` drukt alleen het rapport en fouten af: met `--json` laat het de waarschuwingskopieën op standaardfout weg, en het tekstrapport blijft gelijk. `--verbose` voegt voortgangsregels toe op standaardfout (`s3-inventory-bill: reading FILE`, `pricing N files`, `done`) en laat standaarduitvoer ongemoeid. Beide opgeven is een gebruiksfout met exit 2.

`--output BESTAND` schrijft het rapport naar een bestand in plaats van naar standaarduitvoer (tekst, `--json` en `--md` evengoed) en drukt niets af op standaarduitvoer. Het rapport wordt naar een tijdelijk bestand in dezelfde map geschreven en over BESTAND heen hernoemd, dus BESTAND is het oude rapport of het hele nieuwe. Een bestaand bestand wordt vervangen. Een symbolische koppeling op BESTAND wordt vervangen door het nieuwe bestand en niet gevolgd. Een map, pipe of device wordt geweigerd. Een ontbrekende map wordt niet aangemaakt. Elke fout is exit 2 met `s3-inventory-bill: could not write report to FILE: REASON` en zonder panic. Met `--json` gaan de waarschuwingen nog steeds naar standaardfout.

Ctrl-C (SIGINT) beëindigt de run met exit 130 en de regel `s3-inventory-bill: interrupted by SIGINT` op standaardfout, en er wordt niets op standaarduitvoer afgedrukt. Het rapport wordt aan het eind in één stap geschreven en een SIGINT tijdens die stap wacht tot de stap klaar is. Met `--output` laat een SIGINT vóór de hernoeming een bestaand BESTAND zoals het was en maakt geen bestand aan. Op Windows geldt de eigen Ctrl-C-afhandeling van het platform en is de exitcode niet 130.

## Voorbeeld

`--md` op het voorbeeld drukt af (een test vergelijkt dit blok met de binary):

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

De JSON voor het voorbeeld:

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

## Hoe het telt

De ingebouwde tarieven zijn de US East (N. Virginia)-bedragen op <https://aws.amazon.com/s3/pricing/> per 2026-11-09. De pagina vult zijn tabellen uit de openbare prijslijst die op 2026-11-09T15:43:10Z is gepubliceerd. De opslag van Glacier Deep Archive is het paginatarief 0,00099 USD per GB-maand; die lijst is op 2026-09-11T12:45:07Z gepubliceerd. 1 GB is 1.073.741.824 bytes. 1 TB is 1.024 GB. Standard gebruikt drie trappen over het totaal: de eerste 50 TB tegen 0,023, de volgende 450 TB tegen 0,022 en de rest tegen 0,021. `--prices` vervangt de hele prijsopgave. Geef een ander bestand met dezelfde velden mee als de regio niet us-east-1 is.

C1. Eén inventory-rij is één object. Dezelfde sleutel in twee bestanden telt twee keer.
C2. 1 GiB is 1.073.741.824 bytes. 1 TB is 1.024 GB, dat is 1.099.511.627.776 bytes.
C3. Standard-opslag in de eerste 50 TB kost 0,023 USD per GB-maand. De bron is https://aws.amazon.com/s3/pricing/ per 2026-11-09, regio us-east-1.
C4. Standard-IA rekent minimaal 128 KiB per object. De besparingshorizon is 30 dagen.
C5. One Zone-IA rekent minimaal 128 KiB per object. Het opslagtarief is 0,01 USD per GB-maand.
C6. Glacier Instant Retrieval rekent minimaal 128 KiB per object. De besparingshorizon is 90 dagen en het tarief is 0,004 USD per GB-maand.
C7. Glacier Flexible Retrieval telt voor elk object 8 KiB tegen Standard-tarief en 32 KiB tegen het Glacier-tarief erbij, ook voor een object van 0 bytes. De horizon is 90 dagen en het tarief is 0,0036 USD per GB-maand.
C8. Glacier Deep Archive gebruikt dezelfde metadata-verdeling van 8 KiB en 32 KiB, tegen het Deep Archive-tarief van 0,00099 USD per GB-maand. De horizon is 180 dagen.
C9. Een Intelligent-Tiering-object onder 128 KiB blijft op Frequent Access en betaalt geen monitoringkosten. Een object van 128 KiB of meer wordt gemonitord tegen 0,0000025 USD, dat is 0,0025 USD per 1.000 objecten.
C10. Express One Zone kost 0,11 USD per GB-maand. Er is geen ondergrens van 128 KiB.
C11. Een lifecycle-overgang wordt per verzoek geprijsd: Standard-IA, One Zone-IA en Intelligent-Tiering 0,00001; Glacier Instant Retrieval 0,00002; Glacier Flexible Retrieval 0,00003; Glacier Deep Archive 0,00005. Duizend monitoringkosten zijn 0,0025 USD.
C12. Elke besparingsregel eindigt met de woorden Not executed. Zonder `--as-of` wordt het lifecycle-blok weggelaten.
C13. Opslag wordt geprijsd of `--as-of` nu gezet is of niet. Besparingen vervallen als hij ontbreekt. De klok wordt niet als standaarddatum gebruikt.
C14. Een ontbrekende, lege, ongeldige of toekomstige LastModifiedDate blijft in de opslagkosten en wordt buiten de besparingen gelaten. De waarschuwing noemt de datum of de sleutel niet.
C15. Prefixdiepte 1 van `logs/2024/a.bin` is `logs/`. Diepte 2 is `logs/2024/`. Een sleutel met minder segmenten blijft de sleutel, dus `pics/c` op diepte 2 is `pics/c`. De Standard-dollars van een prefix zijn het aandeel van die prefix in het getrapte Standard-totaal.
C16. REDUCED_REDUNDANCY en elke andere onbekende klasse zijn ongeprijsd. Ze tellen nog wel mee als objecten en ruwe bytes. Ze zijn uitgesloten van het dollartotaal.
C17. Gzip-CSV wordt uitgepakt en daarna geprijsd. Parquet en ORC, ook een gzip die naar die magics uitpakt, worden geweigerd. De crate linkt geen AWS-SDK.
C18. `--prices` vervangt de ingebouwde prijsopgave. Een onbekend JSON-veld wordt genegeerd. Een dubbele sleutel op elke diepte is een fout, en de melding is `duplicate key`.
C19. Het dollartotaal is de exacte som van de delen van alle geprijsde klassen, één keer afgerond, niet de som van de afgeronde klassebedragen. Een klasse met meerdere delen (Intelligent-Tiering, Glacier, Deep Archive) is ook de exacte som van haar delen, één keer afgerond. Bedragen en byte-totalen in JSON zijn strings. Elk bedrag wordt één keer afgerond, half weg van nul (zie "Getallen" hieronder). Elke prefix is zijn aandeel van het getrapte Standard-totaal, op zichzelf gedeeld, dus de prefixbedragen kunnen in de laatste decimaal van het totaal afwijken. Het hele verschil wordt bij de grootste prefix opgeteld. Buiten de tool opgeteld kunnen de prefixbedragen in de laatste cijfers nog steeds van het totaal afwijken. Bij een heel groot totaal is de correctie te klein voor de decimale precisie en blijft een verschil in het laatste cijfer op die grootste prefix staan.
C20. Een Size die leeg is, een teken heeft of geen geheel getal is, is `FILE: row N: invalid Size`, waarbij `FILE` het pad is zoals opgegeven, ook als er maar één bestand is. `N` is het regelnummer in het bestand, lege regels meegeteld. De melding bevat de waarde of de sleutel niet.
C21. Glacier-metadata wordt op het Standard-trappenschema gestapeld na de Standard-objectbytes, en die dollars horen bij de koude klasse. Eén Standard-object van 50 TB min 4.096 bytes plus één Glacier-object van 0 bytes prijst 4.096 metadatabytes tegen 0,023 en 4.096 tegen 0,022.
C22. Een Standard-object van 1.000 bytes dat naar Standard-IA gaat, kan een negatieve besparing tonen, omdat de ondergrens van 128 KiB meer kost dan het kleine object nu.
C23. Kosten voor ophalen en GET worden niet opgeteld. 1 GiB Standard-IA is 0,0125 USD per maand.
C24. Elk bestand wordt eerst opgeteld en één keer geprijsd, dus de Standard-trappen gelden voor de som. 50 TB min 1 GiB in één bestand plus 2 GiB in een ander is 1177,622 USD.

Geschikte Standard-objecten worden op hun eigen trappenschema geprijsd. Het early-delete-restant voor een Standard-bron is 0.00, en dat veld wordt nog steeds afgedrukt. One Zone-IA en Intelligent-Tiering hebben overgangstarieven in de prijsopgave. Het zijn geen besparingsdoelen.

## Afsluitcodes

| Code | Betekenis |
| ---: | --- |
| 0 | Het rapport is geschreven. Waarschuwingen (bijvoorbeeld verouderde prijsdata) veranderen dit niet. `--help` en `--version` geven ook 0. |
| 2 | Een invoer- of gebruiksfout: een onbekende of strijdige optie, een bestand dat niet te lezen is of geweigerd wordt, een slecht prijsbestand, een bedrag of byte-totaal dat niet past, of een standaarduitvoer (of `--output`-bestand) waarnaar niet te schrijven is. De melding staat op standaardfout en er wordt niets op standaarduitvoer afgedrukt. Een fout drukt nooit JSON af. |
| 130 | Onderbroken met Ctrl-C (SIGINT). Er wordt niets op standaarduitvoer afgedrukt. |

De tool geeft geen panic bij slechte invoer; een fout is exit 2 met een melding van één regel die geen cel of sleutel herhaalt.

## Beperkingen

Reikwijdte:

- De tool leest alleen CSV. Parquet- en ORC-inventories worden geweigerd, evenals een gzip met die magics.
- De lifecycle-regel wordt niet uitgevoerd: de tool voert nooit een overgang uit. Er is geen AWS-SDK en geen netwerkaanroep.
- De ingebouwde prijsopgave is us-east-1. Het ophalen van objecten zit niet in deze opslagrekening. Het gratis tegoed van het account is niet toegepast: er wordt niets kwijtgescholden. Reduced Redundancy is ongeprijsd, evenals elke klasse buiten de prijsopgave.
- Besparingen gaan ervan uit dat het object de hele horizon blijft staan. Dit is geen voorspelling.

Invoerregels:

- Een bestand boven 1073741824 bytes wordt geweigerd, op schijf en na gzip. Een symlink naar een bestand wordt gevolgd. Een map is een fout, evenals een pipe, een device of elk ander bestand dat geen gewoon bestand is (`not a file`). `--prices` accepteert alleen een gewoon bestand van hoogstens 1048576 bytes (`file is larger than 1048576 bytes`); de ingebouwde prijsopgave is ongeveer 2 KB. Een gzip-bestand heeft hoogstens 100000 leden (`gzip file has more than 100000 members`); elk lid kost tijd, ook als het leeg is.
- De eerste rij is de kopregel (zie "Waarom"). Een record heeft hoogstens 64 kolommen, tussen aanhalingstekens of niet. Een aanhalingsteken dat midden in een veld zonder aanhalingstekens opent, is een fout, evenals elke tekst na het sluitende aanhalingsteken van een geciteerd veld behalve een komma of het regeleinde (`row N: text after a closing quote`), dus `"1"0` wordt niet als 10 gelezen.
- Regeleinden zijn LF of CRLF. Een bestand waarvan de enige regeleinden CR zijn, wordt als één regel gelezen en geweigerd, met `missing column ...` als een verplichte kolom niet wordt gevonden, of met `the first row is not a header line` als dat wel zo is (bijvoorbeeld `LastModifiedDate` is de laatste kolom); beide meldingen zeggen dat het bestand mogelijk alleen CR-regeleinden gebruikt.
- Een `LastModifiedDate` wordt gelezen als `YYYY-MM-DD`, gevolgd door het veldeinde, een `T` of een spatie. Wat daarna komt wordt niet gecontroleerd, dus `2024-01-15T99:99:99.000Z` telt als 2024-01-15. `--as-of` vergelijkt alleen dagen; een object dat op de `--as-of`-dag is gewijzigd, is geschikt.
- `--prefix-depth` is 1 tot 8; `08` en `+3` worden gelezen als 8 en 3. Elke andere waarde geeft exit 2.
- Een lege sleutel en een sleutel geschreven als `(empty)` vallen onder dezelfde prefixregel.
- Een `Size` bestaat alleen uit ASCII-cijfers, met willekeurig veel voorloopnullen, en is hoogstens 18446744073709551615. Witruimte rond het getal, en rond `Bucket` en `StorageClass`, wordt genegeerd (spatie, tab, CR, LF en form feed; een verticale tab niet), dus ` 5` en `5 ` zijn 5 bytes; een teken, een underscore, `0x`, een niet-ASCII-cijfer, een niet-afbrekende spatie of een leeg veld is `invalid Size`. Een sleutel wordt gebruikt zoals hij er staat, spaties inbegrepen.
- Een opslagklasse wordt vergeleken zonder onderscheid tussen ASCII-hoofdletters en kleine letters: `standard` is `STANDARD`. Een onbekende klasse wordt in hoofdletters getoond.
- Een NUL-byte in de eerste 512 bytes betekent `not a CSV inventory`; een NUL-byte later in het bestand is deel van de sleutel en wordt ge-escaped getoond (`\0`). Een bestand dat begint met de letters `ORC` of `PAR1` wordt als dat formaat geweigerd, ook als het CSV is (een kopregel die met `ORC_ID` begint, wordt geweigerd). Bytes na het laatste gzip-lid, ook nullen, betekenen `not a gzip inventory`.
- Dezelfde sleutel telt twee keer als twee rijen hem noemen. Een Intelligent-Tiering-laag met de naam `ARCHIVE` of `DEEP_ARCHIVE` wordt gelezen als `ARCHIVE_ACCESS` of `DEEP_ARCHIVE_ACCESS`.
- Een fout over een bestand begint met het pad van dat bestand, ook als er maar één bestand is opgegeven.
- Prijzen in een `--prices`-bestand zijn decimale getallen van hoogstens 28 decimalen; alleen spatie, tab, CR en LF eromheen worden weggehaald.

Getallen:

- Bytes zijn 64-bit. De som van alle `Size`-waarden is hoogstens 18446744073709551615, anders eindigt de run met `byte total does not fit`. Met `--as-of` wordt de som ook genomen met elk object minstens op de minimale factureerbare grootte, dus een invoer op die grens die zonder `--as-of` wordt geaccepteerd, kan ermee worden geweigerd (één object van 18446744073709551615 bytes plus één leeg object).
- Geld houdt hoogstens 28 decimalen, en minder als de cijfers vóór de punt de ruimte nodig hebben, dus een groot bedrag heeft hoogstens 28 significante cijfers. Elk bedrag wordt één keer, aan het eind, afgerond, half weg van nul. 3 GiB tegen 3,3333333333333333333333333333 per GiB is precies 9,9999999999999999999999999999; die waarde past niet en de ene afronding toont `10.00`. Een bedrag boven 79228162514264337593543950335 is `amount does not fit`.
- In een `--prices`-bestand zijn de cijfers van een prijs, zonder de punt geschreven, hoogstens 79228162514264337593543950335, dus `9.9999999999999999999999999999` wordt geweigerd als `price file FILE: bad standard_first_50_tb`. De `*_min_days`-velden zijn hele getallen van 1 tot 36500 en `min_billable_bytes` is een geheel JSON-getal (niet `131072.0`, geen string, niet negatief). Die melding noemt het veld, niet de waarde.
- `--as-of` is een echte dag van 0000-01-01 tot 9999-12-31.

Uitvoer:

- Als standaarduitvoer gesloten is of een schrijfactie ernaartoe mislukt, eindigt de tool met exit 2 en `could not write output` op standaardfout. Een gesloten standaarduitvoer wordt alleen op Linux herkend.

Geheugen en tijd:

- Het geheugen volgt het aantal verschillende prefixen en verschillende opslagklassenamen, niet alleen de bestandsgrootte. Een gewoon bestand wordt één keer in het geheugen gehouden en een gzip-bestand ingepakt en uitgepakt. Gemeten op Linux met de release-build, tekstrapport, één run per meting na de laatste codewijziging (rijen over acht opslagklassen, elke rij een eigen prefix, groottes vanaf 1.000.003 bytes): 500.000 prefixen (17,7 MB) 1,35 s en 346 MB resident geheugen, 1.000.000 prefixen 2,67 s en 694 MB, 2.000.000 prefixen (72,5 MB) 5,42 s en 1,39 GB. Tweemaal de invoer kostte 1,98 keer zoveel tijd en 2,00 keer zoveel geheugen, viermaal 4,0 keer en 4,0 keer. Met `--json` kostten dezelfde bestanden 2,9 s, 6,0 s en 11,7 s, en 2.000.000 prefixen vroegen 4,9 GB. Dezelfde vorm met 50.000, 100.000 en 200.000 prefixen kostte 0,13 s, 0,26 s en 0,53 s; de versie vóór deze wijziging deed er 7,4 s, 15,3 s en 33,5 s over. 1.500.000 rijen (45,4 MB) met 10 verschillende prefixen kostten 0,31 s en 46 MB. 1.000.000 rijen (29,8 MB) met één prefix en elk een eigen klassenaam kostten 1,32 s en 339 MB; met ook elk een eigen prefix 3,21 s en 1,36 GB. Een bestand van precies 1073741824 bytes met twee verschillende prefixen kostte 3,9 s en 1,05 GB (gemeten vóór deze wijziging; het prijzen van twee prefixen is niet waar die tijd aan opgaat). Een bestand op de limiet met alleen verschillende prefixen of klassenamen zou veel meer nodig hebben; dat is niet gedraaid. Het geheugen is niet begrensd door deze tool.
- Eén regel met 5.000.000 geciteerde velden (20 MB) wordt in 0,05 s geweigerd met `too many columns`, met ongeveer het geheugen van het bestand; de 65e kolom stopt het lezen. Een gzip-bestand van 100.000 lege leden wordt in 0,45 s gelezen.

Systeemeisen:

- Rust 1.85 of nieuwer om te bouwen. De Linux-binary gebouwd met glibc heeft GLIBC_2.34 of nieuwer nodig. Windows- en macOS-builds zijn hier alleen typegecontroleerd; hun CI-jobs zijn het bewijs zodra ze draaien. Er is geen minimale Windows- of macOS-versie vastgesteld en een musl-build is alleen gelint, niet gedraaid.

De testsuite en wat die dekt staan in [QUALITY.nl.md](QUALITY.nl.md).

## Prijzen en verantwoordelijkheid

De ingebouwde prijzen zijn een momentopname: lijstprijzen voor US East (N. Virginia) van <https://aws.amazon.com/s3/pricing/>, met peildatum 2026-11-09 (de `source_date` in het prijsbestand). Ze verouderen. Controleer zelf de actuele prijzen. Je bent zelf verantwoordelijk voor elke beslissing op basis van de uitvoer; het is een schatting, geen factuur. Geef met `--prices` een eigen prijsbestand mee.

## Onderhoud

Meld bugs via GitHub Issues van deze repository. Na een release staat de tool in sluimermodus: een nieuwe versie komt er alleen bij een gemelde bug, een aanvraag of een beveiligingsprobleem, en beveiliging gaat altijd voor.

## Licentie

MIT OR Apache-2.0. Zie [LICENSE-MIT](LICENSE-MIT) en [LICENSE-APACHE](LICENSE-APACHE). [THIRD-PARTY-LICENSES.html](THIRD-PARTY-LICENSES.html) noemt de derde-partij-crates in de binary en hun licentieteksten. Een release-archief bevat datzelfde bestand.

## Ondersteuning

Meld een bug of stel een vraag via GitHub Issues van deze repository. Voor een beveiligingsprobleem gebruik je de privéroute in [SECURITY.md](SECURITY.md), niet een openbare issue. Bijdragen: zie [CONTRIBUTING.md](CONTRIBUTING.md). Voeg nooit een echte inventory bij; een kleine synthetische CSV die het probleem toont, is genoeg.

## Contact

- E-mail: pep_ai@icloud.com
- Web: <https://www.pepai.nl>
- X: <https://x.com/pepai_nl>
- Ko-fi: <https://ko-fi.com/pepai>
