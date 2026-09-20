//! Извлечение typed request ID для реестра ожиданий reactor-а.

use super::*;

/// Все варианты bounded worker-запроса обязаны иметь уникальную корреляцию.
pub(super) fn request_id(request: &Request) -> u64 {
    match request {
        Request::Authenticate { request_id, .. }
        | Request::ValidateRecipient { request_id, .. }
        | Request::Append { request_id, .. }
        | Request::AppendMany { request_id, .. }
        | Request::Read { request_id, .. }
        | Request::UpdateFlags { request_id, .. }
        | Request::CreateFolder { request_id, .. }
        | Request::RenameFolder { request_id, .. }
        | Request::DeleteFolder { request_id, .. }
        | Request::ListFolders { request_id, .. }
        | Request::ResolveFolder { request_id, .. }
        | Request::ReadFolder { request_id, .. }
        | Request::SelectFolder { request_id, .. }
        | Request::SetSubscription { request_id, .. }
        | Request::ListSubscriptions { request_id, .. }
        | Request::Expunge { request_id, .. }
        | Request::Copy { request_id, .. }
        | Request::Move { request_id, .. }
        | Request::SievePut { request_id, .. }
        | Request::SieveList { request_id, .. }
        | Request::SieveSetActive { request_id, .. }
        | Request::SieveGet { request_id, .. }
        | Request::SieveDelete { request_id, .. }
        | Request::SieveRename { request_id, .. }
        | Request::SieveHaveSpace { request_id, .. }
        | Request::SieveCheck { request_id, .. }
        | Request::AppendDsn { request_id, .. }
        | Request::OutboundPending { request_id }
        | Request::OutboundBlob { request_id, .. }
        | Request::OutboundTransition { request_id, .. } => *request_id,
    }
}
