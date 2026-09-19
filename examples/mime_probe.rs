//! Выводит результаты нашего MIME decoder для независимого blackbox-сравнения.

use litterkeep_server::mime::{encoded_word, parameters};
use std::process::ExitCode;

fn main() -> ExitCode {
    let subject = match encoded_word::decode("=?UTF-8?B?0J/RgNC40LLQtdGC?=") {
        Ok(subject) => subject,
        Err(error) => return fail(error),
    };
    let disposition = "attachment; filename*0*=UTF-8''%D1%81%D1%87; filename*1*=%D1%91%D1%82.txt";
    let structured = match parameters::parse(disposition) {
        Ok(structured) => structured,
        Err(error) => return fail(error),
    };
    let Some(filename) = structured.parameters.get("filename") else {
        return fail("filename is absent");
    };
    println!("{subject}");
    println!("{filename}");
    ExitCode::SUCCESS
}

fn fail(error: &str) -> ExitCode {
    eprintln!("MIME probe error: {error}");
    ExitCode::FAILURE
}
