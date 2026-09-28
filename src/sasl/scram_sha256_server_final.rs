//! Проверяет SCRAM-SHA-256 proof и кодирует mutual server signature.

use super::{
    scram_client_first::ClientFirst, scram_sha256_client_final::ClientFinal,
    scram_sha256_server_first::ServerFirst, scram_sha256_verifier::ScramSha256Verifier,
};

/// Итог проверки содержит только server-final wire payload.
pub enum ServerFinal {
    Verified([u8; 32]),
    InvalidProof,
    InvalidEncoding,
}

impl ServerFinal {
    /// Сверяет proof и вычисляет серверную подпись для того же transcript.
    pub fn complete(
        client: &ClientFirst,
        server: &ServerFirst,
        response: &ClientFinal,
        verifier: &ScramSha256Verifier,
    ) -> Self {
        let auth_message = response.auth_message(client, server);
        match verifier.verify_client_proof(&auth_message, &response.proof) {
            true => Self::Verified(verifier.server_signature(&auth_message)),
            false => Self::InvalidProof,
        }
    }

    /// Сериализует success или нормативный SCRAM error attribute.
    pub fn encode(&self) -> String {
        match self {
            Self::Verified(signature) => {
                format!("v={}", crate::sasl::encode_base64(signature))
            }
            Self::InvalidProof => "e=invalid-proof".to_owned(),
            Self::InvalidEncoding => "e=invalid-encoding".to_owned(),
        }
    }
}
