//! Ограниченная lease-таблица POP3 maildrop без удержания Store на TCP-сеансе.

use std::collections::HashMap;

/// Один POP3 account может владеть snapshot-операциями ровно одной сессии.
#[derive(Default)]
pub(super) struct MaildropLeases {
    owners: HashMap<String, Owner>,
}

#[derive(Clone, Copy, Eq, PartialEq)]
struct Owner {
    slot: usize,
    generation: u64,
}

impl MaildropLeases {
    /// Выдаёт lease только отсутствующему account, не меняя Store или snapshot.
    pub(super) fn acquire(&mut self, account: String, slot: usize, generation: u64) -> bool {
        match self.owners.entry(account) {
            std::collections::hash_map::Entry::Vacant(entry) => {
                entry.insert(Owner { slot, generation });
                true
            }
            std::collections::hash_map::Entry::Occupied(_) => false,
        }
    }

    /// Освобождает все lease, фактически принадлежащие закрытому session owner-у.
    pub(super) fn release(&mut self, slot: usize, generation: u64) {
        let owner = Owner { slot, generation };
        self.owners.retain(|_, value| *value != owner);
    }

    /// Session generation уникальна в одном reactor, поэтому close не нуждается
    /// в уже освобождённом slot для безопасного снятия его lease.
    pub(super) fn release_generation(&mut self, generation: u64) {
        self.owners
            .retain(|_, owner| owner.generation != generation);
    }

    /// Позволяет completion отличить свой successful auth от занятого maildrop.
    pub(super) fn owns(&self, account: &str, slot: usize, generation: u64) -> bool {
        self.owners.get(account) == Some(&Owner { slot, generation })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reuses_account_only_after_actual_owner_releases_lease() {
        let mut leases = MaildropLeases::default();
        assert!(leases.acquire("pop".into(), 3, 7));
        assert!(!leases.acquire("pop".into(), 4, 1));
        leases.release(4, 1);
        assert!(!leases.acquire("pop".into(), 4, 2));
        leases.release(3, 7);
        assert!(leases.acquire("pop".into(), 4, 2));
    }
}
