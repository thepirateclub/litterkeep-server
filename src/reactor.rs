//! One Linux `poll(2)` reactor for both public plaintext protocol listeners.
//!
//! This deliberately keeps protocol sessions as state machines. A session never
//! owns `Store`: it submits a bounded request to the single storage worker and
//! waits for a completion identified by its slot and generation.
use crate::platform::linux_poll::{self, Interest};
use crate::runtime::{Orchestrator, ResourceLease};
use crate::storage_worker::{Request, Response, WakingStorageWorker};
use crate::{imap, managesieve, pop3, smtp, tls_transport::ServerTls};
use rustls::ServerConfig;
use std::collections::HashMap;
use std::io;
use std::net::{TcpListener, TcpStream};
use std::os::fd::AsFd;
use std::{path::Path, sync::Arc};
#[path = "reactor/completion.rs"]
mod completion;
#[path = "reactor/completion_dispatch.rs"]
mod completion_dispatch;
#[path = "reactor/completion_id.rs"]
mod completion_id;
#[path = "reactor/completion_retry.rs"]
mod completion_retry;
#[path = "reactor/deadline.rs"]
mod deadline;
#[path = "reactor/flag_completion.rs"]
mod flag_completion;
#[path = "reactor/imap_append.rs"]
mod imap_append;
#[path = "reactor/imap_capability.rs"]
mod imap_capability;
#[path = "reactor/imap_check.rs"]
mod imap_check;
#[path = "reactor/imap_copy.rs"]
mod imap_copy;
#[path = "reactor/imap_expunge.rs"]
mod imap_expunge;
#[path = "reactor/imap_fetch_completion.rs"]
mod imap_fetch_completion;
#[path = "reactor/imap_fetch_response.rs"]
mod imap_fetch_response;
#[path = "reactor/imap_folders.rs"]
mod imap_folders;
#[path = "reactor/imap_idle.rs"]
mod imap_idle;
#[path = "reactor/imap_list.rs"]
mod imap_list;
#[path = "reactor/imap_literal.rs"]
mod imap_literal;
#[path = "reactor/imap_search.rs"]
mod imap_search;
#[path = "reactor/imap_select.rs"]
mod imap_select;
#[path = "reactor/imap_selected.rs"]
mod imap_selected;
#[path = "reactor/imap_starttls.rs"]
mod imap_starttls;
#[path = "reactor/imap_status.rs"]
mod imap_status;
#[path = "reactor/imap_subscriptions.rs"]
mod imap_subscriptions;
#[path = "reactor/maildrop_lease.rs"]
mod maildrop_lease;
#[path = "reactor/managesieve_completion.rs"]
mod managesieve_completion;
#[path = "reactor/managesieve_literal.rs"]
mod managesieve_literal;
#[path = "reactor/managesieve_protocol.rs"]
mod managesieve_protocol;
#[path = "reactor/outbound_entry.rs"]
mod outbound_entry;
#[path = "reactor/outbound_scheduler.rs"]
mod outbound_scheduler;
#[cfg(test)]
#[path = "reactor/outbound_scheduler_tests.rs"]
mod outbound_scheduler_tests;
#[path = "reactor/pending.rs"]
mod pending;
#[path = "reactor/pop3_completion.rs"]
mod pop3_completion;
#[path = "reactor/pop3_protocol.rs"]
mod pop3_protocol;
#[path = "reactor/protocol.rs"]
mod protocol;
#[path = "reactor/control.rs"]
mod reactor_control;
#[path = "reactor/scheduler_ids.rs"]
mod scheduler_ids;
#[path = "reactor/server_loop.rs"]
mod server_loop;
#[path = "reactor/session.rs"]
mod session;
#[path = "reactor/session_accept.rs"]
mod session_accept;
#[path = "reactor/session_close.rs"]
mod session_close;
#[path = "reactor/session_io.rs"]
mod session_io;
#[path = "reactor/smtp_completion.rs"]
mod smtp_completion;
#[path = "reactor/smtp_protocol.rs"]
mod smtp_protocol;
#[path = "reactor/submit_id.rs"]
mod submit_id;
use completion_dispatch::complete;
use completion_retry::retry_submissions;
use imap_idle::IdleState;
use imap_literal::LiteralReceiver;
use maildrop_lease::MaildropLeases;
pub use outbound_entry::serve_all_mail_outbound_starttls;
use pending::Pending;
use protocol::process_input;
use reactor_control::service_control;
use session::{Session, SmtpState};
use session_io::{flush, read_session};

type SessionLease = ResourceLease<Session>;

const MAX_SESSIONS: usize = 256;
const MAX_INPUT: usize = 1024 * 1024;
const MAX_MESSAGE: usize = 10 * 1024 * 1024;
const MAX_OUTPUT: usize = MAX_MESSAGE + 16 * 1024;
const ACCEPT_BUDGET: usize = 16;
const READ_BUDGET: usize = 16 * 1024;

#[derive(Clone, Copy)]
enum Kind {
    Smtp,
    Imap,
    Pop3,
    ManageSieve,
}
#[derive(Clone, Copy)]
enum Target {
    SmtpListener,
    ImapListener,
    Pop3Listener,
    ManageSieveListener,
    ControlListener,
    WorkerWake,
    OutboundWake,
    Session(usize, u64),
    ControlSession(usize),
}
#[derive(Clone)]
enum TlsMode {
    Plain,
    Implicit(Arc<ServerConfig>),
    StartTls(Arc<ServerConfig>),
}

/// Запускает plaintext SMTP и IMAP для локального peer-теста без паролей.
pub fn serve(smtp_address: &str, imap_address: &str, root: impl AsRef<Path>) -> io::Result<()> {
    serve_with_tls(smtp_address, imap_address, root, TlsMode::Plain)
}

/// Запускает implicit-TLS SMTP submission и IMAP с одним material для listener-ов.
pub fn serve_tls(
    smtp_address: &str,
    imap_address: &str,
    root: impl AsRef<Path>,
    tls_config: Arc<ServerConfig>,
) -> io::Result<()> {
    serve_with_tls(
        smtp_address,
        imap_address,
        root,
        TlsMode::Implicit(tls_config),
    )
}

/// Запускает plaintext listener с доступным SMTP STARTTLS upgrade.
pub fn serve_starttls(
    smtp_address: &str,
    imap_address: &str,
    root: impl AsRef<Path>,
    tls_config: Arc<ServerConfig>,
) -> io::Result<()> {
    serve_with_tls(
        smtp_address,
        imap_address,
        root,
        TlsMode::StartTls(tls_config),
    )
}

/// Владеет listener-ами, worker-ом и состоянием TLS каждого session.
fn serve_with_tls(
    smtp_address: &str,
    imap_address: &str,
    root: impl AsRef<Path>,
    tls_mode: TlsMode,
) -> io::Result<()> {
    server_loop::serve(
        Some(smtp_address),
        Some(imap_address),
        None,
        None,
        None,
        root,
        tls_mode,
    )
}

/// Запускает POP3 с обязательным STLS перед передачей password.
pub fn serve_pop3_starttls(
    pop3_address: &str,
    root: impl AsRef<Path>,
    tls_config: Arc<ServerConfig>,
) -> io::Result<()> {
    server_loop::serve(
        None,
        None,
        Some(pop3_address),
        None,
        None,
        root,
        TlsMode::StartTls(tls_config),
    )
}

/// Запускает отдельный ManageSieve STARTTLS listener с общим Store owner.
pub fn serve_managesieve_starttls(
    address: &str,
    root: impl AsRef<Path>,
    tls_config: Arc<ServerConfig>,
) -> io::Result<()> {
    server_loop::serve(
        None,
        None,
        None,
        Some(address),
        None,
        root,
        TlsMode::StartTls(tls_config),
    )
}

/// Запускает POP3 с implicit TLS, не выдавая greeting до handshake.
pub fn serve_pop3_tls(
    pop3_address: &str,
    root: impl AsRef<Path>,
    tls_config: Arc<ServerConfig>,
) -> io::Result<()> {
    server_loop::serve(
        None,
        None,
        Some(pop3_address),
        None,
        None,
        root,
        TlsMode::Implicit(tls_config),
    )
}

/// Запускает три STARTTLS listener-а в одном poll-reactor и с одним Store owner.
pub fn serve_all_starttls(
    smtp_address: &str,
    imap_address: &str,
    pop3_address: &str,
    root: impl AsRef<Path>,
    tls_config: Arc<ServerConfig>,
) -> io::Result<()> {
    server_loop::serve(
        Some(smtp_address),
        Some(imap_address),
        Some(pop3_address),
        None,
        None,
        root,
        TlsMode::StartTls(tls_config),
    )
}

/// Запускает все входящие протоколы в одном reactor-е и с одним Store owner.
pub fn serve_all_mail_starttls(
    smtp_address: &str,
    imap_address: &str,
    pop3_address: &str,
    managesieve_address: &str,
    root: impl AsRef<Path>,
    tls_config: Arc<ServerConfig>,
) -> io::Result<()> {
    server_loop::serve(
        Some(smtp_address),
        Some(imap_address),
        Some(pop3_address),
        Some(managesieve_address),
        None,
        root,
        TlsMode::StartTls(tls_config),
    )
}
