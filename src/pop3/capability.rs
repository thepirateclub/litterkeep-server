//! POP3 CAPA и RFC 6856 LANG responses.

use super::{LanguageRequest, Reply};

/// Формирует расширения RFC 2449 и только реально работающие SASL mechanisms.
///
/// До TLS объявляется STLS, а PLAIN и SCRAM-SHA-1 доступны только внутри
/// защищённого канала. `UTF8 USER` рекламируется вместе с parser/auth path,
/// который принимает UTF-8 credentials и применяет SASLprep.
pub(super) fn capa_reply(tls_active: bool) -> Reply {
    let mut response =
        b"+OK Capability list\r\nUSER\r\nUIDL\r\nTOP\r\nRESP-CODES\r\nUTF8 USER\r\nLANG\r\n"
            .to_vec();
    match tls_active {
        true => response.extend_from_slice(b"SASL PLAIN SCRAM-SHA-1\r\n"),
        false => response.extend_from_slice(b"STLS\r\n"),
    }
    response.extend_from_slice(b".\r\n");
    Reply::Multiline(response)
}

/// Отвечает на RFC 6856 LANG listing или выбирает единственную локаль `en`.
pub(super) fn lang_reply(language: LanguageRequest) -> Reply {
    match language {
        LanguageRequest::Listing => {
            Reply::Multiline(b"+OK Language listing follows\r\nen English\r\n.\r\n".to_vec())
        }
        LanguageRequest::Preferred | LanguageRequest::English => {
            Reply::Ok("en Language selected".into())
        }
        LanguageRequest::Unsupported => Reply::Error("language not supported"),
    }
}
