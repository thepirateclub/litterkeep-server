//! Минимальные MIME-примитивы для protocol и независимых проверок.
use std::collections::BTreeMap;

#[path = "mime/headers.rs"]
pub mod header_block;
#[path = "mime/quoted_printable.rs"]
pub mod quoted_printable;

pub fn headers(raw: &[u8]) -> Result<(BTreeMap<String, String>, &[u8]), &'static str> {
    let parsed = header_block::parse(raw)?;
    let mut out = BTreeMap::new();
    for field in parsed.fields {
        let value = String::from_utf8(field.value).map_err(|_| "header value is not utf-8")?;
        out.insert(field.name, value);
    }
    Ok((
        out,
        raw.get(parsed.body_offset..)
            .ok_or("invalid body boundary")?,
    ))
}

pub fn decode_base64(input: &[u8]) -> Result<Vec<u8>, &'static str> {
    let mut values = Vec::new();
    for &b in input {
        if b.is_ascii_whitespace() {
            continue;
        }
        values.push(match b {
            b'A'..=b'Z' => b - b'A',
            b'a'..=b'z' => b - b'a' + 26,
            b'0'..=b'9' => b - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            b'=' => 64,
            _ => return Err("invalid base64 character"),
        });
    }
    if values.len() % 4 != 0 {
        return Err("invalid base64 width");
    }
    let mut out = Vec::new();
    for chunk in values.chunks_exact(4) {
        let Some(&[first, second, third, fourth]) = chunk.get(..4) else {
            return Err("invalid base64 chunk");
        };
        match (first == 64, second == 64, third == 64, fourth == 64) {
            (true, _, _, _) | (false, true, _, _) | (false, false, true, false) => {
                return Err("invalid base64 padding");
            }
            _ => {}
        }
        out.push((first << 2) | (second >> 4));
        if third != 64 {
            out.push((second << 4) | (third >> 2));
        }
        if fourth != 64 {
            out.push((third << 6) | fourth);
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_headers_and_attachment_bytes() {
        let raw = b"Subject: test\r\nContent-Transfer-Encoding: base64\r\n\r\naGVsbG8A";
        let (h, body) = headers(raw).expect("headers");
        assert_eq!(h.get("subject"), Some(&"test".to_owned()));
        assert_eq!(decode_base64(body).expect("base64"), b"hello\0");
    }
    #[test]
    fn rejects_invalid_base64() {
        assert!(decode_base64(b"xx=").is_err());
    }
}
