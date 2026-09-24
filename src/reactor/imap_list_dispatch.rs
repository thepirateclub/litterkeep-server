//! Маршрутизация вариантов LIST в быстрый и snapshot-пути reactor-а.

use super::*;

/// Контекст владельца reactor-а для очереди единственного LIST snapshot запроса.
pub(super) struct Dispatch<'a> {
    slot: usize,
    generation: u64,
    worker: &'a mut WakingStorageWorker,
    owners: &'a mut HashMap<u64, (usize, u64, Pending)>,
    request_id: &'a mut u64,
}

impl<'a> Dispatch<'a> {
    /// Создаёт краткоживущий адаптер над владельцами текущего reactor-вызова.
    pub(super) fn new(
        slot: usize,
        generation: u64,
        worker: &'a mut WakingStorageWorker,
        owners: &'a mut HashMap<u64, (usize, u64, Pending)>,
        request_id: &'a mut u64,
    ) -> Self {
        Self {
            slot,
            generation,
            worker,
            owners,
            request_id,
        }
    }
}

impl Dispatch<'_> {
    /// Использует быстрый каталог для обычного LIST без дополнительных данных.
    pub(super) fn basic(
        &mut self,
        session: &mut Session,
        tag: String,
        pattern: imap::folder::ListPattern,
        returns: imap::folder::ListReturnOptions,
    ) -> io::Result<()> {
        match returns.needs_catalog_projection() {
            true => self.snapshot_list(
                session,
                tag,
                pattern,
                imap::folder::ListSelection::default(),
                returns,
            ),
            false => imap_folders::list(
                session,
                tag,
                pattern,
                false,
                self.slot,
                self.generation,
                self.worker,
                self.owners,
                self.request_id,
            ),
        }
    }

    /// Передаёт extended selection и RETURN через один snapshot каталога.
    pub(super) fn selected(
        &mut self,
        session: &mut Session,
        tag: String,
        pattern: imap::folder::ListPattern,
        selection: imap::folder::ListSelection,
        returns: imap::folder::ListReturnOptions,
    ) -> io::Result<()> {
        self.snapshot_list(session, tag, pattern, selection, returns)
    }

    /// Сохраняет или снимает durable-подписку для указанной папки.
    pub(super) fn subscription(
        &mut self,
        session: &mut Session,
        tag: String,
        mailbox: imap::MailboxName,
        subscribed: bool,
    ) -> io::Result<()> {
        imap_subscriptions::set(
            session,
            tag,
            mailbox,
            subscribed,
            self.slot,
            self.generation,
            self.worker,
            self.owners,
            self.request_id,
        )
    }

    /// Выполняет legacy LSUB, сохраняя отдельный формат wire-ответа.
    pub(super) fn lsub(
        &mut self,
        session: &mut Session,
        tag: String,
        pattern: imap::folder::ListPattern,
    ) -> io::Result<()> {
        imap_subscriptions::list(
            session,
            tag,
            pattern,
            self.slot,
            self.generation,
            self.worker,
            self.owners,
            self.request_id,
            SubscriptionResponse::Lsub,
        )
    }

    /// Очередит единый storage snapshot для вычисления атрибутов LIST.
    fn snapshot_list(
        &mut self,
        session: &mut Session,
        tag: String,
        pattern: imap::folder::ListPattern,
        selection: imap::folder::ListSelection,
        returns: imap::folder::ListReturnOptions,
    ) -> io::Result<()> {
        imap_subscriptions::list(
            session,
            tag,
            pattern,
            self.slot,
            self.generation,
            self.worker,
            self.owners,
            self.request_id,
            SubscriptionResponse::List { selection, returns },
        )
    }
}
