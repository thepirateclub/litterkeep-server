//! Извлечение request id из completion без привязки к protocol wire-слою.

use super::*;

/// Сохраняет один полный mapping всех ответов worker-а к session owner-у.
pub(super) fn response_id(response: &Response) -> u64 {
    match response {
        Response::Authenticated { request_id, .. }
        | Response::RecipientValidated { request_id, .. }
        | Response::Appended { request_id, .. }
        | Response::Read { request_id, .. }
        | Response::FlagsUpdated { request_id, .. }
        | Response::FolderCreated { request_id, .. }
        | Response::FolderMutated { request_id, .. }
        | Response::Folders { request_id, .. }
        | Response::FolderResolved { request_id, .. }
        | Response::FolderRead { request_id, .. }
        | Response::FolderSelected { request_id, .. }
        | Response::SubscriptionSet { request_id, .. }
        | Response::Subscriptions { request_id, .. }
        | Response::Expunged { request_id, .. }
        | Response::Copied { request_id, .. }
        | Response::Moved { request_id, .. } => *request_id,
    }
}
