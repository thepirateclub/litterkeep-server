//! Регрессии wire-framing для literal имён RENAMESCRIPT.

use super::*;

#[test]
fn literal_old_name_accepts_quoted_new_name() {
    let Start::Begin { mut receiver, sync } = start("{3+}") else {
        panic!("literal old name was not recognized");
    };
    assert!(!sync);
    let mut input = b"old \"new\"\r\nNOOP\r\n".to_vec();
    assert!(matches!(
        receiver.consume(&mut input),
        Progress::Complete(Command::RenameScript { old, new })
            if old == "old" && new == "new"
    ));
    assert_eq!(input, b"NOOP\r\n");
}

#[test]
fn literal_names_preserve_two_sync_continuations() {
    let Start::Begin { mut receiver, sync } = start("{3}") else {
        panic!("sync old name was not recognized");
    };
    assert!(sync);
    let mut old_and_new_marker = b"old {3}\r\n".to_vec();
    assert!(matches!(
        receiver.consume(&mut old_and_new_marker),
        Progress::Continue
    ));
    let mut new = b"newNOOP\r\n".to_vec();
    assert!(matches!(
        receiver.consume(&mut new),
        Progress::Complete(Command::RenameScript { old, new: name })
            if old == "old" && name == "new"
    ));
    assert_eq!(new, b"NOOP\r\n");
}

#[test]
fn quoted_old_name_accepts_non_sync_new_literal() {
    let Start::Begin { mut receiver, sync } = start("\"old\" {3+}") else {
        panic!("new-name literal was not recognized");
    };
    assert!(!sync);
    let mut input = b"newNOOP\r\n".to_vec();
    assert!(matches!(
        receiver.consume(&mut input),
        Progress::Complete(Command::RenameScript { old, new })
            if old == "old" && new == "new"
    ));
    assert_eq!(input, b"NOOP\r\n");
}

#[test]
fn empty_quoted_old_name_still_enters_new_name_literal_phase() {
    let Start::Begin { mut receiver, sync } = start("\"\" {3+}") else {
        panic!("empty old name with literal new name was not recognized");
    };
    assert!(!sync);
    let mut input = b"new".to_vec();
    assert!(matches!(
        receiver.consume(&mut input),
        Progress::Complete(Command::RenameScript { old, new })
            if old.is_empty() && new == "new"
    ));
}
