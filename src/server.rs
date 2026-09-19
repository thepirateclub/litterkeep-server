//! Local SMTP submission listener. One accept loop owns all protocol state.
use crate::{smtp, store::Store};
use std::io::{self, BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::path::Path;
pub fn serve_smtp(address: &str, root: impl AsRef<Path>) -> io::Result<()> {
    let listener = TcpListener::bind(address)?;
    let store = Store::open(root)?;
    for stream in listener.incoming() {
        handle(stream?, &store)?;
    }
    Ok(())
}

pub fn serve_imap(address: &str, root: impl AsRef<Path>) -> io::Result<()> {
    let listener = TcpListener::bind(address)?;
    let store = Store::open(root)?;
    for stream in listener.incoming() {
        let mut s = stream?;
        reply(&mut s, "* OK IMAP4rev1\r\n")?;
        let mut r = BufReader::new(s.try_clone()?);
        loop {
            let mut l = String::new();
            if r.read_line(&mut l)? == 0 {
                break;
            };
            let mut p = l.split_whitespace();
            let t = p.next().unwrap_or("*");
            match p.next().unwrap_or("").to_ascii_uppercase().as_str() {
                "LOGIN" => reply(&mut s, &format!("{t} OK\r\n"))?,
                "SELECT" => reply(
                    &mut s,
                    &format!("* {} EXISTS\r\n{t} OK\r\n", store.messages("bob")?.len()),
                )?,
                "FETCH" => {
                    let n = p.next().and_then(|x| x.parse::<usize>().ok()).unwrap_or(0);
                    if let Some(m) = store.messages("bob")?.get(n.saturating_sub(1)) {
                        reply(
                            &mut s,
                            &format!("* {n} FETCH (RFC822 {{{}}}\r\n", m.bytes.len()),
                        )?;
                        s.write_all(&m.bytes)?;
                        reply(&mut s, &format!("\r\n)\r\n{t} OK\r\n"))?
                    } else {
                        reply(&mut s, &format!("{t} NO\r\n"))?
                    }
                }
                "LOGOUT" => {
                    reply(&mut s, "* BYE\r\n")?;
                    break;
                }
                _ => reply(&mut s, &format!("{t} BAD\r\n"))?,
            }
        }
    }
    Ok(())
}
fn reply(stream: &mut TcpStream, text: &str) -> io::Result<()> {
    stream.write_all(text.as_bytes())?;
    stream.flush()
}
fn handle(mut stream: TcpStream, store: &Store) -> io::Result<()> {
    reply(
        &mut stream,
        "220 litterkeep.test ESMTP litterkeep-server\r\n",
    )?;
    let read = stream.try_clone()?;
    let mut input = BufReader::new(read);
    let mut tx = smtp::Transaction::default();
    loop {
        let mut line = String::new();
        if input.read_line(&mut line)? == 0 {
            return Ok(());
        };
        let command = match smtp::parse(&line) {
            Ok(c) => c,
            Err(_) => {
                reply(&mut stream, "500 syntax error\r\n")?;
                continue;
            }
        };
        if command == smtp::Command::Quit {
            reply(&mut stream, "221 bye\r\n")?;
            return Ok(());
        };
        if matches!(command, smtp::Command::Ehlo(_)) {
            reply(&mut stream, "250-litterkeep.test\r\n250 SIZE 10485760\r\n")?;
            continue;
        };
        match tx.apply(&command) {
            Ok(true) => {
                reply(&mut stream, "354 end with <CRLF>.<CRLF>\r\n")?;
                let mut raw = Vec::new();
                loop {
                    let mut data = String::new();
                    if input.read_line(&mut data)? == 0 {
                        return Ok(());
                    };
                    if data == ".\r\n" {
                        break;
                    };
                    if let Some(rest) = data.strip_prefix("..") {
                        raw.extend_from_slice(rest.as_bytes())
                    } else {
                        raw.extend_from_slice(data.as_bytes())
                    };
                    if raw.len() > 10 * 1024 * 1024 {
                        reply(&mut stream, "552 message too large\r\n")?;
                        break;
                    }
                }
                if raw.len() <= 10 * 1024 * 1024 {
                    store.append("bob", &raw)?;
                    reply(&mut stream, "250 queued\r\n")?;
                }
            }
            Ok(false) => reply(&mut stream, "250 ok\r\n")?,
            Err(_) => reply(&mut stream, "503 bad sequence\r\n")?,
        }
    }
}
