//! Формирование однострочных и многострочных ответов LIST/UIDL.

use super::Reply;

/// Сериализует LIST: аргумент означает одиночный ответ +OK, без него нужен frame.
pub(super) fn list_reply(number: Option<u32>, entries: Vec<(u32, usize)>) -> Reply {
    match (number, entries.as_slice()) {
        (Some(_), [(message, size)]) => Reply::Ok(format!("{message} {size}")),
        (Some(_), _) => Reply::Error("no such message"),
        (None, _) => list_multiline(entries),
    }
}

/// Сериализует UIDL согласно той же arity-зависимой форме RFC 1939.
pub(super) fn uidl_reply(number: Option<u32>, entries: Vec<(u32, String)>) -> Reply {
    match (number, entries.as_slice()) {
        (Some(_), [(message, uid)]) => Reply::Ok(format!("{message} {uid}")),
        (Some(_), _) => Reply::Error("no such message"),
        (None, _) => uidl_multiline(entries),
    }
}

/// Сохраняет scan listing для LIST без номера, включая пустой maildrop.
fn list_multiline(entries: Vec<(u32, usize)>) -> Reply {
    let mut output = b"+OK scan listing follows\r\n".to_vec();
    for (number, size) in entries {
        output.extend_from_slice(format!("{number} {size}\r\n").as_bytes());
    }
    output.extend_from_slice(b".\r\n");
    Reply::Multiline(output)
}

/// Сохраняет unique-id listing для UIDL без номера.
fn uidl_multiline(entries: Vec<(u32, String)>) -> Reply {
    let mut output = b"+OK unique-id listing follows\r\n".to_vec();
    for (number, uid) in entries {
        output.extend_from_slice(format!("{number} {uid}\r\n").as_bytes());
    }
    output.extend_from_slice(b".\r\n");
    Reply::Multiline(output)
}
