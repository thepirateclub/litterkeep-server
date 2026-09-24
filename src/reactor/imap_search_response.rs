//! Сериализация legacy SEARCH и базового ESEARCH результата.

use crate::imap::{
    Addressing,
    search_result::{self, ResultRequest, ReturnItem},
};
use std::io;

/// Выбирает legacy SEARCH или correlator-aware ESEARCH по запросу клиента.
pub(in crate::reactor) fn write(
    session: &mut super::Session,
    addressing: Addressing,
    request: ResultRequest,
    tag: &str,
    values: &[u64],
) -> io::Result<()> {
    match request {
        ResultRequest::Legacy => write_legacy(session, tag, values),
        ResultRequest::Extended(items) => {
            match items.iter().any(|item| !matches!(item, ReturnItem::Save)) {
                true => write_extended(session, addressing, tag, items, values),
                false => {
                    session.push(format!("{tag} OK SEARCH completed, result saved\r\n").as_bytes())
                }
            }
        }
    }
}

fn write_legacy(session: &mut super::Session, tag: &str, values: &[u64]) -> io::Result<()> {
    let result = values
        .iter()
        .map(u64::to_string)
        .collect::<Vec<_>>()
        .join(" ");
    session.push(format!("* SEARCH {result}\r\n").as_bytes())?;
    session.push(format!("{tag} OK SEARCH completed\r\n").as_bytes())
}

fn write_extended(
    session: &mut super::Session,
    addressing: Addressing,
    tag: &str,
    items: Vec<ReturnItem>,
    values: &[u64],
) -> io::Result<()> {
    let uid = match addressing {
        Addressing::Sequence => "",
        Addressing::Uid => " UID",
    };
    let mut fields = Vec::new();
    let count_requested = items.contains(&ReturnItem::Count);
    for item in items {
        append_item(&mut fields, item, values);
    }
    let correlator = quoted_tag(tag);
    // RFC 9051 требует COUNT 0 только при запросе COUNT; при пустом наборе
    // остальные запрошенные поля пропускаются, но ESEARCH response остаётся.
    if let (true, true) = (fields.is_empty(), count_requested) {
        fields.push(format!("COUNT {}", values.len()));
    }
    let body = fields.join(" ");
    // Пустой список return-data допустим, поэтому не добавляем лишний пробел.
    let response = match body.is_empty() {
        true => format!("* ESEARCH (TAG {correlator}){uid}\r\n"),
        false => format!("* ESEARCH (TAG {correlator}){uid} {body}\r\n"),
    };
    session.push(response.as_bytes())?;
    session.push(format!("{tag} OK SEARCH completed\r\n").as_bytes())
}

fn quoted_tag(tag: &str) -> String {
    let escaped = tag.replace('\\', "\\\\").replace('"', "\\\"");
    format!("\"{escaped}\"")
}

fn append_item(fields: &mut Vec<String>, item: ReturnItem, values: &[u64]) {
    match item {
        ReturnItem::All if !values.is_empty() => {
            fields.push(format!("ALL {}", search_result::sequence_set(values)))
        }
        ReturnItem::All | ReturnItem::Min | ReturnItem::Max if values.is_empty() => {}
        ReturnItem::Count => fields.push(format!("COUNT {}", values.len())),
        ReturnItem::Min => {
            fields.push(format!("MIN {}", values.iter().min().copied().unwrap_or(0)))
        }
        ReturnItem::Max => {
            fields.push(format!("MAX {}", values.iter().max().copied().unwrap_or(0)))
        }
        ReturnItem::All => {}
        ReturnItem::Save => {}
    }
}
