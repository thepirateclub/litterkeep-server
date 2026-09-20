//! Read-only срез наблюдаемого состояния lifecycle оркестратора.

/// Числа отражают только живые task/resource records без бессрочной истории.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Stats {
    pub ready_tasks: usize,
    pub waiting_tasks: usize,
    pub cancelling_tasks: usize,
    pub completed_tasks: usize,
    pub failed_tasks: usize,
    pub available_resources: usize,
    pub leased_resources: usize,
    pub quarantined_resources: usize,
    pub closed_resources: usize,
    pub ready_queue_depth: usize,
    pub wait_registrations: usize,
}
