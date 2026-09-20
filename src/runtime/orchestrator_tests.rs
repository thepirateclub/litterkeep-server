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
fn stats_count_real_leases_and_wait_registrations() {
    let mut lifecycle = Orchestrator::new();
    let task = lifecycle.spawn_task();
    let resource = lifecycle.register_resource("socket");
    let lease = lifecycle.acquire(task, resource).expect("lease");
    let observed = lifecycle.event_version(Event::Completion(7));
    assert_eq!(
        lifecycle.park_on(task, Event::Completion(7), observed),
        WakeResult::Parked
    );
    let stats = lifecycle.stats();
    assert_eq!(stats.waiting_tasks, 1);
    assert_eq!(stats.leased_resources, 1);
    assert_eq!(stats.ready_queue_depth, 0);
    assert_eq!(stats.wait_registrations, 1);
    lifecycle.signal(Event::Completion(7));
    assert_eq!(lifecycle.stats().ready_tasks, 1);
    assert_eq!(lifecycle.stats().ready_queue_depth, 1);
    lifecycle.quarantine(lease).expect("quarantine");
    assert_eq!(lifecycle.stats().quarantined_resources, 1);
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

#[test]
fn cancellation_clears_wait_registration_before_safe_retirement() {
    let mut runtime = Orchestrator::<()>::new();
    let task = runtime.spawn_task();
    let event = Event::Timer(task);
    let observed = runtime.event_version(event);
    assert_eq!(runtime.park_on(task, event, observed), WakeResult::Parked);
    assert!(runtime.request_cancel(task));
    assert_eq!(runtime.stats().wait_registrations, 0);
    assert!(runtime.retire_task(task));
    assert_eq!(runtime.task_count(), 0);
}
