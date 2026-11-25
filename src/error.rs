use std::borrow::Cow;
use std::fmt::{self, Write as _};

/// An input or usage problem. The message never includes file contents.
/// The process exit code is chosen by the binary: 0 or 2.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error {
    message: String,
}

impl Error {
    #[must_use]
    pub fn new(message: impl Into<String>) -> Self {
        let message = message.into();
        let message = if message.chars().any(needs_escape) {
            escape_controls(&message).into_owned()
        } else {
            message
        };
        Self { message }
    }

    /// Text without the `s3-inventory-bill:` prefix.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for Error {}

/// Control characters would open a new text line or send a terminal sequence.
/// Line and paragraph separators and bidirectional marks or isolates would forge
/// or reverse a line, so they get the same visible escape.
/// A backslash is doubled so a literal `\n` differs from a real newline.
/// Printable text, including `/` in a prefix, is left as itself.
pub fn escape_controls(text: &str) -> Cow<'_, str> {
    if !text.chars().any(needs_escape) {
        return Cow::Borrowed(text);
    }
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        if ch == '\\' {
            out.push_str("\\\\");
        } else if ch.is_control() {
            out.extend(ch.escape_debug());
        } else if needs_escape(ch) {
            let _ = write!(out, "\\u{{{:x}}}", u32::from(ch));
        } else {
            out.push(ch);
        }
    }
    Cow::Owned(out)
}

pub(crate) fn needs_escape(ch: char) -> bool {
    ch == '\\'
        || ch.is_control()
        || matches!(
            ch,
            '\u{00AD}'
                | '\u{034F}'
                | '\u{061C}'
                | '\u{115F}'
                | '\u{1160}'
                | '\u{17B4}'
                | '\u{17B5}'
                | '\u{180B}'..='\u{180E}'
                | '\u{200B}'..='\u{200F}'
                | '\u{2028}'..='\u{202E}'
                | '\u{2060}'..='\u{206F}'
                | '\u{2800}'
                | '\u{3164}'
                | '\u{FE00}'..='\u{FE0F}'
                | '\u{FFA0}'
                | '\u{FFF9}'..='\u{FFFB}'
                | '\u{FEFF}'
                | '\u{E0000}'..='\u{E007F}'
        )
}

#[cfg(test)]
mod tests {
    use super::Error;

    #[test]
    fn error_text_escapes_override_separator_and_backslash_n() {
        let err = Error::new("nofile\u{202E}gnp.csv nofile\u{2028}forged.csv");
        let text = err.message();
        assert!(!text.contains('\u{202E}'), "{text}");
        assert!(!text.contains('\u{2028}'), "{text}");
        assert_eq!(text, "nofile\\u{202e}gnp.csv nofile\\u{2028}forged.csv");

        let literal = Error::new(r"line\n");
        let newline = Error::new("line\n");
        assert_ne!(literal.message(), newline.message());
        assert_eq!(literal.message(), r"line\\n");
        assert_eq!(newline.message(), r"line\n");
        assert!(!literal.message().contains('\n'));
        assert!(!newline.message().contains('\n'));
    }

    #[test]
    fn error_text_escapes_each_hidden_filler_and_selector() {
        let mut chars = vec!['\u{115F}', '\u{1160}', '\u{17B4}', '\u{17B5}', '\u{FFA0}'];
        chars.extend('\u{180B}'..='\u{180D}');
        chars.extend('\u{FE00}'..='\u{FE0E}');
        chars.extend('\u{FFF9}'..='\u{FFFB}');
        for ch in chars {
            let text = Error::new(format!("a{ch}b")).message().to_string();
            assert!(!text.contains(ch), "{:x}", u32::from(ch));
            assert_eq!(text, format!("a\\u{{{:x}}}b", u32::from(ch)));
        }
    }
}
