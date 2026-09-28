//! SCRAM-SHA-1 proof completion and RFC server-final attribute encoding.
use super::{
    scram_client_final::ClientFinal, scram_client_first::ClientFirst,
    scram_server_first::ServerFirst, scram_sha1_verifier::ScramSha1Verifier,
};

/// Допустимые server-error значения из RFC 5802 §7.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ServerError {
    InvalidEncoding,
    ExtensionsNotSupported,
    InvalidProof,
    ChannelBindingsDontMatch,
    ServerDoesSupportChannelBinding,
    ChannelBindingNotSupported,
    UnsupportedChannelBindingType,
    UnknownUser,
    InvalidUsernameEncoding,
    NoResources,
    Other,
}

/// Server-final attribute: успешная взаимная проверка либо typed error token.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ServerFinal {
    Verified([u8; 20]),
    Error(ServerError),
}

impl ServerFinal {
    /// Проверяет proof и вычисляет подпись только для подтверждённого клиента.
    pub fn complete(
        client: &ClientFirst,
        server: &ServerFirst,
        response: &ClientFinal,
        verifier: &ScramSha1Verifier,
    ) -> Self {
        let auth_message = response.auth_message(client, server);
        match verifier.verify_client_proof(&auth_message, &response.proof) {
            true => Self::Verified(verifier.server_signature(&auth_message)),
            false => Self::Error(ServerError::InvalidProof),
        }
    }

    /// Кодирует один SCRAM server-final message без SASL transport framing.
    pub fn encode(self) -> String {
        match self {
            Self::Verified(signature) => {
                format!("v={}", crate::sasl::encode_base64(&signature))
            }
            Self::Error(error) => format!("e={}", error.attribute_value()),
        }
    }
}

impl ServerError {
    /// Возвращает нормативное wire-значение закрытого error enum.
    fn attribute_value(self) -> &'static str {
        match self {
            Self::InvalidEncoding => "invalid-encoding",
            Self::ExtensionsNotSupported => "extensions-not-supported",
            Self::InvalidProof => "invalid-proof",
            Self::ChannelBindingsDontMatch => "channel-bindings-dont-match",
            Self::ServerDoesSupportChannelBinding => "server-does-support-channel-binding",
            Self::ChannelBindingNotSupported => "channel-binding-not-supported",
            Self::UnsupportedChannelBindingType => "unsupported-channel-binding-type",
            Self::UnknownUser => "unknown-user",
            Self::InvalidUsernameEncoding => "invalid-username-encoding",
            Self::NoResources => "no-resources",
            Self::Other => "other-error",
        }
    }
}

#[cfg(test)]
#[path = "scram_server_final_tests.rs"]
mod tests;
