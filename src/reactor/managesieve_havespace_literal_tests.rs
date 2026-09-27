//! Проверки framing literal-имени HAVESPACE.

use super::*;

#[test]
fn non_sync_literal_name_parses_size_and_keeps_pipeline() {
    let Start::Begin { mut receiver, sync } = start("{3+}") else {
        panic!("literal name was not recognized");
    };
    assert!(!sync);
    let mut input = b"box 435\r\nNOOP\r\n".to_vec();
    assert!(matches!(
        receiver.consume(&mut input),
        Progress::Complete(Command::HaveSpace { name, size })
            if name == "box" && size == 435
    ));
    assert_eq!(input, b"NOOP\r\n");
}

#[test]
fn sync_literal_accepts_split_name_and_numeric_tail() {
    let Start::Begin { mut receiver, sync } = start("{3}") else {
        panic!("sync literal name was not recognized");
    };
    assert!(sync);
    let mut name = b"bo".to_vec();
    assert!(matches!(receiver.consume(&mut name), Progress::Waiting));
    name.extend_from_slice(b"x ");
    assert!(matches!(receiver.consume(&mut name), Progress::Waiting));
    let mut number = b"435\r\n".to_vec();
    assert!(matches!(
        receiver.consume(&mut number),
        Progress::Complete(Command::HaveSpace { name, size })
            if name == "box" && size == 435
    ));
}

#[test]
fn numeric_tail_rejects_extra_arguments_and_non_decimal_size() {
    for tail in [b"box 4 extra\r\n".as_slice(), b"box +4\r\n".as_slice()] {
        let Start::Begin { mut receiver, .. } = start("{3+}") else {
            panic!("literal name was not recognized");
        };
        let mut input = tail.to_vec();
        assert!(matches!(receiver.consume(&mut input), Progress::Invalid));
    }
}
