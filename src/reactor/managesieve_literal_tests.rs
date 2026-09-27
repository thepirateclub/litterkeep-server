//! Проверки bounded literal receiver-а ManageSieve.

use super::*;

/// Любая TCP-граница даёт тот же PUTSCRIPT и не съедает pipeline command.
#[test]
fn preserves_all_two_chunk_boundaries_and_pipeline() {
    let Start::Begin { .. } = start(b"PUTSCRIPT \"box\" {5+}\r\n") else {
        panic!("literal command was not recognized");
    };
    let wire = b"keep;\r\nNOOP\r\n";
    for split in 0..=wire.len() {
        let mut receiver = LiteralReceiver::new(Operation::Put("box".into()), 5);
        let mut input = wire.get(..split).expect("prefix").to_vec();
        let first_progress = receiver.consume(&mut input);
        input.extend_from_slice(wire.get(split..).expect("suffix"));
        let progress = match first_progress {
            Progress::Waiting => receiver.consume(&mut input),
            complete => complete,
        };
        let Progress::Complete(Command::PutScript { name, source }) = progress else {
            panic!("literal did not complete at split {split}");
        };
        assert_eq!(name, "box");
        assert_eq!(source, "keep;");
        assert_eq!(input, b"NOOP\r\n");
    }
}

#[test]
fn recognizes_sync_literal_and_rejects_oversized_body() {
    assert!(matches!(
        start(b"CHECKSCRIPT {4}\r\n"),
        Start::Begin {
            continuation: true,
            ..
        }
    ));
    assert!(matches!(start(b"CHECKSCRIPT {65537+}\r\n"), Start::Invalid));
}

#[test]
fn noop_literal_keeps_sync_mode_and_pipeline_boundary() {
    let Start::Begin {
        mut receiver,
        continuation,
    } = start(b"NOOP {7}\r\n")
    else {
        panic!("synchronizing NOOP literal was not recognized");
    };
    assert!(continuation);
    let mut input = b"sync-42\r\nNOOP\r\n".to_vec();
    let Progress::Complete(Command::Noop(Some(tag))) = receiver.consume(&mut input) else {
        panic!("NOOP literal tag did not complete");
    };
    assert_eq!(tag, "sync-42");
    assert_eq!(input, b"NOOP\r\n");
    assert!(matches!(
        start(b"NOOP {7+}\r\n"),
        Start::Begin {
            continuation: false,
            ..
        }
    ));
}

#[test]
fn final_literal_requires_crlf_before_the_next_command() {
    let Start::Begin { mut receiver, .. } = start(b"NOOP {3+}\r\n") else {
        panic!("NOOP literal was not recognized");
    };
    let mut payload = b"tag\r".to_vec();
    assert!(matches!(receiver.consume(&mut payload), Progress::Waiting));
    let mut terminator_and_pipeline = b"\nNOOP\r\n".to_vec();
    assert!(matches!(
        receiver.consume(&mut terminator_and_pipeline),
        Progress::Complete(Command::Noop(Some(tag))) if tag == "tag"
    ));
    assert_eq!(terminator_and_pipeline, b"NOOP\r\n");
}

#[test]
fn literal_script_names_preserve_command_types_and_empty_deactivation() {
    assert_eq!(
        complete(b"GETSCRIPT {3+}\r\n", b"box\r\n"),
        Command::GetScript("box".into())
    );
    assert_eq!(
        complete(b"DELETESCRIPT {3+}\r\n", b"box\r\n"),
        Command::DeleteScript("box".into())
    );
    assert_eq!(
        complete(b"SETACTIVE {3+}\r\n", b"box\r\n"),
        Command::SetActive(Some("box".into()))
    );
    assert_eq!(
        complete(b"SETACTIVE {0+}\r\n", b"\r\n"),
        Command::SetActive(None)
    );
}

#[test]
fn putscript_literal_name_accepts_quoted_or_literal_body() {
    assert_eq!(
        complete_put(b"PUTSCRIPT {3+}\r\n", b"box \"keep;\"\r\n"),
        Command::PutScript {
            name: "box".into(),
            source: "keep;".into(),
        }
    );
    assert_eq!(
        complete_put(b"PUTSCRIPT {3+}\r\n", b"box {5+}\r\nkeep;\r\n"),
        Command::PutScript {
            name: "box".into(),
            source: "keep;".into(),
        }
    );
    assert_eq!(
        complete_put(b"PUTSCRIPT {3+}\r\n", b"box {5+}\r\nkeep;\r\nNOOP\r\n"),
        Command::PutScript {
            name: "box".into(),
            source: "keep;".into(),
        }
    );
}

#[test]
fn putscript_literal_name_and_body_keep_sync_continuations() {
    let Start::Begin {
        mut receiver,
        continuation,
    } = start(b"PUTSCRIPT {3}\r\n")
    else {
        panic!("synchronizing script-name literal was not recognized");
    };
    assert!(continuation);
    let mut name_and_body = b"box {5}\r\n".to_vec();
    assert!(matches!(
        receiver.consume(&mut name_and_body),
        Progress::NeedContinuation
    ));
    let mut source = b"keep;\r\n".to_vec();
    assert!(matches!(
        receiver.consume(&mut source),
        Progress::Complete(Command::PutScript { name, source })
            if name == "box" && source == "keep;"
    ));
}

fn complete(line: &[u8], content: &[u8]) -> Command {
    let Start::Begin { mut receiver, .. } = start(line) else {
        panic!("ManageSieve literal was not recognized");
    };
    let mut input = content.to_vec();
    match receiver.consume(&mut input) {
        Progress::Complete(command) => command,
        Progress::Waiting
        | Progress::NeedContinuation
        | Progress::Invalid
        | Progress::InvalidUtf8 => {
            panic!("ManageSieve literal did not complete")
        }
    }
}

fn complete_put(line: &[u8], content: &[u8]) -> Command {
    let Start::Begin { mut receiver, .. } = start(line) else {
        panic!("PUTSCRIPT literal name was not recognized");
    };
    let mut input = content.to_vec();
    match receiver.consume(&mut input) {
        Progress::Complete(command) => command,
        Progress::Waiting
        | Progress::NeedContinuation
        | Progress::Invalid
        | Progress::InvalidUtf8 => {
            panic!("PUTSCRIPT literal arguments did not complete")
        }
    }
}
