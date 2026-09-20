//! Детерминированные MIME-вложения, включая граничное вложение 10 MiB.

use super::{Delivery, Fixture, Mailbox, Owner, fixture, message};

pub(super) fn fixtures() -> Vec<Fixture> {
    vec![
        attached(
            "seed-029-text",
            "note.txt",
            "text/plain",
            b"seed text attachment\r\n",
        ),
        attached("seed-030-png", "pixel.png", "image/png", &pixel_png()),
        attached(
            "seed-031-binary",
            "bytes.bin",
            "application/octet-stream",
            &[0, 1, 2, 3, 255],
        ),
        attached(
            "seed-032-empty",
            "empty.bin",
            "application/octet-stream",
            &[],
        ),
        attached(
            "seed-033-unicode",
            "отчёт.txt",
            "text/plain",
            "данные\r\n".as_bytes(),
        ),
        duplicate_names(),
        attached(
            "seed-035-ten-mib",
            "ten-mib.bin",
            "application/octet-stream",
            &vec![0xA5; 10 * 1024 * 1024],
        ),
    ]
}

fn attached(id: &str, name: &str, media_type: &str, bytes: &[u8]) -> Fixture {
    let boundary = format!("boundary-{id}");
    let encoded = base64_lines(bytes);
    let body = format!(
        "--{boundary}\r\nContent-Type: text/plain; charset=utf-8\r\n\r\nAttachment fixture.\r\n--{boundary}\r\nContent-Type: {media_type}\r\nContent-Disposition: attachment; filename*=utf-8''{}\r\nContent-Transfer-Encoding: base64\r\n\r\n{encoded}--{boundary}--\r\n",
        percent_encode(name.as_bytes())
    );
    fixture(
        id,
        Mailbox::Inbox,
        Owner::Bob,
        Delivery::Smtp,
        "",
        message(
            id,
            "Attachment fixture",
            "",
            &format!("multipart/mixed; boundary={boundary}"),
            body.as_bytes(),
        ),
    )
}

fn duplicate_names() -> Fixture {
    let body = b"--seed-duplicate\r\nContent-Type: application/octet-stream\r\nContent-Disposition: attachment; filename=same.bin\r\nContent-Transfer-Encoding: base64\r\n\r\nAQ==\r\n--seed-duplicate\r\nContent-Type: application/octet-stream\r\nContent-Disposition: attachment; filename=same.bin\r\nContent-Transfer-Encoding: base64\r\n\r\nAgM=\r\n--seed-duplicate--\r\n";
    fixture(
        "seed-034-duplicate-names",
        Mailbox::Inbox,
        Owner::Bob,
        Delivery::Smtp,
        "",
        message(
            "seed-034-duplicate-names",
            "Duplicate attachment names",
            "",
            "multipart/mixed; boundary=seed-duplicate",
            body,
        ),
    )
}

fn pixel_png() -> Vec<u8> {
    vec![
        137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 1, 0, 0, 0, 1, 8, 4,
        0, 0, 0, 181, 28, 12, 2, 0, 0, 0, 11, 73, 68, 65, 84, 120, 218, 99, 252, 255, 31, 0, 3, 3,
        2, 0, 238, 86, 218, 62, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96, 130,
    ]
}

fn base64_lines(input: &[u8]) -> String {
    input
        .chunks(57)
        .map(base64_block)
        .map(|line| format!("{line}\r\n"))
        .collect()
}

fn base64_block(input: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut output = String::with_capacity(input.len().div_ceil(3).saturating_mul(4));
    for chunk in input.chunks(3) {
        let a = u32::from(chunk.first().copied().unwrap_or(0));
        let b = u32::from(chunk.get(1).copied().unwrap_or(0));
        let c = u32::from(chunk.get(2).copied().unwrap_or(0));
        let value = (a << 16) | (b << 8) | c;
        for shift in [18_u32, 12, 6, 0] {
            let index = usize::try_from((value >> shift) & 63).unwrap_or_default();
            output.push(char::from(ALPHABET.get(index).copied().unwrap_or(b'A')));
        }
        match chunk.len() {
            1 => output.replace_range(output.len().saturating_sub(2).., "=="),
            2 => output.replace_range(output.len().saturating_sub(1).., "="),
            _ => {}
        }
    }
    output
}

fn percent_encode(input: &[u8]) -> String {
    input
        .iter()
        .map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'.' | b'-' | b'_' => {
                char::from(*byte).to_string()
            }
            _ => format!("%{byte:02X}"),
        })
        .collect()
}
