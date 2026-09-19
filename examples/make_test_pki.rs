//! Однократный выпуск тестового CA и leaf-сертификата для Linux acceptance.
//!
//! Серверу требуются только `server.der` и `server.key.der`. `ca.pem` создан
//! исключительно для независимых клиентов, например Python `ssl`, а закрытый
//! ключ CA остаётся в каталоге разработчика и никогда не передаётся `serve`.
use rcgen::{
    BasicConstraints, CertificateParams, CertifiedIssuer, DnType, ExtendedKeyUsagePurpose, IsCa,
    KeyPair, KeyUsagePurpose,
};
use std::{env, fs, io, path::Path};

/// Имена всех результатов, существование которых запрещает неявную замену CA.
const OUTPUTS: [&str; 5] = [
    "ca.der",
    "ca.pem",
    "ca.key.der",
    "server.der",
    "server.key.der",
];

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let directory = env::args().nth(1).unwrap_or_else(|| "test-pki".into());
    prepare_directory(Path::new(&directory))?;

    let mut ca_params = CertificateParams::default();
    ca_params
        .distinguished_name
        .push(DnType::CommonName, "litterkeep test CA");
    ca_params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
    ca_params.key_usages = vec![
        KeyUsagePurpose::KeyCertSign,
        KeyUsagePurpose::CrlSign,
        KeyUsagePurpose::DigitalSignature,
    ];
    let ca_key = KeyPair::generate()?;
    let ca = CertifiedIssuer::self_signed(ca_params, ca_key)?;

    let mut server_params = CertificateParams::new(vec![
        "litterkeep.test".to_owned(),
        "localhost".to_owned(),
        "127.0.0.1".to_owned(),
    ])?;
    // Связывает leaf с ключом issuer, чтобы строгий клиент строил цепочку CA.
    server_params.use_authority_key_identifier_extension = true;
    server_params
        .distinguished_name
        .push(DnType::CommonName, "litterkeep.test");
    server_params.extended_key_usages = vec![ExtendedKeyUsagePurpose::ServerAuth];
    let server_key = KeyPair::generate()?;
    let server = server_params.signed_by(&server_key, &ca)?;

    fs::write(Path::new(&directory).join("ca.der"), ca.der())?;
    fs::write(Path::new(&directory).join("ca.pem"), ca.pem())?;
    fs::write(
        Path::new(&directory).join("ca.key.der"),
        ca.key().serialize_der(),
    )?;
    fs::write(Path::new(&directory).join("server.der"), server.der())?;
    fs::write(
        Path::new(&directory).join("server.key.der"),
        server_key.serialize_der(),
    )?;
    println!("created test PKI in {directory}");
    Ok(())
}

/// Создаёт каталог только при отсутствии всех результатов предыдущего выпуска.
fn prepare_directory(directory: &Path) -> io::Result<()> {
    fs::create_dir_all(directory)?;
    let existing = OUTPUTS
        .iter()
        .map(|name| directory.join(name))
        .find(|path| path.exists());
    match existing {
        Some(path) => Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!("refusing to replace existing PKI file: {}", path.display()),
        )),
        None => Ok(()),
    }
}
