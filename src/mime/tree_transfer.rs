//! Политика выбора и проверки Content-Transfer-Encoding MIME entity.

use super::super::transfer::TransferEncoding;
use super::header_block::{HeaderField, HeaderName};
use super::parameters::MediaType;

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
            let parsed = TransferEncoding::parse(&field.value)?;
            validate_composite_transfer(media_type, &parsed)?;
            Ok(Some(parsed))
        })
        .map(|selected| selected.unwrap_or(TransferEncoding::SevenBit))
}

/// Проверяет RFC 2045 §6.4 для entity с вложенной MIME-структурой.
fn validate_composite_transfer(
    media_type: &MediaType,
    transfer_encoding: &TransferEncoding,
) -> Result<(), &'static str> {
    match is_composite(media_type) {
        false => Ok(()),
        true => match transfer_encoding {
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
}
