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
#[path = "sieve/execution.rs"]
mod execution;
#[path = "sieve/lexer.rs"]
mod lexer;
#[path = "sieve/matching.rs"]
mod matching;
#[path = "sieve/parser.rs"]
mod parser;
#[path = "sieve/parser_values.rs"]
mod parser_values;
#[cfg(test)]
#[path = "sieve/tests.rs"]
mod tests;
#[path = "sieve/validate.rs"]
mod validate;

pub use ast::{
    Action, AddressPart, Comparator, Condition, MatchType, Script, SizeRelation, StringMatch,
};
pub use execution::{DeliveryPlan, ExecutionError, execute};

/// Разбирает и валидирует script до его сохранения или исполнения.
pub fn parse(source: &str) -> Result<Script, &'static str> {
    let tokens = lexer::tokenize(source)?;
    let script = parser::parse(tokens)?;
    validate::script(&script)?;
    Ok(script)
}
