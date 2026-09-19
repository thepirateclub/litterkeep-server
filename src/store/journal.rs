use super::model::{Record, blob_name};
use std::{fs, io, path::Path};
pub fn frame(uid: u64, name: &str) -> String {
    format!("V1 {uid} {name} {:016x}", checksum(uid, name))
}
pub fn records(mailbox: &str, path: &Path) -> io::Result<Vec<Record>> {
    let text = match fs::read_to_string(path) {
        Ok(v) => v,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e),
    };
    let mut previous = 0;
    text.lines()
        .map(|line| {
            let r = parse(mailbox, line)?;
            if r.uid <= previous {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "non-monotonic uid",
                ));
            }
            previous = r.uid;
            Ok(r)
        })
        .collect()
}
fn parse(mailbox: &str, line: &str) -> io::Result<Record> {
    let mut f = line.split(' ');
    let (Some("V1"), Some(uid), Some(name), Some(sum), None) =
        (f.next(), f.next(), f.next(), f.next(), f.next())
    else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "bad journal frame",
        ));
    };
    let uid = uid
        .parse()
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "bad uid"))?;
    if name != blob_name(mailbox, uid) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "unsafe blob reference",
        ));
    };
    let sum = u64::from_str_radix(sum, 16)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "bad checksum"))?;
    if sum != checksum(uid, name) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "checksum mismatch",
        ));
    };
    Ok(Record {
        uid,
        name: name.into(),
    })
}
fn checksum(uid: u64, name: &str) -> u64 {
    let mut v = 0xcbf29ce484222325_u64;
    for b in format!("{uid} {name}").bytes() {
        v ^= u64::from(b);
        v = v.wrapping_mul(0x100000001b3)
    }
    v
}
