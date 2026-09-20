//! Парсинг и рендеринг малого local control protocol.

use crate::runtime::{Orchestrator, ResourceSnapshot, TaskId, TaskSnapshot};

/// Команды администратора. Строки внешнего wire-format сразу переводятся в enum.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Command {
    Stats,
    Tasks,
    Resources,
    Inspect(TaskId),
}

impl Command {
    /// Принимает только один короткий control запрос без аргументов-секретов.
    pub fn parse(line: &str) -> Result<Self, &'static str> {
        let mut words = line.split_ascii_whitespace();
        let Some(first) = words.next() else {
            return Err("empty command");
        };
        match (first, words.next(), words.next()) {
            ("stats", None, None) => Ok(Self::Stats),
            ("tasks", None, None) => Ok(Self::Tasks),
            ("resources", None, None) => Ok(Self::Resources),
            ("inspect", Some(value), None) => TaskId::parse(value)
                .map(Self::Inspect)
                .ok_or("inspect expects task-id slot:generation"),
            _ => Err("expected stats, tasks, resources, or inspect <task-id>"),
        }
    }
}

/// Формирует line-oriented ответ, пригодный и для CLI, и для ручной диагностики.
pub(super) fn response<T>(lifecycle: &Orchestrator<T>, request: Command) -> String {
    match request {
        Command::Stats => stats(lifecycle),
        Command::Tasks => tasks(lifecycle.tasks()),
        Command::Resources => resources(lifecycle.resources()),
        Command::Inspect(id) => match lifecycle.inspect_task(id) {
            Some(task) => task_line(&task),
            None => format!("NOT_FOUND task={id}\n"),
        },
    }
}

fn stats<T>(lifecycle: &Orchestrator<T>) -> String {
    let value = lifecycle.stats();
    format!(
        "STATS ready_tasks={} waiting_tasks={} cancelling_tasks={} completed_tasks={} failed_tasks={} available_resources={} leased_resources={} quarantined_resources={} closed_resources={} ready_queue_depth={} wait_registrations={}\n",
        value.ready_tasks,
        value.waiting_tasks,
        value.cancelling_tasks,
        value.completed_tasks,
        value.failed_tasks,
        value.available_resources,
        value.leased_resources,
        value.quarantined_resources,
        value.closed_resources,
        value.ready_queue_depth,
        value.wait_registrations,
    )
}

fn tasks(values: Vec<TaskSnapshot>) -> String {
    let mut response = String::new();
    for task in values {
        response.push_str(&task_line(&task));
    }
    response
}

fn resources(values: Vec<ResourceSnapshot>) -> String {
    let mut response = String::new();
    for resource in values {
        let owner = resource
            .owner
            .map_or_else(|| "none".into(), |id| id.to_string());
        response.push_str(&format!(
            "RESOURCE id={} state={:?} owner={owner}\n",
            resource.id, resource.state
        ));
    }
    response
}

fn task_line(task: &TaskSnapshot) -> String {
    let held = task
        .held_resources
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(",");
    format!("TASK id={} state={:?} held={held}\n", task.id, task.state)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commands_become_typed_variants() {
        assert_eq!(Command::parse("stats"), Ok(Command::Stats));
        assert_eq!(
            Command::parse("inspect 4:2"),
            Ok(Command::Inspect(TaskId::parse("4:2").expect("id")))
        );
        assert!(Command::parse("inspect bad").is_err());
    }

    #[test]
    fn snapshot_does_not_render_resource_value() {
        let mut runtime = Orchestrator::new();
        let task = runtime.spawn_task();
        let resource = runtime.register_resource("secret message bytes");
        let _lease = runtime.acquire(task, resource).expect("lease");
        let response = super::response(&runtime, Command::Resources);
        assert!(response.contains("RESOURCE"));
        assert!(!response.contains("secret message bytes"));
    }
}
