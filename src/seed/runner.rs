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
    pub qa_password: Vec<u8>,
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
    let mut qa_imap = Client::connect(
        &config.imap_address,
        &config.server_name,
        &config.ca_der,
        "qa",
        &config.qa_password,
    )?;
    let mut smtp = crate::outbound_tls::Connection::connect(
        &config.smtp_address,
        &config.server_name,
        &config.ca_der,
    )?;
    smtp.authenticate(&config.account, &config.password)?;
    let mut created = 0_usize;
    let mut existing = 0_usize;
    for item in fixture::all() {
        let primary_exists = client_for(item.owner, &mut alice_imap, &mut bob_imap, &mut qa_imap)
            .contains(item.mailbox.wire(), &item.id)?;
        let additional_exists = match item.additional_owner {
            Some(owner) => client_for(owner, &mut alice_imap, &mut bob_imap, &mut qa_imap)
                .contains(item.mailbox.wire(), &item.id)?,
            None => true,
        };
        match (primary_exists, additional_exists) {
            (true, true) => {
                existing = checked_increment(existing)?;
                continue;
            }
            (false, false) => {}
            (false, true) if item.additional_owner.is_none() => {}
            _ => return Err(io::Error::other("partial multi-recipient seed fixture")),
        }
        match item.delivery {
            Delivery::Smtp => {
                let mut recipients = vec![item.recipient.clone()];
                if let Some(recipient) = &item.additional_recipient {
                    recipients.push(recipient.clone());
                }
                match smtp.deliver_many("alice@litterkeep.test", &recipients, &item.message)? {
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
                client_for(item.owner, &mut alice_imap, &mut bob_imap, &mut qa_imap).append(
                    item.mailbox.wire(),
                    &item.flags,
                    &item.message,
                )?;
            }
        }
        let primary_visible = client_for(item.owner, &mut alice_imap, &mut bob_imap, &mut qa_imap)
            .contains(item.mailbox.wire(), &item.id)?;
        let additional_visible = match item.additional_owner {
            Some(owner) => client_for(owner, &mut alice_imap, &mut bob_imap, &mut qa_imap)
                .contains(item.mailbox.wire(), &item.id)?,
            None => true,
        };
        if !matches!((primary_visible, additional_visible), (true, true)) {
            return Err(io::Error::other("seed fixture not visible after commit"));
        }
        created = checked_increment(created)?;
    }
    Ok(SeedReport { created, existing })
}

fn client_for<'a>(
    owner: Owner,
    alice: &'a mut Client,
    bob: &'a mut Client,
    qa: &'a mut Client,
) -> &'a mut Client {
    match owner {
        Owner::Alice => alice,
        Owner::Bob => bob,
        Owner::Qa => qa,
    }
}

fn checked_increment(value: usize) -> io::Result<usize> {
    value
        .checked_add(1)
        .ok_or_else(|| io::Error::other("seed counter overflow"))
}
