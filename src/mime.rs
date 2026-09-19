//! Deliberately small MIME primitives used by the protocol and independent tests.
use std::collections::BTreeMap;

pub fn headers(raw: &[u8]) -> Result<(BTreeMap<String, String>, &[u8]), &'static str> {
    let split = raw
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .ok_or("missing header separator")?;
    let text = std::str::from_utf8(&raw[..split]).map_err(|_| "headers are not utf-8")?;
    let mut out = BTreeMap::new();
    for line in text.split("\r\n") {
        let (name, value) = line.split_once(':').ok_or("malformed header")?;
        if name.is_empty() {
            return Err("empty header name");
        }
        out.insert(name.to_ascii_lowercase(), value.trim().to_owned());
    }
    Ok((out, &raw[split + 4..]))
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
        if chunk[0] == 64 || chunk[1] == 64 || (chunk[2] == 64 && chunk[3] != 64) {
            return Err("invalid base64 padding");
        }
        out.push((chunk[0] << 2) | (chunk[1] >> 4));
        if chunk[2] != 64 {
            out.push((chunk[1] << 4) | (chunk[2] >> 2));
        }
        if chunk[3] != 64 {
            out.push((chunk[2] << 6) | chunk[3]);
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
        assert_eq!(h["subject"], "test");
        assert_eq!(decode_base64(body).expect("base64"), b"hello\0");
    }
    #[test]
    fn rejects_invalid_base64() {
        assert!(decode_base64(b"xx=").is_err());
    }
}
