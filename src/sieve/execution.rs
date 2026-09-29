//! Bounded исполнение AST в DeliveryPlan без доступа к сети и файловой системе.

use super::ast::{Action, Command, Script, VacationAction};
use super::variables::{VariableError, VariableStore};
#[path = "execution_condition.rs"]
mod condition;
use std::fmt;

/// Максимум redirect действий за одну оценку Sieve script.
pub const MAX_REDIRECTS_PER_EVALUATION: usize = 16;

/// Единственный результат Sieve, который delivery transaction может применить.
#[derive(Debug, Eq, PartialEq)]
pub struct DeliveryPlan {
    pub keep: bool,
    pub discard: bool,
    pub folders: Vec<String>,
    pub redirects: Vec<String>,
    /// Generated-response intents, сохраняющие независимость executor-а от Store.
    pub vacations: Vec<VacationAction>,
}

/// Типизированная причина отказа bounded executor-а после успешной валидации.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExecutionError {
    CommandLimit,
    MatcherLimit,
    RedirectLimit,
    RedirectRouteDenied,
    RedirectLoop,
    VariableRuntime,
}

impl ExecutionError {
    /// Стабильный код для долговечного журнала пользовательской диагностики.
    pub fn code(self) -> &'static str {
        match self {
            Self::CommandLimit => "command-limit",
            Self::MatcherLimit => "matcher-work-limit",
            Self::RedirectLimit => "redirect-limit",
            Self::RedirectRouteDenied => "redirect-route-denied",
            Self::RedirectLoop => "redirect-loop",
            Self::VariableRuntime => "variables-runtime-error",
        }
    }

    /// Разбирает только версионированные коды на границе дискового формата.
    pub fn from_code(value: &str) -> Option<Self> {
        match value {
            "command-limit" => Some(Self::CommandLimit),
            "matcher-work-limit" => Some(Self::MatcherLimit),
            "redirect-limit" => Some(Self::RedirectLimit),
            "redirect-route-denied" => Some(Self::RedirectRouteDenied),
            "redirect-loop" => Some(Self::RedirectLoop),
            "variables-runtime-error" => Some(Self::VariableRuntime),
            _ => None,
        }
    }
}

impl fmt::Display for ExecutionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::CommandLimit => "sieve execution limit exceeded",
            Self::MatcherLimit => "sieve matcher work limit exceeded",
            Self::RedirectLimit => "sieve redirect limit exceeded",
            Self::RedirectRouteDenied => "sieve redirect route denied",
            Self::RedirectLoop => "sieve redirect hop limit exceeded",
            Self::VariableRuntime => "sieve variables runtime error",
        })
    }
}

impl std::error::Error for ExecutionError {}

/// Отличает неявный keep от явно запрошенного действия с тем же результатом.
enum KeepState {
    Implicit,
    Cancelled,
    Explicit,
}

/// Владеет изменяемым состоянием одного script execution.
struct ExecutionState {
    plan: DeliveryPlan,
    stopped: bool,
    remaining: usize,
    keep_state: KeepState,
    match_variables: condition::MatchVariables,
    variables: VariableStore,
}

/// Запускает script над заголовками и исходным размером с fixed budget команд.
pub fn execute(
    script: &Script,
    headers: &[(String, String)],
    message_size: usize,
) -> Result<DeliveryPlan, ExecutionError> {
    execute_with_envelope(script, headers, message_size, "", "")
}

/// Исполняет правило с SMTP MAIL FROM и конкретным RCPT TO текущей доставки.
pub fn execute_with_envelope(
    script: &Script,
    headers: &[(String, String)],
    message_size: usize,
    envelope_from: &str,
    envelope_to: &str,
) -> Result<DeliveryPlan, ExecutionError> {
    execute_state(script, headers, message_size, envelope_from, envelope_to).map(|(plan, _)| plan)
}

/// Исполняет script и возвращает приватное состояние для integration tests.
fn execute_state(
    script: &Script,
    headers: &[(String, String)],
    message_size: usize,
    envelope_from: &str,
    envelope_to: &str,
) -> Result<(DeliveryPlan, VariableStore), ExecutionError> {
    let mut state = ExecutionState {
        plan: DeliveryPlan {
            keep: true,
            discard: false,
            folders: Vec::new(),
            redirects: Vec::new(),
            vacations: Vec::new(),
        },
        stopped: false,
        remaining: 128,
        keep_state: KeepState::Implicit,
        match_variables: condition::MatchVariables::default(),
        variables: VariableStore::default(),
    };
    let context = condition::MessageContext {
        headers,
        size: message_size,
        envelope_from,
        envelope_to,
    };
    commands(&script.commands, &context, &mut state)?;
    Ok((state.plan, state.variables))
}

fn commands(
    items: &[Command],
    context: &condition::MessageContext<'_>,
    state: &mut ExecutionState,
) -> Result<(), ExecutionError> {
    for command in items {
        if state.stopped {
            return Ok(());
        }
        match state.remaining.checked_sub(1) {
            Some(value) => state.remaining = value,
            None => return Err(ExecutionError::CommandLimit),
        }
        match command {
            Command::Action(action) => apply(action, state)?,
            Command::If {
                condition,
                then_commands,
                else_commands,
            } => {
                let selected = match condition::matches(
                    condition,
                    context,
                    &state.variables,
                    &mut state.match_variables,
                )? {
                    true => then_commands,
                    false => else_commands,
                };
                commands(selected, context, state)?;
            }
        }
    }
    Ok(())
}

fn apply(action: &Action, state: &mut ExecutionState) -> Result<(), ExecutionError> {
    match action {
        Action::Keep => {
            state.plan.keep = true;
            state.keep_state = KeepState::Explicit;
        }
        Action::Discard => {
            cancel_implicit_keep(state);
            state.plan.discard = true;
        }
        Action::FileInto(folder) => {
            cancel_implicit_keep(state);
            state.plan.folders.push(folder.clone());
        }
        Action::FileIntoCopy(folder) => state.plan.folders.push(folder.clone()),
        Action::Redirect(address) => {
            if state.plan.redirects.len() >= MAX_REDIRECTS_PER_EVALUATION {
                return Err(ExecutionError::RedirectLimit);
            }
            cancel_implicit_keep(state);
            state.plan.redirects.push(address.clone());
        }
        Action::RedirectCopy(address) => {
            if state.plan.redirects.len() >= MAX_REDIRECTS_PER_EVALUATION {
                return Err(ExecutionError::RedirectLimit);
            }
            state.plan.redirects.push(address.clone());
        }
        Action::Vacation(action) => state.plan.vacations.push(action.clone()),
        Action::Set(action) => {
            let captures = state.match_variables.octets();
            state
                .variables
                .set_expanded_octets(&action.name, &action.value, captures, &action.modifiers)
                .map_err(variable_error)?;
        }
        Action::Stop => state.stopped = true,
    }
    Ok(())
}

fn variable_error(_error: VariableError) -> ExecutionError {
    ExecutionError::VariableRuntime
}

/// Delivery action отменяет implicit keep, но не ранее выполненный explicit keep.
fn cancel_implicit_keep(state: &mut ExecutionState) {
    if let KeepState::Implicit = state.keep_state {
        state.plan.keep = false;
        state.keep_state = KeepState::Cancelled;
    }
}

#[cfg(test)]
#[path = "execution_work_limit_tests.rs"]
mod work_limit_tests;

#[cfg(test)]
#[path = "execution_variables_tests.rs"]
mod variables_tests;
