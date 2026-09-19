use super::*;
#[test]
fn exclusive_resource_cannot_be_double_issued() {
    let mut r = Orchestrator::new();
    let a = r.spawn_task();
    let b = r.spawn_task();
    let id = r.register_resource("tcp");
    let lease = r.acquire(a, id).unwrap();
    assert!(matches!(
        r.acquire(b, id),
        Err(AcquireError::NotAvailable(ResourceState::Leased))
    ));
    r.release(lease).unwrap();
    assert!(r.acquire(b, id).is_ok());
}
#[test]
fn signal_between_observation_and_park_is_not_lost() {
    let mut r: Orchestrator<()> = Orchestrator::new();
    let task = r.spawn_task();
    let event = Event::MailboxRevision(7);
    let seen = r.event_version(event);
    r.signal(event);
    assert_eq!(r.park_on(task, event, seen), WakeResult::AlreadyReady);
}
#[test]
fn timeout_cancel_does_not_release_live_lease() {
    let mut r = Orchestrator::new();
    let task = r.spawn_task();
    let other = r.spawn_task();
    let id = r.register_resource(42_u8);
    let lease = r.acquire(task, id).unwrap();
    assert!(r.request_cancel(task));
    assert!(matches!(
        r.acquire(other, id),
        Err(AcquireError::NotAvailable(ResourceState::Leased))
    ));
    r.quarantine(lease).unwrap();
}

#[test]
fn close_drops_value_and_terminal_registry_records() {
    let mut runtime = Orchestrator::new();
    let task = runtime.spawn_task();
    let resource = runtime.register_resource(String::from("tcp/tls session"));
    let lease = runtime.acquire(task, resource).expect("lease");
    runtime.close(lease).expect("close");
    assert_eq!(runtime.resource_count(), 0);
    assert_eq!(runtime.task_count(), 0);
    assert!(matches!(
        runtime.acquire(task, resource),
        Err(AcquireError::MissingTask)
    ));
}
