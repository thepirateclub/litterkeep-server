//! Оркестрация идемпотентного seed через реальные SMTP и IMAP соединения.

use super::fixture::{self, Delivery, Owner};
use super::imap_client::Client;
use crate::outbound::DeliveryOutcome;
use std::io;

/// Явная конфигурация локальных TLS endpoints и credential-файла.
pub struct SeedConfig {
    pub smtp_address: String,
    pub imap_address: String,
    pub server_name: String,
    pub ca_der: Vec<u8>,
    pub account: String,
    pub password: Vec<u8>,
    pub bob_password: Vec<u8>,
}

/// Измеримый результат одного запуска без удаления пользовательских сообщений.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SeedReport {
    pub created: usize,
    pub existing: usize,
}

/// Добавляет только отсутствующие fixture ID и повторно подтверждает их по IMAP.
pub fn run(config: &SeedConfig) -> io::Result<SeedReport> {
    let mut alice_imap = Client::connect(
        &config.imap_address,
        &config.server_name,
        &config.ca_der,
        &config.account,
        &config.password,
    )?;
    let mut bob_imap = Client::connect(
        &config.imap_address,
        &config.server_name,
        &config.ca_der,
        "bob",
        &config.bob_password,
    )?;
    let mut smtp = crate::outbound_tls::Connection::connect(
        &config.smtp_address,
        &config.server_name,
        &config.ca_der,
    )?;
    smtp.authenticate(&config.account, &config.password)?;
    let mut created = 0_usize;
    let mut existing = 0_usize;
    for item in fixture::initial() {
        let imap = match item.owner {
            Owner::Alice => &mut alice_imap,
            Owner::Bob => &mut bob_imap,
        };
        if imap.contains(item.mailbox.wire(), item.id)? {
            existing = checked_increment(existing)?;
            continue;
        }
        match item.delivery {
            Delivery::Smtp => {
                match smtp.deliver("alice@litterkeep.test", item.recipient, item.message)? {
                    DeliveryOutcome::Delivered => {}
                    DeliveryOutcome::Deferred => {
                        return Err(io::Error::other("seed SMTP deferred"));
                    }
                    DeliveryOutcome::PermanentFailure => {
                        return Err(io::Error::other("seed SMTP rejected"));
                    }
                }
            }
            Delivery::ImapAppend => {
                imap.append(item.mailbox.wire(), item.flags, item.message)?;
            }
        }
        if !imap.contains(item.mailbox.wire(), item.id)? {
            return Err(io::Error::other("seed fixture not visible after commit"));
        }
        created = checked_increment(created)?;
    }
    Ok(SeedReport { created, existing })
}

fn checked_increment(value: usize) -> io::Result<usize> {
    value
        .checked_add(1)
        .ok_or_else(|| io::Error::other("seed counter overflow"))
}
