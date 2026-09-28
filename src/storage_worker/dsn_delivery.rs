//! Долговечная локальная публикация DSN владельцем Store.

use crate::{accounts::Accounts, dsn, local_route, store::Store};
use std::io;

/// Формирует и публикует DSN через единственного владельца Store.
///
/// Внешний sender и пустой reverse-path не дают локальную запись. Проверка
/// account происходит перед `Store::append`, поэтому DSN не создаёт mailbox
/// для произвольного local-part и не подменяет policy submission reactor-а.
pub(super) fn append(
    store: &mut Store,
    accounts: &Accounts,
    request: dsn::ReportRequest<'_>,
) -> io::Result<Option<u64>> {
    let Some(mailbox) = local_route::local_mailbox(request.reverse_path) else {
        return Ok(None);
    };
    if !accounts.exists(&mailbox) {
        return Ok(None);
    }
    let Some(report) = dsn::render_request(request) else {
        return Ok(None);
    };
    store.append(&mailbox, &report).map(Some)
}
