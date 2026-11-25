use std::collections::HashSet;

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum KeyFault {
    Duplicate,
    Bad,
}

/// Walk one JSON value. A repeated key inside any object is [`KeyFault::Duplicate`].
/// A value that does not start with `{` is not a duplicate-key problem.
pub(crate) fn reject_duplicate_keys(text: &str) -> Result<(), KeyFault> {
    let bytes = text.as_bytes();
    let mut i = 0;
    skip_ws(bytes, &mut i);
    if i >= bytes.len() {
        return Err(KeyFault::Bad);
    }
    if bytes[i] != b'{' {
        return Ok(());
    }
    skip_value(bytes, &mut i, 1)?;
    Ok(())
}

fn skip_ws(bytes: &[u8], i: &mut usize) {
    while *i < bytes.len() && bytes[*i].is_ascii_whitespace() {
        *i += 1;
    }
}

fn skip_value(bytes: &[u8], i: &mut usize, depth: u32) -> Result<(), KeyFault> {
    if depth > 128 {
        return Err(KeyFault::Bad);
    }
    skip_ws(bytes, i);
    if *i >= bytes.len() {
        return Err(KeyFault::Bad);
    }
    match bytes[*i] {
        b'"' => {
            parse_json_string(bytes, i)?;
            Ok(())
        }
        b'{' => skip_object(bytes, i, depth),
        b'[' => skip_container(bytes, i, b'[', b']', depth),
        b't' => skip_lit(bytes, i, b"true"),
        b'f' => skip_lit(bytes, i, b"false"),
        b'n' => skip_lit(bytes, i, b"null"),
        b'-' | b'0'..=b'9' => skip_number(bytes, i),
        _ => Err(KeyFault::Bad),
    }
}

fn skip_object(bytes: &[u8], i: &mut usize, depth: u32) -> Result<(), KeyFault> {
    if bytes[*i] != b'{' {
        return Err(KeyFault::Bad);
    }
    *i += 1;
    let mut keys = HashSet::new();
    loop {
        skip_ws(bytes, i);
        if *i < bytes.len() && bytes[*i] == b'}' {
            *i += 1;
            return Ok(());
        }
        let key = parse_json_string(bytes, i)?;
        if !keys.insert(key) {
            return Err(KeyFault::Duplicate);
        }
        skip_ws(bytes, i);
        if *i >= bytes.len() || bytes[*i] != b':' {
            return Err(KeyFault::Bad);
        }
        *i += 1;
        skip_value(bytes, i, depth + 1)?;
        skip_ws(bytes, i);
        if *i < bytes.len() && bytes[*i] == b',' {
            *i += 1;
            continue;
        }
        if *i < bytes.len() && bytes[*i] == b'}' {
            *i += 1;
            return Ok(());
        }
        return Err(KeyFault::Bad);
    }
}

fn skip_container(
    bytes: &[u8],
    i: &mut usize,
    open: u8,
    close: u8,
    depth: u32,
) -> Result<(), KeyFault> {
    if bytes[*i] != open {
        return Err(KeyFault::Bad);
    }
    *i += 1;
    loop {
        skip_ws(bytes, i);
        if *i < bytes.len() && bytes[*i] == close {
            *i += 1;
            return Ok(());
        }
        skip_value(bytes, i, depth + 1)?;
        skip_ws(bytes, i);
        if *i < bytes.len() && bytes[*i] == b',' {
            *i += 1;
            continue;
        }
        if *i < bytes.len() && bytes[*i] == close {
            *i += 1;
            return Ok(());
        }
        return Err(KeyFault::Bad);
    }
}

fn skip_lit(bytes: &[u8], i: &mut usize, lit: &[u8]) -> Result<(), KeyFault> {
    if bytes[*i..].starts_with(lit) {
        *i += lit.len();
        Ok(())
    } else {
        Err(KeyFault::Bad)
    }
}

fn skip_number(bytes: &[u8], i: &mut usize) -> Result<(), KeyFault> {
    let start = *i;
    if bytes[*i] == b'-' {
        *i += 1;
    }
    if *i >= bytes.len() || !bytes[*i].is_ascii_digit() {
        return Err(KeyFault::Bad);
    }
    while *i < bytes.len() && bytes[*i].is_ascii_digit() {
        *i += 1;
    }
    if *i < bytes.len() && bytes[*i] == b'.' {
        *i += 1;
        if *i >= bytes.len() || !bytes[*i].is_ascii_digit() {
            return Err(KeyFault::Bad);
        }
        while *i < bytes.len() && bytes[*i].is_ascii_digit() {
            *i += 1;
        }
    }
    if *i < bytes.len() && (bytes[*i] == b'e' || bytes[*i] == b'E') {
        *i += 1;
        if *i < bytes.len() && (bytes[*i] == b'+' || bytes[*i] == b'-') {
            *i += 1;
        }
        if *i >= bytes.len() || !bytes[*i].is_ascii_digit() {
            return Err(KeyFault::Bad);
        }
        while *i < bytes.len() && bytes[*i].is_ascii_digit() {
            *i += 1;
        }
    }
    if *i == start {
        return Err(KeyFault::Bad);
    }
    Ok(())
}

fn parse_json_string(bytes: &[u8], i: &mut usize) -> Result<String, KeyFault> {
    if *i >= bytes.len() || bytes[*i] != b'"' {
        return Err(KeyFault::Bad);
    }
    *i += 1;
    let mut out = String::new();
    while *i < bytes.len() {
        let byte = bytes[*i];
        *i += 1;
        match byte {
            b'"' => return Ok(out),
            b'\\' => {
                if *i >= bytes.len() {
                    return Err(KeyFault::Bad);
                }
                let esc = bytes[*i];
                *i += 1;
                match esc {
                    b'"' => out.push('"'),
                    b'\\' => out.push('\\'),
                    b'/' => out.push('/'),
                    b'b' => out.push('\u{0008}'),
                    b'f' => out.push('\u{000c}'),
                    b'n' => out.push('\n'),
                    b'r' => out.push('\r'),
                    b't' => out.push('\t'),
                    b'u' => push_unicode_escape(bytes, i, &mut out)?,
                    _ => return Err(KeyFault::Bad),
                }
            }
            0x00..=0x1f => return Err(KeyFault::Bad),
            _ => {
                *i -= 1;
                let width = match byte {
                    0xC0..=0xDF => 2,
                    0xE0..=0xEF => 3,
                    0xF0..=0xF7 => 4,
                    _ => 1,
                };
                let end = (*i + width).min(bytes.len());
                let chunk = std::str::from_utf8(&bytes[*i..end]).map_err(|_| KeyFault::Bad)?;
                let ch = chunk.chars().next().ok_or(KeyFault::Bad)?;
                out.push(ch);
                *i += ch.len_utf8();
            }
        }
    }
    Err(KeyFault::Bad)
}

/// One `\u` unit. A high surrogate must be followed by a `\u` low surrogate,
/// and the pair is one scalar. Either half alone is not a character.
fn push_unicode_escape(bytes: &[u8], i: &mut usize, out: &mut String) -> Result<(), KeyFault> {
    let code = hex_code(bytes, i)?;
    let ch = if (0xD800..=0xDBFF).contains(&code) {
        if *i + 1 >= bytes.len() || bytes[*i] != b'\\' || bytes[*i + 1] != b'u' {
            return Err(KeyFault::Bad);
        }
        *i += 2;
        let low = hex_code(bytes, i)?;
        if !(0xDC00..=0xDFFF).contains(&low) {
            return Err(KeyFault::Bad);
        }
        let combined = 0x1_0000 + (((code - 0xD800) << 10) | (low - 0xDC00));
        char::from_u32(combined).ok_or(KeyFault::Bad)?
    } else {
        char::from_u32(code).ok_or(KeyFault::Bad)?
    };
    out.push(ch);
    Ok(())
}

fn hex_code(bytes: &[u8], i: &mut usize) -> Result<u32, KeyFault> {
    if *i + 4 > bytes.len() {
        return Err(KeyFault::Bad);
    }
    let hex = std::str::from_utf8(&bytes[*i..*i + 4]).map_err(|_| KeyFault::Bad)?;
    let code = u32::from_str_radix(hex, 16).map_err(|_| KeyFault::Bad)?;
    *i += 4;
    Ok(code)
}

#[cfg(test)]
mod tests {
    use super::{reject_duplicate_keys, KeyFault};
    use std::time::{Duration, Instant};

    fn timed(text: &str) -> (Result<(), KeyFault>, Duration) {
        let start = Instant::now();
        let result = reject_duplicate_keys(text);
        (result, start.elapsed())
    }

    #[test]
    fn large_padding_is_scanned_in_linear_time() {
        let limit = Duration::from_millis(500);
        // Many distinct keys, about 2 MB.
        let mut keys = String::from("{");
        for n in 0..150_000 {
            keys.push_str(&format!("\"k{n}\":1,"));
        }
        keys.push_str("\"last\":1}");
        assert!(keys.len() >= 1_000_000);
        let (result, took) = timed(&keys);
        assert_eq!(result, Ok(()));
        assert!(took < limit, "many keys took {took:?}");
        // The same keys with a repeat at the very end.
        let dup = keys.replace("\"last\":1}", "\"k0\":1}");
        let (result, took) = timed(&dup);
        assert_eq!(result, Err(KeyFault::Duplicate));
        assert!(took < limit, "late duplicate took {took:?}");
        // One long non-ASCII string value.
        let wide = format!("{{\"pad\":\"{}\"}}", "\u{e9}\u{1F600}".repeat(300_000));
        assert!(wide.len() >= 1_000_000);
        let (result, took) = timed(&wide);
        assert_eq!(result, Ok(()));
        assert!(took < limit, "non-ASCII padding took {took:?}");
        // Whitespace and a bad tail.
        let tail = format!("{{\"a\":1}}{}x", " ".repeat(1_000_000));
        let (result, took) = timed(&tail);
        assert!(result.is_ok() || result == Err(KeyFault::Bad));
        assert!(took < limit, "whitespace padding took {took:?}");
    }

    #[test]
    fn surrogate_pair_matches_raw_utf8_and_a_lone_half_is_bad() {
        assert_eq!(reject_duplicate_keys(r#"{"\uD83D\uDE00":1}"#), Ok(()));
        assert_eq!(reject_duplicate_keys(r#"{"tag":"\uD83D\uDE00"}"#), Ok(()));
        assert_eq!(
            reject_duplicate_keys("{\"\u{1F600}\":1,\"\\uD83D\\uDE00\":2}"),
            Err(KeyFault::Duplicate)
        );
        assert_eq!(
            reject_duplicate_keys("{\"pre\u{1F600}\":1,\"pre\\ud83d\\uDe00\":2}"),
            Err(KeyFault::Duplicate)
        );
        assert_eq!(
            reject_duplicate_keys("{\"\u{10000}\":1,\"\\uD800\\uDC00\":2}"),
            Err(KeyFault::Duplicate)
        );
        assert_eq!(
            reject_duplicate_keys("{\"caf\u{e9}\":1,\"caf\\u00e9\":2}"),
            Err(KeyFault::Duplicate)
        );
        assert_eq!(
            reject_duplicate_keys(r#"{"a":1,"\u0061":2}"#),
            Err(KeyFault::Duplicate)
        );
        assert_eq!(
            reject_duplicate_keys(r#"{"a":1,"a":2}"#),
            Err(KeyFault::Duplicate)
        );
        assert_eq!(
            reject_duplicate_keys(r#"{"\uD83D\uDE00":1,"\uD83D\uDE01":2}"#),
            Ok(())
        );
        assert_eq!(reject_duplicate_keys(r#"{"\uD800":1}"#), Err(KeyFault::Bad));
        assert_eq!(reject_duplicate_keys(r#"{"\uDE00":1}"#), Err(KeyFault::Bad));
        assert_eq!(
            reject_duplicate_keys(r#"{"\uD800\uD800":1}"#),
            Err(KeyFault::Bad)
        );
        assert_eq!(
            reject_duplicate_keys(r#"{"\uD800\u0041":1}"#),
            Err(KeyFault::Bad)
        );
    }
}
