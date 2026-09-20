//! Отдельный диапазон Store request ID для будущего delivery scheduler-а.

/// Scheduler выделяет ID сверху вниз и не сталкивается с session request ID,
/// которые reactor выдаёт снизу вверх.
pub(super) struct SchedulerIds {
    next: u64,
}

impl SchedulerIds {
    pub(super) fn new() -> Self {
        Self { next: u64::MAX }
    }

    pub(super) fn take(&mut self) -> Option<u64> {
        let value = self.next;
        self.next = self.next.checked_sub(1)?;
        Some(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scheduler_range_is_disjoint_from_session_floor() {
        let mut ids = SchedulerIds::new();
        assert_eq!(ids.take(), Some(u64::MAX));
        assert_eq!(ids.take(), Some(u64::MAX - 1));
    }
}
