//! Linux `poll(2)` reactor: session FSM submits bounded work to the sole Store owner.
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
#[path = "reactor/completion_managesieve.rs"]
mod completion_managesieve;
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
#[path = "reactor/imap_fetch_seen.rs"]
mod imap_fetch_seen;
#[path = "reactor/imap_fetch_seen_response.rs"]
mod imap_fetch_seen_response;
#[path = "reactor/imap_folders.rs"]
mod imap_folders;
#[path = "reactor/imap_id.rs"]
mod imap_id;
#[path = "reactor/imap_idle.rs"]
mod imap_idle;
#[path = "reactor/imap_line_context.rs"]
mod imap_line_context;
#[path = "reactor/imap_list.rs"]
mod imap_list;
mod imap_list_selection;
mod imap_list_status;
#[path = "reactor/imap_namespace.rs"]
mod imap_namespace;
#[path = "reactor/imap_protocol.rs"]
mod imap_protocol;
#[path = "reactor/imap_search.rs"]
mod imap_search;
#[path = "reactor/imap_search_literal.rs"]
mod imap_search_literal;
#[path = "reactor/imap_select.rs"]
mod imap_select;
#[path = "reactor/imap_select_flags.rs"]
mod imap_select_flags;
#[path = "reactor/imap_selected.rs"]
mod imap_selected;
#[path = "reactor/imap_starttls.rs"]
mod imap_starttls;
#[path = "reactor/imap_status.rs"]
mod imap_status;
#[path = "reactor/imap_subscriptions.rs"]
mod imap_subscriptions;
#[path = "reactor/imap_unselect.rs"]
mod imap_unselect;
#[path = "reactor/listener.rs"]
mod listener;
#[path = "reactor/maildrop_lease.rs"]
mod maildrop_lease;
#[path = "reactor/managesieve_completion.rs"]
mod managesieve_completion;
#[path = "reactor/managesieve_havespace_literal.rs"]
mod managesieve_havespace_literal;
#[path = "reactor/managesieve_literal.rs"]
mod managesieve_literal;
#[path = "reactor/managesieve_literal_terminator.rs"]
mod managesieve_literal_terminator;
#[path = "reactor/managesieve_protocol.rs"]
mod managesieve_protocol;
#[path = "reactor/managesieve_rename_literal.rs"]
mod managesieve_rename_literal;
#[path = "reactor/managesieve_scram_protocol.rs"]
mod managesieve_scram_protocol;
#[path = "reactor/managesieve_string.rs"]
mod managesieve_string;
#[path = "reactor/outbound_entry.rs"]
mod outbound_entry;
#[path = "reactor/outbound_scheduler.rs"]
mod outbound_scheduler;
#[path = "reactor/outbound_scheduler_submit.rs"]
mod outbound_scheduler_submit;
#[cfg(test)]
#[path = "reactor/outbound_scheduler_tests.rs"]
mod outbound_scheduler_tests;
#[cfg(test)]
#[path = "reactor/outbound_scheduler_tls_tests.rs"]
mod outbound_scheduler_tls_tests;
#[path = "reactor/pending.rs"]
mod pending;
#[path = "reactor/pop3_completion.rs"]
mod pop3_completion;
#[path = "reactor/pop3_protocol.rs"]
mod pop3_protocol;
#[path = "reactor/pop3_scram_protocol.rs"]
mod pop3_scram_protocol;
#[path = "reactor/protocol.rs"]
mod protocol;
#[path = "reactor/control.rs"]
mod reactor_control;
#[path = "reactor/scheduler_ids.rs"]
mod scheduler_ids;
#[path = "reactor/scheduler_lifecycle.rs"]
mod scheduler_lifecycle;
#[path = "reactor/server_all_starttls.rs"]
mod server_all_starttls;
#[path = "reactor/server_entry.rs"]
mod server_entry;
#[path = "reactor/server_loop.rs"]
mod server_loop;
#[path = "reactor/session.rs"]
mod session;
#[path = "reactor/session_accept.rs"]
mod session_accept;
#[path = "reactor/session_cancel.rs"]
mod session_cancel;
#[path = "reactor/session_close.rs"]
mod session_close;
#[path = "reactor/session_io.rs"]
mod session_io;
mod smtp_auth;
#[path = "reactor/smtp_bdat.rs"]
mod smtp_bdat;
#[path = "reactor/smtp_bdat_chunk.rs"]
mod smtp_bdat_chunk;
#[path = "reactor/smtp_bdat_completion.rs"]
mod smtp_bdat_completion;
#[path = "reactor/smtp_completion.rs"]
mod smtp_completion;
#[path = "reactor/smtp_data.rs"]
mod smtp_data;
#[path = "reactor/smtp_greeting.rs"]
mod smtp_greeting;
mod smtp_protocol;
#[path = "reactor/smtp_transaction_command.rs"]
mod smtp_transaction_command;
#[path = "reactor/submit_id.rs"]
mod submit_id;
use completion_dispatch::complete;
use completion_retry::retry_submissions;
use imap_idle::IdleState;
use maildrop_lease::MaildropLeases;
pub use outbound_entry::{serve_all_mail_outbound_starttls, serve_all_mail_outbound_tls_starttls};
use pending::{AuthenticationCommand, Pending, SubscriptionResponse};
use protocol::process_input;
use reactor_control::service_control;
pub use server_all_starttls::serve_all_starttls;
pub(super) use server_entry::TlsMode;
pub use server_entry::{
    serve, serve_all_mail_implicit_tls, serve_all_mail_starttls, serve_managesieve_starttls,
    serve_pop3_starttls, serve_pop3_tls, serve_starttls, serve_tls,
};
use session::{Session, SmtpAuthState, SmtpState};
use session_io::{flush, read_session};
type SessionLease = ResourceLease<Session>;
const MAX_SESSIONS: usize = 256;
const MAX_INPUT: usize = 1024 * 1024;
// Полное MIME-сообщение учитывает base64 overhead 10 MiB-вложения.
const MAX_MESSAGE: usize = crate::smtp::MAX_MESSAGE_SIZE as usize;
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
