//! Политика выбора и проверки Content-Transfer-Encoding MIME entity.

use super::super::header_block::{HeaderField, HeaderName};
use super::super::parameters::MediaType;
use super::super::structured_comments;
use super::super::transfer::TransferEncoding;

/// Разбирает единственное CTE, отвергая неоднозначные повторные поля.
pub(super) fn resolve(
    fields: &[HeaderField],
    media_type: &MediaType,
) -> Result<TransferEncoding, &'static str> {
    fields
        .iter()
        .filter(|field| matches!(field.kind(), HeaderName::ContentTransferEncoding))
        .try_fold(None, |selected, field| {
            if selected.is_some() {
                return Err("duplicate Content-Transfer-Encoding field");
            }
            let parsed = parse_transfer_encoding(&field.value)?;
            validate_composite_transfer(media_type, &parsed)?;
            Ok(Some(parsed))
        })
        .map(|selected| selected.unwrap_or(TransferEncoding::SevenBit))
}

/// Удаляет RFC 822 comments до разбора MIME CTE token.
fn parse_transfer_encoding(value: &[u8]) -> Result<TransferEncoding, &'static str> {
    let value = std::str::from_utf8(value).map_err(|_| "transfer encoding is not ASCII")?;
    let uncommented = structured_comments::remove(value)?;
    TransferEncoding::parse(uncommented.as_bytes())
}

/// Проверяет RFC 2045 §6.4 для entity с вложенной MIME-структурой.
fn validate_composite_transfer(
    media_type: &MediaType,
    transfer_encoding: &TransferEncoding,
) -> Result<(), &'static str> {
    match (media_type, is_composite(media_type)) {
        (MediaType::MessageGlobal, _) | (_, false) => Ok(()),
        (_, true) => match transfer_encoding {
            TransferEncoding::SevenBit | TransferEncoding::EightBit | TransferEncoding::Binary => {
                Ok(())
            }
            TransferEncoding::Base64
            | TransferEncoding::QuotedPrintable
            | TransferEncoding::Unsupported(_) => Err("encoded composite MIME entity is forbidden"),
        },
    }
}

/// Классифицирует multipart и message типы по уже разобранному enum.
fn is_composite(media_type: &MediaType) -> bool {
    matches!(
        media_type,
        MediaType::MultipartMixed
            | MediaType::MultipartAlternative
            | MediaType::MultipartRelated
            | MediaType::MultipartOther(_)
            | MediaType::MessageRfc822
            | MediaType::MessageGlobal
            | MediaType::MessageOther(_)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_absent_and_single_leaf_transfer_encoding() {
        let headers = super::super::header_block::parse(
            b"Content-Type: text/plain\r\nContent-Transfer-Encoding: base64\r\n\r\nbody",
        )
        .expect("headers");

        assert_eq!(
            resolve(&headers.fields, &MediaType::TextPlain),
            Ok(TransferEncoding::Base64)
        );
        assert_eq!(
            resolve(&[], &MediaType::TextPlain),
            Ok(TransferEncoding::SevenBit)
        );
    }

    #[test]
    fn ignores_comments_around_transfer_encoding_token() {
        let headers = super::super::header_block::parse(
            b"Content-Transfer-Encoding: (created by client) base64 (RFC 2045)\r\n\r\nbody",
        )
        .expect("headers");

        assert_eq!(
            resolve(&headers.fields, &MediaType::TextPlain),
            Ok(TransferEncoding::Base64)
        );
    }
}
