use super::{ResourceId, TaskId};
/// Sole, non-cloneable ownership of a checked-out resource.
#[derive(Debug)]
pub struct ResourceLease<T> {
    pub(super) resource: ResourceId,
    pub(super) owner: TaskId,
    pub(super) value: Option<T>,
}
impl<T> ResourceLease<T> {
    #[must_use]
    pub fn resource_id(&self) -> ResourceId {
        self.resource
    }
    #[must_use]
    pub fn owner(&self) -> TaskId {
        self.owner
    }
    pub fn get_mut(&mut self) -> &mut T {
        self.value.as_mut().expect("live lease has a value")
    }

    /// Даёт reactor-у read-only доступ без копирования и второго владельца.
    #[must_use]
    pub fn get(&self) -> &T {
        self.value.as_ref().expect("live lease has a value")
    }
}
