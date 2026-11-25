use crate::error::Error;

/// The most columns a record may have, quoted or not.
const MAX_COLUMNS: usize = 64;

/// First path segments of an object key.
///
/// Depth 1 of `logs/2024/a` is `logs/`. A key with fewer segments is the key
/// itself. An empty key is `(empty)`.
#[must_use]
pub(crate) fn prefix_of(key: &str, depth: u8) -> String {
    if key.is_empty() {
        return "(empty)".to_string();
    }
    let depth = usize::from(depth.max(1));
    let mut seen = 0usize;
    for (index, byte) in key.bytes().enumerate() {
        if byte == b'/' {
            seen += 1;
            if seen == depth {
                return key[..=index].to_string();
            }
        }
    }
    key.to_string()
}

pub(crate) fn visit_csv(
    data: &[u8],
    mut row: impl FnMut(u64, &[&[u8]]) -> Result<(), Error>,
) -> Result<(), Error> {
    let data = data.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(data);
    let mut i = 0;
    let mut line_no = 1u64;
    while i < data.len() {
        let start = i;
        let record_line = line_no;
        let mut in_quotes = false;
        // A quote is legal only as the first byte of a field. A later quote is a
        // hand-written file: AWS quotes each field, and accepting the mark here
        // would swallow the next lines and undercount the bill.
        let mut field_started = false;
        // True right after the closing quote of a field: only a comma or the end
        // of the line may follow, or the text would be glued to the field.
        let mut closed = false;
        let mut quote_line = record_line;
        // Lines seen so far inside this record, counted once while scanning.
        let mut newlines = 0u64;
        while i < data.len() {
            let byte = data[i];
            if in_quotes {
                if byte == b'"' {
                    if i + 1 < data.len() && data[i + 1] == b'"' {
                        i += 2;
                        continue;
                    }
                    in_quotes = false;
                    closed = true;
                } else if byte == b'\n' {
                    newlines += 1;
                }
                i += 1;
                continue;
            }
            if closed {
                let line_end = byte == b'\n'
                    || (byte == b'\r' && (i + 1 >= data.len() || data[i + 1] == b'\n'));
                if byte != b',' && !line_end {
                    return Err(Error::new(format!(
                        "row {}: text after a closing quote",
                        record_line + newlines
                    )));
                }
                closed = false;
            }
            if byte == b'"' {
                if field_started {
                    return Err(Error::new(format!(
                        "row {}: quote in an unquoted field",
                        record_line + newlines
                    )));
                }
                in_quotes = true;
                quote_line = record_line + newlines;
                field_started = true;
                i += 1;
                continue;
            }
            if byte == b'\n' {
                break;
            }
            field_started = byte != b',';
            i += 1;
        }
        if in_quotes {
            return Err(Error::new(format!("row {quote_line}: unterminated quote")));
        }
        let mut end = i;
        if i < data.len() && data[i] == b'\n' {
            i += 1;
        }
        line_no += newlines + u64::from(end < i);
        if end > start && data[end - 1] == b'\r' {
            end -= 1;
        }
        if start == end {
            continue;
        }
        let record = &data[start..end];
        if record.contains(&b'"') {
            let owned = unescape_record(record)?;
            let refs: Vec<&[u8]> = owned.iter().map(Vec::as_slice).collect();
            row(record_line, &refs)?;
        } else {
            let mut parts: [&[u8]; MAX_COLUMNS] = [&[]; MAX_COLUMNS];
            let mut count = 0;
            for part in record.split(|c| *c == b',') {
                if count >= parts.len() {
                    return Err(Error::new("too many columns"));
                }
                parts[count] = part;
                count += 1;
            }
            row(record_line, &parts[..count])?;
        }
    }
    Ok(())
}

fn unescape_record(record: &[u8]) -> Result<Vec<Vec<u8>>, Error> {
    let mut fields = Vec::new();
    let mut current = Vec::new();
    let mut in_quotes = false;
    let mut field_started = false;
    let mut i = 0;
    while i < record.len() {
        let byte = record[i];
        if in_quotes {
            if byte == b'"' {
                if i + 1 < record.len() && record[i + 1] == b'"' {
                    current.push(b'"');
                    i += 2;
                    continue;
                }
                in_quotes = false;
                i += 1;
                continue;
            }
            current.push(byte);
            i += 1;
            continue;
        }
        match byte {
            b'"' => {
                if field_started {
                    return Err(Error::new("quote in an unquoted field"));
                }
                in_quotes = true;
                field_started = true;
                i += 1;
            }
            b',' => {
                if fields.len() + 1 >= MAX_COLUMNS {
                    return Err(Error::new("too many columns"));
                }
                fields.push(std::mem::take(&mut current));
                field_started = false;
                i += 1;
            }
            _ => {
                current.push(byte);
                field_started = true;
                i += 1;
            }
        }
    }
    if in_quotes {
        return Err(Error::new("unterminated quote"));
    }
    fields.push(current);
    Ok(fields)
}

pub(crate) fn trim_ascii(bytes: &[u8]) -> &[u8] {
    let start = bytes
        .iter()
        .position(|byte| !byte.is_ascii_whitespace())
        .unwrap_or(bytes.len());
    let end = bytes
        .iter()
        .rposition(|byte| !byte.is_ascii_whitespace())
        .map_or(0, |index| index + 1);
    if start >= end {
        &[]
    } else {
        &bytes[start..end]
    }
}

pub(crate) fn parse_size(bytes: &[u8]) -> Result<u64, ()> {
    let bytes = trim_ascii(bytes);
    if bytes.is_empty() || bytes.iter().any(|byte| !byte.is_ascii_digit()) {
        return Err(());
    }
    let zeros = bytes.iter().take_while(|byte| **byte == b'0').count();
    let bytes = if zeros == bytes.len() {
        &bytes[..1]
    } else {
        &bytes[zeros..]
    };
    if bytes.len() > 20 {
        return Err(());
    }
    let text = std::str::from_utf8(bytes).map_err(|_| ())?;
    text.parse().map_err(|_| ())
}

#[cfg(test)]
mod tests {
    use super::{parse_size, prefix_of, unescape_record, visit_csv};
    use std::time::{Duration, Instant};

    fn rows(data: &str) -> Vec<Vec<String>> {
        let mut out = Vec::new();
        visit_csv(data.as_bytes(), |_, fields| {
            out.push(
                fields
                    .iter()
                    .map(|field| String::from_utf8(field.to_vec()).unwrap())
                    .collect(),
            );
            Ok(())
        })
        .unwrap();
        out
    }

    #[test]
    fn prefix_depth_keeps_a_trailing_slash_only_when_more_segments_exist() {
        assert_eq!(prefix_of("logs/2024/a.bin", 1), "logs/");
        assert_eq!(prefix_of("logs/2024/a.bin", 2), "logs/2024/");
        assert_eq!(prefix_of("logs/2024", 2), "logs/2024");
        assert_eq!(prefix_of("logs", 1), "logs");
        assert_eq!(prefix_of("a/b/", 1), "a/");
        assert_eq!(prefix_of("a/b/", 2), "a/b/");
        assert_eq!(prefix_of("", 1), "(empty)");
    }

    #[test]
    fn quoted_comma_and_newline_stay_inside_the_field() {
        let parsed = rows("Bucket,Key,Size\nb,\"a,b\",10\nb,\"a\nb\",11\n");
        assert_eq!(parsed[1][1], "a,b");
        assert_eq!(parsed[1][2], "10");
        assert_eq!(parsed[2][1], "a\nb");
        assert_eq!(parsed[2][2], "11");
    }

    #[test]
    fn escaped_quote_and_crlf_and_bom() {
        let parsed = rows("\u{feff}Bucket,Key\r\nb,\"say \"\"hi\"\"\"\r\n");
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[1][1], "say \"hi\"");
    }

    #[test]
    fn size_rejects_sign_exponent_and_empty() {
        assert_eq!(parse_size(b"0"), Ok(0));
        assert_eq!(parse_size(b"00010"), Ok(10));
        assert_eq!(parse_size(b" 12 "), Ok(12));
        assert!(parse_size(b"").is_err());
        assert!(parse_size(b"-1").is_err());
        assert!(parse_size(b"1e2").is_err());
        assert!(parse_size(b"1.5").is_err());
        assert!(parse_size(b"+3").is_err());
    }

    #[test]
    fn unterminated_quote_has_no_payload() {
        let err = visit_csv(b"Bucket\n\"secret-key", |_, _| Ok(())).unwrap_err();
        assert_eq!(err.message(), "row 2: unterminated quote");
        assert!(!err.message().contains("secret"));
    }

    #[test]
    fn unescape_record_reports_bare_unterminated_quote() {
        let err = unescape_record(b"a,\"open").unwrap_err();
        assert_eq!(err.message(), "unterminated quote");
        assert!(!err.message().contains("open"));
    }

    #[test]
    fn quote_errors_name_the_file_line() {
        let err = visit_csv(b"H\n\n\"x\ny\",a\"b\n", |_, _| Ok(())).unwrap_err();
        assert_eq!(err.message(), "row 4: quote in an unquoted field");
        let err = visit_csv(b"H\n\nb,\"x\ny\n", |_, _| Ok(())).unwrap_err();
        assert_eq!(err.message(), "row 3: unterminated quote");
    }

    #[test]
    fn empty_lines_are_skipped() {
        let parsed = rows("Bucket,Key\n\nb,k\n");
        assert_eq!(parsed.len(), 2);
    }

    #[test]
    fn quote_inside_an_unquoted_field_is_refused() {
        let data = b"b,a\"b,1,STANDARD\nb,c\"d,2,STANDARD\nb,e,4,STANDARD\n";
        let mut rows = 0u32;
        let err = visit_csv(data, |_, _| {
            rows += 1;
            Ok(())
        })
        .unwrap_err();
        assert_eq!(rows, 0);
        assert_eq!(err.message(), "row 1: quote in an unquoted field");
    }

    #[test]
    fn line_numbers_count_file_lines() {
        let mut seen = Vec::new();
        visit_csv(b"H\n\n\"a\nb\"\r\nc\n", |line, _| {
            seen.push(line);
            Ok(())
        })
        .unwrap();
        assert_eq!(seen, vec![1, 3, 5]);
    }

    #[test]
    fn sixty_four_columns_pass_and_sixty_five_are_refused() {
        let line = |n: usize| vec!["x"; n].join(",");
        let ok = format!("{}\n", line(64));
        let mut widths = Vec::new();
        visit_csv(ok.as_bytes(), |_, fields| {
            widths.push(fields.len());
            Ok(())
        })
        .unwrap();
        assert_eq!(widths, vec![64]);
        let too_wide = format!("{}\n", line(65));
        let err = visit_csv(too_wide.as_bytes(), |_, _| Ok(())).unwrap_err();
        assert_eq!(err.message(), "too many columns");
    }

    #[test]
    fn parse_size_accepts_the_limit_and_refuses_one_more() {
        assert_eq!(parse_size(b"18446744073709551615"), Ok(u64::MAX));
        assert!(parse_size(b"18446744073709551616").is_err());
        let zeros = format!("{}18446744073709551615", "0".repeat(50));
        assert_eq!(parse_size(zeros.as_bytes()), Ok(u64::MAX));
        assert_eq!(parse_size("0".repeat(50).as_bytes()), Ok(0));
        assert!(parse_size(format!("{}5x", "0".repeat(30)).as_bytes()).is_err());
    }

    #[test]
    fn one_line_with_many_quoted_fields_is_read_in_linear_time() {
        let limit = Duration::from_secs(2);
        let quoted = vec!["\"a\""; 200_000].join(",");
        let data = format!("Bucket,Key,Size,StorageClass\n{quoted}\n");
        assert!(data.len() >= 800_000);
        let start = Instant::now();
        let err = visit_csv(data.as_bytes(), |_, _| Ok(())).unwrap_err();
        assert_eq!(err.message(), "too many columns");
        assert!(start.elapsed() < limit, "took {:?}", start.elapsed());
        // The same line with a quote in an unquoted field at the very end.
        let bad = format!("Bucket\n{quoted},x\"y\n");
        let start = Instant::now();
        let err = visit_csv(bad.as_bytes(), |_, _| Ok(())).unwrap_err();
        assert_eq!(err.message(), "row 2: quote in an unquoted field");
        assert!(start.elapsed() < limit, "took {:?}", start.elapsed());
    }
}
