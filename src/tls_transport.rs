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

/// Максимальный объём plaintext, который один шаг может выдать protocol FSM.
const MAX_PLAINTEXT_STEP: usize = 16 * 1024;
/// Верхняя граница накопления неполных TLS records одной сессии.
const MAX_ENCRYPTED_INPUT: usize = 64 * 1024;

/// Состояние TLS одного принадлежащего reactor-у клиентского соединения.
pub struct ServerTls {
    connection: ServerConnection,
    encrypted_input: Vec<u8>,
}

impl ServerTls {
    /// Создаёт новый handshake без открытия и без владения TCP-сокетом.
    pub fn new(config: Arc<ServerConfig>) -> Result<Self, rustls::Error> {
        Ok(Self {
            connection: ServerConnection::new(config)?,
            encrypted_input: Vec::new(),
        })
    }

    /// Передаёт полученную часть TLS record и возвращает готовый plaintext.
    ///
    /// Неполный record остаётся в ограниченном буфере до следующего вызова.
    /// Ошибка криптографической обработки возвращается вызывающему reactor-у,
    /// который обязан закрыть сессию и не выдавать ее ресурс повторно.
    pub fn receive(&mut self, encrypted: &[u8]) -> io::Result<Vec<u8>> {
        self.encrypted_input.extend_from_slice(encrypted);
        if self.encrypted_input.len() > MAX_ENCRYPTED_INPUT {
            return Err(io::Error::other("TLS input limit"));
        }
        while let Some(length) = complete_record_length(&self.encrypted_input) {
            let record: Vec<u8> = self.encrypted_input.drain(..length).collect();
            self.connection.read_tls(&mut record.as_slice())?;
            self.connection
                .process_new_packets()
                .map_err(io::Error::other)?;
        }
        let mut plaintext = Vec::new();
        let mut reader = self.connection.reader();
        match reader
            .by_ref()
            .take(MAX_PLAINTEXT_STEP as u64)
            .read_to_end(&mut plaintext)
        {
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {}
            Err(error) => return Err(error),
        }
        Ok(plaintext)
    }

    /// Добавляет plaintext protocol-ответ в TLS writer после завершения handshake.
    pub fn queue_plaintext(&mut self, plaintext: &[u8]) -> io::Result<()> {
        self.connection.writer().write_all(plaintext)
    }

    /// Забирает один накопленный TLS output для неблокирующей записи в TCP.
    pub fn take_encrypted(&mut self) -> io::Result<Vec<u8>> {
        let mut encrypted = Vec::new();
        self.connection.write_tls(&mut encrypted)?;
        Ok(encrypted)
    }

    /// Сообщает reactor-у, нужен ли writable interest для TLS handshake или ответа.
    pub fn wants_write(&self) -> bool {
        self.connection.wants_write()
    }

    /// Сообщает reactor-у, ожидается ли дальнейший TLS input.
    pub fn wants_read(&self) -> bool {
        self.connection.wants_read()
    }
}

/// Возвращает длину полного TLS record или `None`, пока заголовок либо тело малы.
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
        assert!(server.receive(first).is_ok());
        assert!(server.receive(second).is_err());
    }
}
