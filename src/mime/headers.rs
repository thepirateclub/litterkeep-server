//! Байтовый разбор RFC 5322 header block с сохранением исходных диапазонов.

use std::ops::Range;

/// Одно поле в исходном порядке, включая повторы одинаковых имён.
#[derive(Debug, Eq, PartialEq)]
pub struct HeaderField {
    pub name: String,
    pub value: Vec<u8>,
    pub raw_range: Range<usize>,
}

/// Известные имена заголовков, используемые MIME FSM.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HeaderName {
    ContentType,
    ContentTransferEncoding,
    ContentDisposition,
    ContentId,
    Date,
    Subject,
    From,
    Sender,
    ReplyTo,
    To,
    Cc,
    Bcc,
    ResentFrom,
    ResentSender,
    ResentReplyTo,
    ResentTo,
    ResentCc,
    ResentBcc,
    InReplyTo,
    MessageId,
    Other,
}

impl HeaderField {
    /// Преобразует строковое wire-имя в enum на границе parser-а.
    pub fn kind(&self) -> HeaderName {
        HeaderName::parse(&self.name)
    }
}

impl HeaderName {
    /// Разбирает имя без учёта регистра, не распространяя wire-строку в модель.
    pub fn parse(value: &str) -> Self {
        match value.to_ascii_lowercase().as_str() {
            "content-type" => HeaderName::ContentType,
            "content-transfer-encoding" => HeaderName::ContentTransferEncoding,
            "content-disposition" => HeaderName::ContentDisposition,
            "content-id" => HeaderName::ContentId,
            "date" => HeaderName::Date,
            "subject" => HeaderName::Subject,
            "from" => HeaderName::From,
            "sender" => HeaderName::Sender,
            "reply-to" => HeaderName::ReplyTo,
            "to" => HeaderName::To,
            "cc" => HeaderName::Cc,
            "bcc" => HeaderName::Bcc,
            "resent-from" => HeaderName::ResentFrom,
            "resent-sender" => HeaderName::ResentSender,
            "resent-reply-to" => HeaderName::ResentReplyTo,
            "resent-to" => HeaderName::ResentTo,
            "resent-cc" => HeaderName::ResentCc,
            "resent-bcc" => HeaderName::ResentBcc,
            "in-reply-to" => HeaderName::InReplyTo,
            "message-id" => HeaderName::MessageId,
            _ => HeaderName::Other,
        }
    }

    /// RFC 5228 address-test допускает только поля с адресной структурой.
    pub const fn is_address_list(self) -> bool {
        matches!(
            self,
            Self::From
                | Self::Sender
                | Self::ReplyTo
                | Self::To
                | Self::Cc
                | Self::Bcc
                | Self::ResentFrom
                | Self::ResentSender
                | Self::ResentReplyTo
                | Self::ResentTo
                | Self::ResentCc
                | Self::ResentBcc
        )
    }
}

/// Заголовки и точное начало тела в исходном сообщении.
#[derive(Debug, Eq, PartialEq)]
pub struct HeaderBlock {
    pub fields: Vec<HeaderField>,
    pub body_offset: usize,
}

/// Разбирает CRLF header block, объединяя continuation lines одним SP.
pub fn parse(raw: &[u8]) -> Result<HeaderBlock, &'static str> {
    let separator = raw
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .ok_or("missing header separator")?;
    let header_end = separator.saturating_add(2);
    let header_bytes = raw.get(..header_end).ok_or("invalid header boundary")?;
    let header_lines = header_bytes
        .strip_suffix(b"\n")
        .ok_or("header block is not CRLF terminated")?;
    let mut fields: Vec<HeaderField> = Vec::new();
    let mut cursor = 0_usize;
    for line in header_lines.split(|byte| *byte == b'\n') {
        let content = line.strip_suffix(b"\r").ok_or("bare LF in headers")?;
        let start = cursor;
        cursor = cursor.saturating_add(line.len()).saturating_add(1);
        match content.first() {
            Some(b' ' | b'\t') => unfold(fields.last_mut(), content, cursor)?,
            Some(_) => fields.push(parse_field(content, start, cursor)?),
            None => return Err("empty header line"),
        }
    }
    Ok(HeaderBlock {
        fields,
        body_offset: separator.saturating_add(4),
    })
}

fn parse_field(line: &[u8], start: usize, end: usize) -> Result<HeaderField, &'static str> {
    let colon = line
        .iter()
        .position(|byte| *byte == b':')
        .ok_or("malformed header")?;
    let raw_name = line.get(..colon).ok_or("invalid header name")?;
    let name = trim_field_name_whitespace(raw_name);
    if name.is_empty() {
        return Err("invalid header name");
    }
    let valid_name = name
        .iter()
        .all(|byte| matches!(byte, b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-'));
    if !valid_name {
        return Err("invalid header name");
    }
    let value = line
        .get(colon.saturating_add(1)..)
        .ok_or("invalid header value")?;
    Ok(HeaderField {
        name: String::from_utf8(name.to_ascii_lowercase()).map_err(|_| "invalid header name")?,
        value: trim_ascii(value).to_vec(),
        raw_range: start..end,
    })
}

/// Игнорирует WSP перед `:`, как требует профиль header-теста RFC 5228.
fn trim_field_name_whitespace(mut name: &[u8]) -> &[u8] {
    while let Some(rest) = name.strip_suffix(b" ").or_else(|| name.strip_suffix(b"\t")) {
        name = rest;
    }
    name
}

fn unfold(
    previous: Option<&mut HeaderField>,
    continuation: &[u8],
    end: usize,
) -> Result<(), &'static str> {
    let field = previous.ok_or("continuation without a field")?;
    field.value.push(b' ');
    field.value.extend_from_slice(trim_ascii(continuation));
    field.raw_range.end = end;
    Ok(())
}

fn trim_ascii(mut value: &[u8]) -> &[u8] {
    while let Some(rest) = value
        .strip_prefix(b" ")
        .or_else(|| value.strip_prefix(b"\t"))
    {
        value = rest;
    }
    while let Some(rest) = value
        .strip_suffix(b" ")
        .or_else(|| value.strip_suffix(b"\t"))
    {
        value = rest;
    }
    value
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preserves_repeats_unfolding_and_raw_body_offset() {
        let raw =
            b"Received: first\r\nReceived: second\r\n\tcontinued\r\nX-Binary: \xff\r\n\r\nbody";
        let block = parse(raw).expect("headers");
        assert_eq!(block.fields.len(), 3);
        assert_eq!(
            block.fields.get(1).map(|field| field.value.as_slice()),
            Some(b"second continued".as_slice())
        );
        assert_eq!(raw.get(block.body_offset..), Some(b"body".as_slice()));
    }

    #[test]
    fn rejects_bare_lf_and_orphan_continuation() {
        assert!(parse(b"Subject: bad\n\nbody").is_err());
        assert!(parse(b" continuation\r\n\r\nbody").is_err());
    }

    #[test]
    fn ignores_whitespace_between_field_name_and_colon() {
        let block = parse(b"Subject \t: invoice\r\n\r\nbody").expect("header");
        assert_eq!(
            block.fields.first().map(|field| field.name.as_str()),
            Some("subject")
        );
        assert_eq!(
            block.fields.first().map(|field| field.value.as_slice()),
            Some(b"invoice".as_slice())
        );
    }
}
