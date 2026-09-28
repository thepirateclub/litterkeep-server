//! Проверяет client-final отдельного ManageSieve SCRAM-SHA-256 exchange.

use super::session::ScramExchange;

/// Результат содержит только server-final SASL payload.
pub(super) enum Outcome {
    Verified(String),
    Rejected(String),
}

/// Проверяет canonical Base64, nonce, proof и server signature.
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

fn verify_message(exchange: &ScramExchange, message: &str) -> Outcome {
    let Some(server_first) = exchange.sha256_server_first.as_ref() else {
        return rejected_encoding();
    };
    let Some(credential) = exchange.sha256_credential.as_ref() else {
        return rejected_encoding();
    };
    let client_final =
        match crate::sasl::scram_sha256_client_final::parse(message, &server_first.nonce) {
            Ok(client_final) => client_final,
            Err(_) => return rejected_encoding(),
        };
    let outcome = crate::sasl::scram_sha256_server_final::ServerFinal::complete(
        &exchange.client_first,
        server_first,
        &client_final,
        credential.verifier(),
    );
    match outcome {
        crate::sasl::scram_sha256_server_final::ServerFinal::Verified(signature) => {
            Outcome::Verified(format!("v={}", crate::sasl::encode_base64(&signature)))
        }
        error @ crate::sasl::scram_sha256_server_final::ServerFinal::InvalidProof
        | error @ crate::sasl::scram_sha256_server_final::ServerFinal::InvalidEncoding => {
            Outcome::Rejected(error.encode())
        }
    }
}

fn rejected_encoding() -> Outcome {
    Outcome::Rejected("e=invalid-encoding".to_owned())
}
