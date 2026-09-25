//! Единый фактический CLI usage без логики выполнения команд.

use litterkeep_server::{PRODUCT, VERSION};

pub(crate) fn print() {
    println!("{PRODUCT} {VERSION}\n");
    println!("Usage:");
    println!("  litterkeep-server init --data-dir <data-dir>");
    println!("  litterkeep-server init-smtputf8-test --data-dir <data-dir>");
    println!(
        "  litterkeep-server seed <smtp> <imap> <server-name> <ca.der> <account> <password-file> <bob-password-file> <qa-password-file>"
    );
    println!("  litterkeep-server check --data-dir <data-dir>");
    println!("  litterkeep-server backup --data-dir <data-dir> --output <backup-dir>");
    println!("  litterkeep-server deliver <address> <sender> <recipient> <message-file>");
    println!(
        "  litterkeep-server deliver-tls <address> <server-name> <ca.der> <sender> <recipient> <message-file>"
    );
    println!("  litterkeep-server queue-drain <data-dir> <peer-address> [limit]");
    println!("  litterkeep-server sieve-errors <data-dir> <account>");
    println!("  litterkeep-server runtime <stats|tasks|resources> --data-dir <data-dir>");
    println!("  litterkeep-server runtime inspect <task-id> --data-dir <data-dir>");
    println!("  litterkeep-server runtime cancel <task-id> --data-dir <data-dir>");
    println!("  litterkeep-server serve [smtp-address] [imap-address] [data-dir]");
    println!("  litterkeep-server serve-tls [smtp] [imap] [data-dir] <certificate.der> <key.der>");
    println!(
        "  litterkeep-server serve-starttls [smtp] [imap] [data-dir] <certificate.der> <key.der>"
    );
    println!(
        "  litterkeep-server serve-all-starttls [smtp] [imap] [pop3] [data-dir] <certificate.der> <key.der>"
    );
    println!(
        "  litterkeep-server serve-pop3-starttls [pop3] [data-dir] <certificate.der> <key.der>"
    );
    println!("  litterkeep-server serve-pop3-tls [pop3] [data-dir] <certificate.der> <key.der>");
    println!(
        "  litterkeep-server serve-managesieve-starttls [address] [data-dir] <certificate.der> <key.der>"
    );
    println!(
        "  litterkeep-server serve-all-mail-starttls [smtp] [imap] [pop3] [sieve] [data-dir] <certificate.der> <key.der>"
    );
    println!(
        "  litterkeep-server serve-all-mail-outbound-starttls [smtp] [imap] [pop3] [sieve] [peer] [data-dir] <certificate.der> <key.der>"
    );
    println!(
        "  litterkeep-server serve-all-mail-outbound-tls-starttls <smtp> <imap> <pop3> <sieve> <peer> <peer-name> <peer-ca.der> <data-dir> <certificate.der> <key.der>"
    );
    println!("  litterkeep-server help");
    println!("  litterkeep-server version");
}
