//! Собственный ограниченный язык Sieve для планирования локальной доставки.
//!
//! Модуль является только фасадом языка. Каждый этап, от лексера до плана,
//! расположен в отдельном файле, чтобы сетевой ManageSieve и storage owner
//! могли использовать ровно один проверяемый контракт.

#[path = "sieve/address.rs"]
mod address;
#[path = "sieve/address_options.rs"]
mod address_options;
#[path = "sieve/ast.rs"]
mod ast;
#[path = "sieve/auto_submitted.rs"]
mod auto_submitted;
#[cfg(test)]
#[path = "sieve/auto_submitted_tests.rs"]
mod auto_submitted_tests;
#[cfg(test)]
#[path = "sieve/copy_tests.rs"]
mod copy_tests;
#[cfg(test)]
#[path = "sieve/envelope_tests.rs"]
mod envelope_tests;
#[path = "sieve/execution.rs"]
mod execution;
#[path = "sieve/lexer.rs"]
mod lexer;
#[path = "sieve/matching.rs"]
mod matching;
#[path = "sieve/parser.rs"]
mod parser;
#[cfg(test)]
#[path = "sieve/parser_tests.rs"]
mod parser_tests;
#[path = "sieve/parser_values.rs"]
mod parser_values;
#[cfg(test)]
#[path = "sieve/tests.rs"]
mod tests;
#[path = "sieve/vacation_identity.rs"]
mod vacation_identity;
#[cfg(test)]
#[path = "sieve/vacation_interval_tests.rs"]
mod vacation_interval_tests;
#[path = "sieve/vacation_message.rs"]
mod vacation_message;
#[path = "sieve/vacation_mime.rs"]
mod vacation_mime;
#[path = "sieve/vacation_policy.rs"]
mod vacation_policy;
#[cfg(test)]
#[path = "sieve/vacation_tests.rs"]
mod vacation_tests;
#[path = "sieve/validate.rs"]
mod validate;

pub use ast::{
    Action, AddressPart, Comparator, Condition, EnvelopePart, MatchType, Script, SizeRelation,
    StringMatch, VacationAction, VacationInterval,
};
pub use execution::{DeliveryPlan, ExecutionError, execute, execute_with_envelope};

/// Разбирает и валидирует script до его сохранения или исполнения.
pub fn parse(source: &str) -> Result<Script, &'static str> {
    let tokens = lexer::tokenize(source)?;
    let script = parser::parse(tokens)?;
    validate::script(&script)?;
    Ok(script)
}

pub(crate) use vacation_identity::response_identity;
pub(crate) use vacation_message::build_response as build_vacation_response;
pub(crate) use vacation_policy::{effective_seconds, may_respond, personal_message};
