//! Проекция extended LIST selection поверх одного согласованного Store snapshot.

use super::*;
use std::collections::{BTreeMap, HashSet};

#[derive(Clone)]
/// Единая запись mailbox из каталога и набора подписок аккаунта.
struct Entry {
    name: String,
    exists: bool,
    subscribed: bool,
    special_use: Option<crate::store::SpecialUse>,
    /// Рассчитывается только по реально существующим папкам каталога.
    has_children: bool,
}

/// Результат отбора с признаком необходимости CHILDINFO для потомков.
struct ResultEntry {
    entry: Entry,
    child_info: bool,
}

/// Формирует RFC LIST ответы и CHILDINFO без повторного чтения Store.
pub(super) fn listed(
    session: &mut Session,
    tag: String,
    pattern: imap::folder::ListPattern,
    selection: imap::folder::ListSelection,
    returns: imap::folder::ListReturnOptions,
    snapshot: crate::store::SubscriptionSnapshot,
) -> io::Result<()> {
    let entries = entries(snapshot);
    let results = project(&entries, &pattern, selection);
    for result in results.values() {
        write_entry(session, result, selection, returns)?;
    }
    session.push(format!("{tag} OK LIST completed\r\n").as_bytes())
}

/// Сливает каталог и подписки, сохраняя отсутствующие mailbox отдельными записями.
fn entries(snapshot: crate::store::SubscriptionSnapshot) -> BTreeMap<String, Entry> {
    let subscribed_names = snapshot
        .subscriptions
        .iter()
        .map(|folder| key(&folder.name))
        .collect::<HashSet<_>>();
    let mut entries = BTreeMap::new();
    let child_parents = snapshot
        .folders
        .iter()
        .filter_map(|folder| folder.name.rsplit_once('/').map(|(parent, _)| key(parent)))
        .collect::<HashSet<_>>();
    for folder in snapshot.folders {
        let folder_key = key(&folder.name);
        entries.insert(
            folder_key.clone(),
            Entry {
                name: folder.name,
                exists: true,
                subscribed: subscribed_names.contains(&folder_key),
                special_use: folder.special_use,
                has_children: child_parents.contains(&folder_key),
            },
        );
    }
    for subscription in snapshot.subscriptions {
        let folder_key = key(&subscription.name);
        let has_children = child_parents.contains(&folder_key);
        entries.entry(folder_key).or_insert(Entry {
            name: subscription.name,
            exists: false,
            subscribed: true,
            special_use: subscription.special_use,
            has_children,
        });
    }
    entries
}

/// Выбирает mailbox по LIST pattern и накопительным selection criteria.
fn project(
    entries: &BTreeMap<String, Entry>,
    pattern: &imap::folder::ListPattern,
    selection: imap::folder::ListSelection,
) -> BTreeMap<String, ResultEntry> {
    let mut results = BTreeMap::new();
    for (entry_key, entry) in entries {
        if !selected(entry, selection) {
            continue;
        }
        if !imap_list::matches_pattern(&entry.name, pattern) {
            continue;
        }
        results.insert(
            entry_key.clone(),
            ResultEntry {
                entry: entry.clone(),
                child_info: false,
            },
        );
    }
    if selection.recursive_match {
        add_matching_parents(entries, pattern, selection, &mut results);
    }
    results
}

/// Добавляет подходящих по LIST pattern родителей выбранных потомков.
fn add_matching_parents(
    entries: &BTreeMap<String, Entry>,
    pattern: &imap::folder::ListPattern,
    selection: imap::folder::ListSelection,
    results: &mut BTreeMap<String, ResultEntry>,
) {
    for descendant in entries.values().filter(|entry| selected(entry, selection)) {
        if imap_list::matches_pattern(&descendant.name, pattern) {
            continue;
        }
        add_ancestors(entries, descendant, pattern, results);
    }
}

/// Идёт по иерархии вверх и объединяет повторные имена в одну LIST запись.
fn add_ancestors(
    entries: &BTreeMap<String, Entry>,
    descendant: &Entry,
    pattern: &imap::folder::ListPattern,
    results: &mut BTreeMap<String, ResultEntry>,
) {
    let mut prefix = descendant.name.as_str();
    while let Some((parent, _child)) = prefix.rsplit_once('/') {
        prefix = parent;
        if !imap_list::matches_pattern(parent, pattern) {
            continue;
        }
        let parent_key = key(parent);
        let entry = entries.get(&parent_key).cloned().unwrap_or_else(|| Entry {
            name: parent.to_owned(),
            exists: false,
            subscribed: false,
            special_use: None,
            has_children: false,
        });
        match results.get_mut(&parent_key) {
            Some(result) => result.child_info = true,
            None => {
                results.insert(
                    parent_key,
                    ResultEntry {
                        entry,
                        child_info: true,
                    },
                );
            }
        }
    }
}

/// Проверяет пересечение критериев подписки и special-use назначения.
fn selected(entry: &Entry, selection: imap::folder::ListSelection) -> bool {
    !matches!(
        (
            selection.subscribed,
            entry.subscribed,
            selection.special_use,
            entry.special_use.is_some(),
        ),
        (true, false, _, _) | (_, _, true, false)
    )
}

fn write_entry(
    session: &mut Session,
    result: &ResultEntry,
    selection: imap::folder::ListSelection,
    returns: imap::folder::ListReturnOptions,
) -> io::Result<()> {
    let attributes = attributes(&result.entry, selection, returns);
    let name = imap_folders::quote(&result.entry.name);
    let child_info = if result.child_info {
        child_info(selection)
    } else {
        String::new()
    };
    session.push(format!("* LIST ({attributes}) \"/\" {name}{child_info}\r\n").as_bytes())
}

/// Возвращает независимые атрибуты отсутствия, подписки и special-use.
fn attributes(
    entry: &Entry,
    selection: imap::folder::ListSelection,
    returns: imap::folder::ListReturnOptions,
) -> String {
    let mut values = Vec::new();
    if !entry.exists {
        values.push("\\NonExistent");
    }
    if matches!(
        (entry.subscribed, selection.subscribed || returns.subscribed),
        (true, true)
    ) {
        values.push("\\Subscribed");
    }
    if returns.children {
        let children_attribute = match entry.has_children {
            true => "\\HasChildren",
            false => "\\HasNoChildren",
        };
        values.push(children_attribute);
    }
    if let Some(special_use) = entry.special_use {
        let value = imap_folders::special_use(Some(special_use));
        if !value.is_empty() {
            values.push(value);
        }
    }
    values.join(" ")
}

/// Кодирует причину возврата родителя в формате RFC CHILDINFO.
fn child_info(selection: imap::folder::ListSelection) -> String {
    let mut values = Vec::new();
    if selection.subscribed {
        values.push("SUBSCRIBED");
    }
    if selection.special_use {
        values.push("SPECIAL-USE");
    }
    let quoted_values = values
        .iter()
        .map(|value| format!("\"{value}\""))
        .collect::<Vec<_>>()
        .join(" ");
    format!(" (\"CHILDINFO\" ({quoted_values}))")
}

fn key(name: &str) -> String {
    match name.eq_ignore_ascii_case("INBOX") {
        true => "INBOX".to_owned(),
        false => name.to_owned(),
    }
}
