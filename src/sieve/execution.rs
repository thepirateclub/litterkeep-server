//! Bounded исполнение AST в DeliveryPlan без доступа к сети и файловой системе.

use super::ast::{Action, Command, Script, VacationAction};
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
    let mut plan = DeliveryPlan {
        keep: true,
        discard: false,
        folders: Vec::new(),
        redirects: Vec::new(),
        vacations: Vec::new(),
    };
    let mut stopped = false;
    let mut remaining = 128;
    let mut keep_state = KeepState::Implicit;
    let context = condition::MessageContext {
        headers,
        size: message_size,
        envelope_from,
        envelope_to,
    };
    let mut match_variables = condition::MatchVariables::default();
    commands(
        &script.commands,
        &context,
        &mut match_variables,
        &mut plan,
        &mut stopped,
        &mut remaining,
        &mut keep_state,
    )?;
    Ok(plan)
}

fn commands(
    items: &[Command],
    context: &condition::MessageContext<'_>,
    match_variables: &mut condition::MatchVariables,
    plan: &mut DeliveryPlan,
    stopped: &mut bool,
    remaining: &mut usize,
    keep_state: &mut KeepState,
) -> Result<(), ExecutionError> {
    for command in items {
        if *stopped {
            return Ok(());
        }
        match remaining.checked_sub(1) {
            Some(value) => *remaining = value,
            None => return Err(ExecutionError::CommandLimit),
        }
        match command {
            Command::Action(action) => apply(action, plan, stopped, keep_state)?,
            Command::If {
                condition,
                then_commands,
                else_commands,
            } => {
                let selected = match condition::matches(condition, context, match_variables)? {
                    true => then_commands,
                    false => else_commands,
                };
                commands(
                    selected,
                    context,
                    match_variables,
                    plan,
                    stopped,
                    remaining,
                    keep_state,
                )?;
            }
        }
    }
    Ok(())
}

fn apply(
    action: &Action,
    plan: &mut DeliveryPlan,
    stopped: &mut bool,
    keep_state: &mut KeepState,
) -> Result<(), ExecutionError> {
    match action {
        Action::Keep => {
            plan.keep = true;
            *keep_state = KeepState::Explicit;
        }
        Action::Discard => {
            cancel_implicit_keep(plan, keep_state);
            plan.discard = true;
        }
        Action::FileInto(folder) => {
            cancel_implicit_keep(plan, keep_state);
            plan.folders.push(folder.clone());
        }
        Action::FileIntoCopy(folder) => plan.folders.push(folder.clone()),
        Action::Redirect(address) => {
            if plan.redirects.len() >= MAX_REDIRECTS_PER_EVALUATION {
                return Err(ExecutionError::RedirectLimit);
            }
            cancel_implicit_keep(plan, keep_state);
            plan.redirects.push(address.clone());
        }
        Action::RedirectCopy(address) => {
            if plan.redirects.len() >= MAX_REDIRECTS_PER_EVALUATION {
                return Err(ExecutionError::RedirectLimit);
            }
            plan.redirects.push(address.clone());
        }
        Action::Vacation(action) => plan.vacations.push(action.clone()),
        Action::Stop => *stopped = true,
    }
    Ok(())
}

/// Delivery action отменяет implicit keep, но не ранее выполненный explicit keep.
fn cancel_implicit_keep(plan: &mut DeliveryPlan, keep_state: &mut KeepState) {
    if let KeepState::Implicit = keep_state {
        plan.keep = false;
        *keep_state = KeepState::Cancelled;
    }
}

#[cfg(test)]
#[path = "execution_work_limit_tests.rs"]
mod work_limit_tests;
