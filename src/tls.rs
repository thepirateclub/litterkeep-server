//! Загрузка TLS-материала для будущего неблокирующего server transport.
//!
//! Runtime принимает только DER-файлы. Это намеренное узкое правило первой
//! версии: декодер PEM не добавляется как лишняя production-зависимость, а
//! ошибочный ключ или сертификат отклоняются `rustls` до открытия listener-а.
use rustls::{
    ServerConfig,
    pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer},
};
use std::{fs, io, path::Path, sync::Arc};

/// Пути к leaf-сертификату и PKCS#8 private key в DER-представлении.
pub struct ServerMaterial {
    certificate_der: Vec<u8>,
    private_key_der: Vec<u8>,
}

impl ServerMaterial {
    /// Читает материал один раз, не сохраняя открытые файловые дескрипторы.
    pub fn load(certificate: impl AsRef<Path>, private_key: impl AsRef<Path>) -> io::Result<Self> {
        Ok(Self {
            certificate_der: fs::read(certificate)?,
            private_key_der: fs::read(private_key)?,
        })
    }

    /// Собирает конфигурацию rustls и тем самым проверяет пару ключ-сертификат.
    pub fn server_config(self) -> Result<Arc<ServerConfig>, rustls::Error> {
        let certificate = CertificateDer::from(self.certificate_der);
        let private_key = PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(self.private_key_der));
        let config = ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(vec![certificate], private_key)?;
        Ok(Arc::new(config))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Отсутствующий секрет не маскируется пустой конфигурацией TLS.
    #[test]
    fn missing_material_is_an_io_error() {
        let absent = std::env::temp_dir().join("litterkeep-tls-material-absent");
        assert!(ServerMaterial::load(&absent, &absent).is_err());
    }

    /// Пустые DER-файлы не становятся работоспособной TLS-конфигурацией.
    #[test]
    fn invalid_der_is_rejected_by_rustls() {
        let material = ServerMaterial {
            certificate_der: Vec::new(),
            private_key_der: Vec::new(),
        };
        assert!(material.server_config().is_err());
    }
}
