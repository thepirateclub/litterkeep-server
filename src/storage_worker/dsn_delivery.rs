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
    report_key: &crate::queue::DsnReportKey,
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
    store.append_dsn_report_once(&mailbox, &report, report_key)
}

/// Публикует один committed local SUCCESS intent через Store owner.
pub(super) fn append_success(
    store: &mut Store,
    accounts: &Accounts,
    key: crate::queue::DsnSuccessKey,
) -> io::Result<Option<u64>> {
    let report_key = crate::queue::DsnSuccessReportKey {
        source_transaction_id: key.transaction_id,
        recipient_index: key.recipient_index,
    };
    if let Some(transaction_id) = store.dsn_success_report_transaction(&report_key) {
        return Ok(Some(transaction_id));
    }
    let context = store.dsn_success_context(key)?;
    let target = match (
        local_route::local_mailbox(&context.reverse_path),
        context.reverse_path.is_empty(),
    ) {
        (Some(mailbox), _) if accounts.exists(&mailbox) => {
            crate::dsn_target::DsnSuccessTarget::LocalMailbox(mailbox)
        }
        (Some(_), _) | (None, true) => return Ok(None),
        (None, false) => {
            crate::dsn_target::DsnSuccessTarget::OutboundAddress(context.reverse_path.clone())
        }
    };
    let Some(report) = dsn::render_delivery_success(
        &context.reverse_path,
        &context.recipient,
        key.transaction_id,
        &context.original,
        Some(&context.metadata),
    ) else {
        return Ok(None);
    };
    store
        .append_dsn_success_report_once(target, &report, report_key)
        .map(Some)
}

/// Обрабатывает ограниченную пачку durable pending intents.
pub(super) fn append_pending_successes(
    store: &mut Store,
    accounts: &Accounts,
) -> io::Result<usize> {
    let notices = store.pending_dsn_success_notices_limited(100);
    let mut published = 0_usize;
    for key in notices {
        if append_success(store, accounts, key)?.is_some() {
            published = published.saturating_add(1);
        }
    }
    Ok(published)
}
