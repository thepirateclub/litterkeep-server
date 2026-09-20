//! Проверка межсессионной проекции атомарного MOVE на настоящих lease/socket.

use super::super::*;
use crate::runtime::Orchestrator;

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

    imap_idle::notify_transfer(&mut sessions, 0, &pending, &response).expect("notify");

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
