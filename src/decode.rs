use std::io::Read;
use std::path::Path;

use flate2::bufread::GzDecoder;

use crate::error::Error;

pub(crate) fn exceeds_file_cap(len: u64) -> bool {
    len > crate::MAX_FILE_BYTES
}

pub(crate) fn rejected_extension(path: &Path) -> Option<&'static str> {
    let file_name = path.file_name()?;
    if let Some(text) = file_name.to_str() {
        let name = text.to_ascii_lowercase();
        return classify(|suffix| name.ends_with(suffix));
    }
    let bytes = file_name.as_encoded_bytes();
    classify(|suffix| {
        let suffix = suffix.as_bytes();
        bytes.len() >= suffix.len()
            && bytes[bytes.len() - suffix.len()..].eq_ignore_ascii_case(suffix)
    })
}

fn classify(ends_with: impl Fn(&str) -> bool) -> Option<&'static str> {
    if ends_with(".parquet") || ends_with(".parquet.gz") || ends_with(".parq") {
        Some("parquet inventories are not read; only CSV and gzip CSV are")
    } else if ends_with(".orc") || ends_with(".orc.gz") {
        Some("orc inventories are not read; only CSV and gzip CSV are")
    } else {
        None
    }
}

/// Path as text for an error line: the characters as they are, or the bytes
/// written with ASCII escapes when the path is not UTF-8. Nothing is escaped
/// for the terminal here; `Error::new` does that once for the whole message.
pub fn shown_path(path: &Path) -> String {
    match path.to_str() {
        Some(text) => text.to_string(),
        None => path
            .as_os_str()
            .as_encoded_bytes()
            .escape_ascii()
            .to_string(),
    }
}

/// The most gzip members one file may hold. Every member costs a fixed amount
/// of time even when it is empty, so the count is limited, not only the bytes.
pub(crate) const MAX_GZIP_MEMBERS: u64 = 100_000;

/// Gzip is inflated. Parquet and ORC are rejected. The result is CSV bytes.
pub(crate) fn prepare_bytes(data: &[u8], cap: u64) -> Result<Vec<u8>, Error> {
    if data.len() as u64 > cap {
        return Err(Error::new(format!("file is larger than {cap} bytes")));
    }
    if is_gzip(data) {
        let plain = inflate(data, cap)?;
        sniff(&plain)?;
        Ok(plain)
    } else {
        sniff(data)?;
        Ok(data.to_vec())
    }
}

/// Like `prepare_bytes`, but takes the bytes and keeps a plain file as it is,
/// so the file is not held twice.
pub(crate) fn prepare_owned(data: Vec<u8>, cap: u64) -> Result<Vec<u8>, Error> {
    if is_gzip(&data) {
        prepare_bytes(&data, cap)
    } else if data.len() as u64 > cap {
        Err(Error::new(format!("file is larger than {cap} bytes")))
    } else {
        sniff(&data)?;
        Ok(data)
    }
}

fn inflate(data: &[u8], cap: u64) -> Result<Vec<u8>, Error> {
    let mut rest = data;
    let mut out = Vec::new();
    let mut members = 0u64;
    while !rest.is_empty() {
        members += 1;
        if members > MAX_GZIP_MEMBERS {
            return Err(Error::new(format!(
                "gzip file has more than {MAX_GZIP_MEMBERS} members"
            )));
        }
        let mut decoder = GzDecoder::new(&mut rest);
        read_into(&mut decoder, &mut out, cap).map_err(|err| {
            if err.message().starts_with("file is larger") {
                err
            } else {
                Error::new("not a gzip inventory")
            }
        })?;
    }
    Ok(out)
}

pub(crate) fn read_capped<R: Read>(reader: R, cap: u64) -> Result<Vec<u8>, Error> {
    let mut out = Vec::new();
    read_into(reader, &mut out, cap)?;
    Ok(out)
}

/// Appends to `out` and refuses when `out` would pass `cap` bytes.
fn read_into<R: Read>(mut reader: R, out: &mut Vec<u8>, cap: u64) -> Result<(), Error> {
    let mut buf = [0u8; 64 * 1024];
    loop {
        let read = reader
            .read(&mut buf)
            .map_err(|_| Error::new("read failed"))?;
        if read == 0 {
            break;
        }
        if (out.len() as u64).saturating_add(read as u64) > cap {
            return Err(Error::new(format!("file is larger than {cap} bytes")));
        }
        out.extend_from_slice(&buf[..read]);
    }
    Ok(())
}

fn is_gzip(data: &[u8]) -> bool {
    data.len() >= 2 && data[0] == 0x1f && data[1] == 0x8b
}

fn sniff(data: &[u8]) -> Result<(), Error> {
    if data.starts_with(b"PAR1") {
        return Err(Error::new(
            "parquet inventories are not read; only CSV and gzip CSV are",
        ));
    }
    if data.starts_with(b"ORC") {
        return Err(Error::new(
            "orc inventories are not read; only CSV and gzip CSV are",
        ));
    }
    if data.iter().take(512).any(|byte| *byte == 0) {
        return Err(Error::new("not a CSV inventory"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{prepare_bytes, read_capped, rejected_extension, shown_path};
    use std::path::Path;

    #[test]
    fn shown_path_leaves_escaping_to_the_error() {
        assert_eq!(shown_path(Path::new("a/b.csv")), "a/b.csv");
        assert_eq!(shown_path(Path::new("a\u{202e}b")), "a\u{202e}b");
        assert_eq!(shown_path(Path::new(r"C:\x")), r"C:\x");
    }

    #[test]
    fn extension_check_ignores_case_and_covers_every_listed_suffix() {
        for name in ["a.parquet", "A.PARQUET", "a.parquet.gz", "a.parq", "a.Parq"] {
            assert_eq!(
                rejected_extension(Path::new(name)),
                Some("parquet inventories are not read; only CSV and gzip CSV are"),
                "{name}"
            );
        }
        for name in ["a.orc", "A.ORC", "a.orc.gz"] {
            assert_eq!(
                rejected_extension(Path::new(name)),
                Some("orc inventories are not read; only CSV and gzip CSV are"),
                "{name}"
            );
        }
        for name in ["a.csv", "a.csv.gz", "parquet", "a.orcx", "a.parquet.bak"] {
            assert_eq!(rejected_extension(Path::new(name)), None, "{name}");
        }
    }

    #[test]
    fn nul_byte_is_only_looked_for_in_the_first_512_bytes() {
        let mut inside = vec![b'a'; 600];
        inside[511] = 0;
        let err = prepare_bytes(&inside, 1000).unwrap_err();
        assert_eq!(err.message(), "not a CSV inventory");
        let mut outside = vec![b'a'; 600];
        outside[512] = 0;
        assert_eq!(prepare_bytes(&outside, 1000).unwrap(), outside);
    }

    struct Broken;

    impl std::io::Read for Broken {
        fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
            Err(std::io::Error::other("secret-disk-detail"))
        }
    }

    #[test]
    fn failed_read_has_its_own_text_without_the_os_detail() {
        let err = read_capped(Broken, 10).unwrap_err();
        assert_eq!(err.message(), "read failed");
    }

    #[test]
    fn plain_and_gzip_over_the_cap_say_how_large() {
        let err = prepare_bytes(b"abcdef", 5).unwrap_err();
        assert_eq!(err.message(), "file is larger than 5 bytes");
        let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
        std::io::Write::write_all(&mut encoder, &[b'a'; 600]).unwrap();
        let packed = encoder.finish().unwrap();
        assert!(packed.len() < 100);
        let err = prepare_bytes(&packed, 100).unwrap_err();
        assert_eq!(err.message(), "file is larger than 100 bytes");
        let err = prepare_bytes(&[0x1f, 0x8b, 0, 0], 100).unwrap_err();
        assert_eq!(err.message(), "not a gzip inventory");
    }
}
