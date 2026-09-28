//! Bounded передача команд единственному Store-владельцу.

use super::StorageWorker;
use crate::storage_worker::Request;

/// Ошибка сохраняет полный request, если очередь временно заполнена.
#[derive(Debug)]
pub enum SubmitError {
    Full(Box<Request>),
    Stopped,
}

impl StorageWorker {
    /// При полном канале возвращает исходное задание для reactor retry.
    pub fn submit(&self, request: Request) -> Result<(), SubmitError> {
        self.requests
            .try_send(request)
            .map_err(|error| match error {
                std::sync::mpsc::TrySendError::Full(request) => {
                    SubmitError::Full(Box::new(request))
                }
                std::sync::mpsc::TrySendError::Disconnected(_) => SubmitError::Stopped,
            })
    }
}
