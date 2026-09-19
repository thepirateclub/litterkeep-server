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
    let multipart = b"Content-Type: multipart/mixed; boundary=x\r\n\r\npreamble\r\n--x\r\nContent-Type: text/plain\r\n\r\ninside --x text\r\n--x--\r\nepilogue";
    let tree = match litterkeep_server::mime::tree::parse(multipart) {
        Ok(tree) => tree,
        Err(error) => return fail(error),
    };
    let Some(first) = tree.children.first() else {
        return fail("multipart child is absent");
    };
    let Some(body) = multipart.get(first.body_range.clone()) else {
        return fail("multipart body range is invalid");
    };
    println!("{}", tree.children.len());
    println!("{}", String::from_utf8_lossy(body));
    let composite = match encoded_word::decode_header(
        "Re: =?UTF-8?B?0J/RgNC40LLQtdGC?= =?ISO-8859-1?Q?_caf=E9?=",
    ) {
        Ok(value) => value,
        Err(error) => return fail(error),
    };
    println!("{composite}");
    let nested_raw = b"Content-Type: message/rfc822\r\n\r\nSubject: nested\r\nContent-Type: text/plain\r\n\r\ninner body";
    let nested_tree = match litterkeep_server::mime::tree::parse(nested_raw) {
        Ok(tree) => tree,
        Err(error) => return fail(error),
    };
    let Some(nested) = nested_tree.children.first() else {
        return fail("message/rfc822 child is absent");
    };
    let Some(nested_body) = nested_raw.get(nested.body_range.clone()) else {
        return fail("nested body range is invalid");
    };
    println!("{}", String::from_utf8_lossy(nested_body));
    ExitCode::SUCCESS
}

fn fail(error: &str) -> ExitCode {
    eprintln!("MIME probe error: {error}");
    ExitCode::FAILURE
}
