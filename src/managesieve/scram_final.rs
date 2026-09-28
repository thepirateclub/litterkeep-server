//! Проверка последнего SCRAM-SHA-1 сообщения ManageSieve клиента.

use super::session::ScramExchange;

/// Итог содержит только RFC-совместимый server-final payload.
pub(super) enum Outcome {
    Verified(String),
    Rejected(String),
}

/// Проверяет encoding, nonce, channel binding и доказательство клиента.
pub(super) fn verify(exchange: &ScramExchange, response: &str) -> Outcome {
    let decoded = match crate::mime::decode_base64(response.as_bytes()) {
        Ok(decoded) if crate::sasl::encode_base64(&decoded) == response => decoded,
        _ => return rejected_encoding(),
    };
    let message = match String::from_utf8(decoded) {
        Ok(message) => message,
        Err(_) => return rejected_encoding(),
    };
    verify_message(exchange, &message)
}

/// Сверяет client proof с сохранённым verifier и transcript-ом exchange.
fn verify_message(exchange: &ScramExchange, message: &str) -> Outcome {
    let Some(server_first) = exchange.server_first.as_ref() else {
        return rejected_encoding();
    };
    let Some(credential) = exchange.credential.as_ref() else {
        return rejected_encoding();
    };
    let client_final = match crate::sasl::scram_client_final::parse(message, &server_first.nonce) {
        Ok(client_final) => client_final,
        Err(_) => return rejected_encoding(),
    };
    match crate::sasl::scram_server_final::ServerFinal::complete(
        &exchange.client_first,
        server_first,
        &client_final,
        credential.verifier(),
    ) {
        crate::sasl::scram_server_final::ServerFinal::Verified(signature) => {
            let server_final = crate::sasl::scram_server_final::ServerFinal::Verified(signature);
            Outcome::Verified(server_final.encode())
        }
        error @ crate::sasl::scram_server_final::ServerFinal::Error(_) => {
            Outcome::Rejected(error.encode())
        }
    }
}

/// Сообщает о malformed SASL data через безопасный нормативный атрибут.
fn rejected_encoding() -> Outcome {
    Outcome::Rejected(
        crate::sasl::scram_server_final::ServerFinal::Error(
            crate::sasl::scram_server_final::ServerError::InvalidEncoding,
        )
        .encode(),
    )
}
