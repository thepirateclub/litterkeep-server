//! Неблокирующая пошаговая оболочка `rustls::ServerConnection`.
//!
//! Reactor передаёт сюда только уже прочитанные TCP-байты и отдельно забирает
//! TLS-байты для записи. Поэтому TLS handshake не скрывает блокирующий цикл
//! `complete_io` и может сохранить состояние при `WouldBlock`.
use rustls::{ServerConfig, ServerConnection};
use std::{
    io::{self, Read, Write},
    sync::Arc,
};

/// Верхняя граница одного encrypted шага, передаваемого rustls.
const MAX_ENCRYPTED_STEP: usize = 64 * 1024;
/// Размер короткого чтения из уже обработанных rustls records.
const PLAINTEXT_READ_BLOCK: usize = 16 * 1024;

/// Состояние TLS одного принадлежащего reactor-у клиентского соединения.
pub struct ServerTls {
    connection: ServerConnection,
    encrypted_input: Vec<u8>,
    peer_closed: bool,
}

impl ServerTls {
    /// Создаёт новый handshake без открытия и без владения TCP-сокетом.
    pub fn new(config: Arc<ServerConfig>) -> Result<Self, rustls::Error> {
        Ok(Self {
            connection: ServerConnection::new(config)?,
            encrypted_input: Vec::new(),
            peer_closed: false,
        })
    }

    /// Передаёт полученную часть TLS record и возвращает готовый plaintext.
    ///
    /// Неполный record остаётся в ограниченном буфере до следующего вызова.
    /// Ошибка криптографической обработки возвращается вызывающему reactor-у,
    /// который обязан закрыть сессию и не выдавать ее ресурс повторно.
    pub fn receive(&mut self, encrypted: &[u8]) -> io::Result<Vec<u8>> {
        self.encrypted_input.extend_from_slice(encrypted);
        if self.encrypted_input.len() > MAX_ENCRYPTED_STEP {
            return Err(io::Error::other("TLS input limit"));
        }
        let mut plaintext = Vec::new();
        while let Some(length) = complete_record_length(&self.encrypted_input) {
            let record: Vec<u8> = self.encrypted_input.drain(..length).collect();
            let mut source = record.as_slice();
            while !source.is_empty() {
                let consumed = self.connection.read_tls(&mut source)?;
                let state = self
                    .connection
                    .process_new_packets()
                    .map_err(io::Error::other)?;
                self.peer_closed |= state.peer_has_closed();
                self.drain_plaintext(&mut plaintext)?;
                if consumed == 0 {
                    return Err(io::Error::other("rustls did not consume TLS record"));
                }
            }
        }
        Ok(plaintext)
    }

    /// Забирает доступный plaintext после каждого шага deframer-а rustls.
    fn drain_plaintext(&mut self, plaintext: &mut Vec<u8>) -> io::Result<()> {
        let mut block = [0_u8; PLAINTEXT_READ_BLOCK];
        let mut reader = self.connection.reader();
        loop {
            match reader.read(&mut block) {
                Ok(0) => break,
                Ok(count) => {
                    let next = plaintext.len().saturating_add(count);
                    if next > MAX_ENCRYPTED_STEP {
                        return Err(io::Error::other("TLS plaintext step limit"));
                    }
                    let Some(received) = block.get(..count) else {
                        return Err(io::Error::other("invalid TLS plaintext length"));
                    };
                    plaintext.extend_from_slice(received);
                }
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => break,
                Err(error) => return Err(error),
            }
        }
        Ok(())
    }

    /// Передаёт в bounded rustls buffer только принятую им часть plaintext.
    pub fn queue_plaintext(&mut self, plaintext: &[u8]) -> io::Result<usize> {
        self.connection.writer().write(plaintext)
    }

    /// Сообщает, завершён ли handshake и допустим ли protocol plaintext.
    ///
    /// До этого состояния reactor обязан принимать и отправлять только TLS
    /// records, не показывая клиенту SMTP либо IMAP greeting.
    pub fn is_handshaking(&self) -> bool {
        self.connection.is_handshaking()
    }

    /// Извлекает RFC 9266 tls-exporter только из завершённого TLS 1.3 канала.
    pub fn tls_exporter_channel_binding(&self) -> Option<[u8; 32]> {
        match self.connection.protocol_version() {
            Some(rustls::ProtocolVersion::TLSv1_3) => self
                .connection
                .export_keying_material([0; 32], b"EXPORTER-Channel-Binding", Some(&[]))
                .ok(),
            _ => None,
        }
    }

    /// Забирает один накопленный TLS output для неблокирующей записи в TCP.
    pub fn take_encrypted(&mut self) -> io::Result<Vec<u8>> {
        let mut encrypted = Vec::new();
        self.connection.write_tls(&mut encrypted)?;
        Ok(encrypted)
    }

    /// Начинает TLS shutdown после передачи всех application bytes.
    pub fn send_close_notify(&mut self) {
        self.connection.send_close_notify();
    }

    /// Сообщает reactor-у, нужен ли writable interest для TLS handshake или ответа.
    pub fn wants_write(&self) -> bool {
        self.connection.wants_write()
    }

    /// Сообщает reactor-у, ожидается ли дальнейший TLS input.
    pub fn wants_read(&self) -> bool {
        self.connection.wants_read()
    }

    /// Сообщает, что peer начал TLS shutdown штатным close_notify.
    pub fn peer_closed(&self) -> bool {
        self.peer_closed
    }
}

#[cfg(test)]
#[path = "tls_transport_exporter_tests.rs"]
mod exporter_tests;

/// Возвращает длину полного TLS record, сохраняя неполный хвост для POLLIN.
fn complete_record_length(bytes: &[u8]) -> Option<usize> {
    let header = bytes.get(..5)?;
    let high = *header.get(3)?;
    let low = *header.get(4)?;
    let total = 5_usize.saturating_add(u16::from_be_bytes([high, low]).into());
    bytes.get(..total).map(|_| total)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustls::{ClientConfig, ClientConnection, RootCertStore, pki_types::ServerName};

    /// ClientHello проходит через частичный ввод, не требуя blocking I/O.
    #[test]
    fn partial_client_hello_is_buffered() {
        let roots = RootCertStore::empty();
        let client_config = ClientConfig::builder()
            .with_root_certificates(roots)
            .with_no_client_auth();
        let name = ServerName::try_from("localhost").expect("server name");
        let mut client = ClientConnection::new(Arc::new(client_config), name).expect("client");
        let mut hello = Vec::new();
        client.write_tls(&mut hello).expect("client hello");
        let split = 1;
        let (first, second) = hello.split_at(split);
        let server_config = ServerConfig::builder()
            .with_no_client_auth()
            .with_cert_resolver(Arc::new(rustls::server::ResolvesServerCertUsingSni::new()));
        let mut server = ServerTls::new(Arc::new(server_config)).expect("server");
        assert!(server.is_handshaking());
        assert!(server.tls_exporter_channel_binding().is_none());
        assert!(server.receive(first).is_ok());
        assert!(server.receive(second).is_err());
    }
}
