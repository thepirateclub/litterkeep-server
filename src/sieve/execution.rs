//! Bounded исполнение AST в DeliveryPlan без доступа к сети и файловой системе.

use super::ast::{Action, Command, Condition, Script};

/// Единственный результат Sieve, который delivery transaction может применить.
#[derive(Debug, Eq, PartialEq)]
pub struct DeliveryPlan {
    pub keep: bool,
    pub discard: bool,
    pub folders: Vec<String>,
    pub redirects: Vec<String>,
}

/// Запускает script над разобранными заголовками с фиксированным budget команд.
pub fn execute(
    script: &Script,
    headers: &[(String, String)],
) -> Result<DeliveryPlan, &'static str> {
    let mut plan = DeliveryPlan {
        keep: true,
        discard: false,
        folders: Vec::new(),
        redirects: Vec::new(),
    };
    let mut stopped = false;
    let mut remaining = 128;
    commands(
        &script.commands,
        headers,
        &mut plan,
        &mut stopped,
        &mut remaining,
    )?;
    Ok(plan)
}

fn commands(
    items: &[Command],
    headers: &[(String, String)],
    plan: &mut DeliveryPlan,
    stopped: &mut bool,
    remaining: &mut usize,
) -> Result<(), &'static str> {
    for command in items {
        if *stopped {
            return Ok(());
        }
        match remaining.checked_sub(1) {
            Some(value) => *remaining = value,
            None => return Err("sieve execution limit exceeded"),
        }
        match command {
            Command::Action(action) => apply(action, plan, stopped)?,
            Command::If {
                condition,
                then_commands,
                else_commands,
            } => {
                let selected = match matches(condition, headers) {
                    true => then_commands,
                    false => else_commands,
                };
                commands(selected, headers, plan, stopped, remaining)?;
            }
        }
    }
    Ok(())
}

fn apply(action: &Action, plan: &mut DeliveryPlan, stopped: &mut bool) -> Result<(), &'static str> {
    match action {
        Action::Keep => plan.keep = true,
        Action::Discard => {
            plan.keep = false;
            plan.discard = true;
        }
        Action::FileInto(folder) => {
            plan.keep = false;
            plan.folders.push(folder.clone());
        }
        Action::Redirect(address) => {
            if plan.redirects.len() >= 16 {
                return Err("sieve redirect limit exceeded");
            }
            plan.redirects.push(address.clone());
        }
        Action::Stop => *stopped = true,
    }
    Ok(())
}

fn matches(condition: &Condition, headers: &[(String, String)]) -> bool {
    match condition {
        Condition::True => true,
        Condition::False => false,
        Condition::HeaderContains { name, needle } => headers
            .iter()
            .any(|(candidate, value)| header_contains(candidate, value, name, needle)),
        Condition::Exists { names } => names.iter().all(|name| {
            headers
                .iter()
                .any(|(candidate, _)| candidate.eq_ignore_ascii_case(name))
        }),
        Condition::Not(value) => !matches(value, headers),
        Condition::AllOf(values) => values.iter().all(|value| matches(value, headers)),
        Condition::AnyOf(values) => values.iter().any(|value| matches(value, headers)),
    }
}

/// Сначала проверяет имя header, затем его значение, не смешивая условия.
fn header_contains(candidate: &str, value: &str, name: &str, needle: &str) -> bool {
    if !candidate.eq_ignore_ascii_case(name) {
        return false;
    }
    value.contains(needle)
}
