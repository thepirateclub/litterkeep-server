//! Проверка межсессионной проекции атомарного MOVE на настоящих lease/socket.

use super::super::*;
use crate::reactor::session::SessionTermination;
use crate::runtime::Orchestrator;
use crate::store::Store;
use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn move_updates_source_and_destination_sessions_without_origin_duplicate() {
    let mut lifecycle = Orchestrator::new();
    let (mut origin, _origin_peer) = leased_imap(&mut lifecycle, 1);
    let (mut source_watcher, _source_peer) = leased_imap(&mut lifecycle, 2);
    let (mut destination_watcher, _destination_peer) = leased_imap(&mut lifecycle, 3);
    select(origin.get_mut(), "source-key", 2, false);
    select(source_watcher.get_mut(), "source-key", 2, true);
    select(destination_watcher.get_mut(), "destination-key", 1, true);
    let mut sessions = vec![
        Some(origin),
        Some(source_watcher),
        Some(destination_watcher),
    ];
    let pending = Pending::Copy {
        tag: "A1".into(),
        transfer: imap::Transfer::Move,
        source: "source-key".into(),
        destination: "destination-key".into(),
        source_uids: vec![10],
        source_sequences: vec![1],
    };
    let response = Response::Moved {
        request_id: 7,
        result: Ok(vec![20]),
    };

    imap_idle::notify_transfer(&mut sessions, 0, &pending, &response);

    let origin = find(&mut sessions, "source-key", false);
    assert_eq!(origin.selected_exists, 2);
    assert!(origin.output.is_empty());
    let source = find(&mut sessions, "source-key", true);
    assert_eq!(source.selected_exists, 1);
    assert_eq!(source.output, b"* 1 EXPUNGE\r\n");
    let destination = find(&mut sessions, "destination-key", true);
    assert_eq!(destination.selected_exists, 2);
    assert!(destination.selected_recent.contains(&20));
    assert_eq!(destination.output, b"* 2 EXISTS\r\n");
}

#[test]
fn lagging_subscriber_is_retired_without_harming_healthy_watcher() {
    let mut lifecycle = Orchestrator::new();
    let (mut lagging, _lagging_peer) = leased_imap(&mut lifecycle, 11);
    let (mut healthy, _healthy_peer) = leased_imap(&mut lifecycle, 12);
    select(lagging.get_mut(), "shared-key", 1, true);
    select(healthy.get_mut(), "shared-key", 1, true);
    lagging.get_mut().output.resize(MAX_OUTPUT, b'x');
    let mut sessions = vec![Some(lagging), Some(healthy)];

    imap_idle::notify(&mut sessions, &[(String::from("shared-key"), 41)]);

    let lagging = find_generation(&mut sessions, 11);
    assert_eq!(
        lagging.termination,
        Some(SessionTermination::NotificationOverflow)
    );
    assert!(lagging.output.is_empty());
    let healthy = find_generation(&mut sessions, 12);
    assert_eq!(healthy.output, b"* 2 EXISTS\r\n");
    assert!(healthy.selected_recent.contains(&41));

    session::termination::close_terminated(
        &mut sessions,
        &mut lifecycle,
        &mut MaildropLeases::default(),
    )
    .expect("controlled close");

    assert_eq!(sessions.iter().filter(|entry| entry.is_some()).count(), 1);
    assert_eq!(lifecycle.task_count(), 1);
    assert_eq!(lifecycle.resource_count(), 1);
    assert!(find_generation(&mut sessions, 12).termination.is_none());
}

#[test]
fn thirty_two_idle_sessions_share_no_threads_or_output_queues() {
    let mut lifecycle = Orchestrator::new();
    let mut peers = Vec::new();
    let mut sessions = Vec::new();
    for generation in 1..=32 {
        let (mut lease, peer) = leased_imap(&mut lifecycle, generation);
        select(lease.get_mut(), "shared-key", 0, true);
        sessions.push(Some(lease));
        peers.push(peer);
    }

    imap_idle::notify(&mut sessions, &[(String::from("shared-key"), 7)]);

    assert_eq!(lifecycle.task_count(), 32);
    assert_eq!(lifecycle.resource_count(), 32);
    assert_eq!(peers.len(), 32);
    assert!(sessions.iter_mut().filter_map(Option::as_mut).all(|lease| {
        let session = lease.get_mut();
        matches!(
            (
                session.selected_exists,
                session.selected_recent.contains(&7),
                session.output.as_slice()
            ),
            (1, true, b"* 1 EXISTS\r\n")
        )
    }));
}

#[test]
fn future_select_claims_recent_once_after_live_notifications() {
    let root = temporary_root();
    let mut store = Store::open(&root).expect("store");
    let mailbox = store.resolve_folder("bob", "INBOX").expect("inbox");
    let uid = store.append(&mailbox, b"message").expect("append");
    let mut lifecycle = Orchestrator::new();
    let (mut first, _first_peer) = leased_imap(&mut lifecycle, 1);
    let (mut second, _second_peer) = leased_imap(&mut lifecycle, 2);
    select(first.get_mut(), &mailbox, 0, true);
    select(second.get_mut(), &mailbox, 0, true);
    let mut sessions = vec![Some(first), Some(second)];

    imap_idle::notify(&mut sessions, &[(mailbox, uid)]);

    assert!(
        sessions
            .iter_mut()
            .filter_map(Option::as_mut)
            .all(|lease| { lease.get_mut().selected_recent.contains(&uid) })
    );
    let first_select = store.select_folder("bob", "INBOX").expect("claim");
    assert_eq!(first_select.recent_uids, vec![uid]);
    let next_select = store.select_folder("bob", "INBOX").expect("next select");
    assert!(next_select.recent_uids.is_empty());
    drop(store);
    fs::remove_dir_all(root).expect("cleanup");
}

/// Создаёт ресурс ровно тем же путём владения, что production accept loop.
fn leased_imap(
    lifecycle: &mut Orchestrator<Session>,
    generation: u64,
) -> (SessionLease, TcpStream) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
    let peer = TcpStream::connect(listener.local_addr().expect("address")).expect("peer");
    let (stream, _) = listener.accept().expect("accept");
    let mut session = Session::new(Kind::Imap, stream, generation, None, None).expect("session");
    session.output.clear();
    let task = lifecycle.spawn_task();
    let resource = lifecycle.register_resource(session);
    let lease = lifecycle.acquire(task, resource).expect("lease");
    (lease, peer)
}

/// Устанавливает минимальную selected/IDLE проекцию без обхода протокольной модели.
fn select(session: &mut Session, mailbox: &str, exists: usize, idle: bool) {
    session.selected_mailbox = Some(mailbox.into());
    session.selected_exists = exists;
    if idle {
        session.imap_idle = Some(IdleState {
            tag: "IDLE1".into(),
            mailbox: mailbox.into(),
        });
    }
}

/// Находит сессию по типизированному состоянию ожидания, не по индексу массива.
fn find<'a>(
    sessions: &'a mut [Option<SessionLease>],
    mailbox: &str,
    idle: bool,
) -> &'a mut Session {
    sessions
        .iter_mut()
        .filter_map(Option::as_mut)
        .map(ResourceLease::get_mut)
        .find(|session| {
            matches!(
                (session.selected_mailbox.as_deref(), session.imap_idle.is_some()),
                (Some(selected), waiting) if (selected, waiting) == (mailbox, idle)
            )
        })
        .expect("selected session")
}

/// Находит владельца по generation без прямого обращения к индексу массива.
fn find_generation(sessions: &mut [Option<SessionLease>], generation: u64) -> &mut Session {
    sessions
        .iter_mut()
        .filter_map(Option::as_mut)
        .map(ResourceLease::get_mut)
        .find(|session| session.generation == generation)
        .expect("session generation")
}

/// Создаёт изолированный каталог интеграционного Store без общего test state.
fn temporary_root() -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "litterkeep-idle-recent-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ))
}
