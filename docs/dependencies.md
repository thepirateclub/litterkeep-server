# Dependency policy

At the G00/G01 checkpoint, `Cargo.toml` declares no dependencies. The lockfile
and `cargo tree -e normal` are evidence that no mail engine, database, runtime,
actor framework, parser generator or hidden service is present.

The project contract permits only a minimally featured `rustls` and RustCrypto
`argon2` as direct production dependencies. They will not be introduced until
G03 selects exact versions, feature flags and certificate/password behavior.
