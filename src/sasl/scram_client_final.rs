//! Проверка SCRAM-SHA-1 client-final полей и точный AuthMessage transcript.
use super::{
    scram_attributes::{self, AttributeName, ParseError},
    scram_client_first::ClientFirst,
    scram_server_first::ServerFirst,
};

/// Подтверждение GS2 channel binding и client proof из одного final message.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClientFinal {
    pub client_final_without_proof: String,
    pub proof: [u8; 20],
}

/// Причины отказа без помещения клиентских полей в diagnostic output.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClientFinalError {
    Attributes(ParseError),
    InvalidFieldSequence,
    InvalidChannelBinding,
    NonceMismatch,
    InvalidProofEncoding,
}

/// Разбирает RFC 5802 client-final для GS2 `n,,` без channel binding.
pub fn parse(message: &str, expected_nonce: &str) -> Result<ClientFinal, ClientFinalError> {
    let attributes = scram_attributes::parse(message).map_err(ClientFinalError::Attributes)?;
    let (channel_binding, nonce, proof) = match attributes.as_slice() {
        [channel_binding, nonce, extensions @ .., proof]
            if channel_binding.name == AttributeName::ChannelBinding
                && nonce.name == AttributeName::Nonce
                && proof.name == AttributeName::Proof
                && extensions
                    .iter()
                    .all(|extension| matches!(extension.name, AttributeName::Extension(_))) =>
        {
            (
                channel_binding.value.as_str(),
                nonce.value.as_str(),
                proof.value.as_str(),
            )
        }
        _ => return Err(ClientFinalError::InvalidFieldSequence),
    };
    validate_channel_binding(channel_binding)?;
    match nonce == expected_nonce {
        true => {}
        false => return Err(ClientFinalError::NonceMismatch),
    }
    let proof = decode_proof(proof)?;
    let (without_proof, _) = message
        .rsplit_once(",p=")
        .ok_or(ClientFinalError::InvalidFieldSequence)?;
    Ok(ClientFinal {
        client_final_without_proof: without_proof.to_owned(),
        proof,
    })
}

impl ClientFinal {
    /// Склеивает три исходных SCRAM transcript поля ровно по RFC 5802 §3.
    pub fn auth_message(&self, client: &ClientFirst, server: &ServerFirst) -> Vec<u8> {
        format!(
            "{},{},{}",
            client.client_first_bare, server.message, self.client_final_without_proof
        )
        .into_bytes()
    }
}

/// В `n,,` channel-binding input равен точной Base64 форме GS2 header.
fn validate_channel_binding(value: &str) -> Result<(), ClientFinalError> {
    let decoded = crate::mime::decode_base64(value.as_bytes())
        .map_err(|_| ClientFinalError::InvalidChannelBinding)?;
    match decoded.as_slice() == b"n,," && crate::sasl::encode_base64(&decoded) == value {
        true => Ok(()),
        false => Err(ClientFinalError::InvalidChannelBinding),
    }
}

/// Принимает только canonical Base64 proof ровно в двадцать SHA-1 октетов.
fn decode_proof(value: &str) -> Result<[u8; 20], ClientFinalError> {
    let decoded = crate::mime::decode_base64(value.as_bytes())
        .map_err(|_| ClientFinalError::InvalidProofEncoding)?;
    match decoded.len() == 20 && crate::sasl::encode_base64(&decoded) == value {
        true => decoded
            .try_into()
            .map_err(|_| ClientFinalError::InvalidProofEncoding),
        false => Err(ClientFinalError::InvalidProofEncoding),
    }
}

#[cfg(test)]
#[path = "scram_client_final_tests.rs"]
mod tests;
