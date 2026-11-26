use crate::config::AppConfig;
use anyhow::{anyhow, Context, Result};
use dashmap::DashMap;
use rcgen::{
    BasicConstraints, Certificate, CertificateParams, DnType, ExtendedKeyUsagePurpose, IsCa,
    KeyPair, KeyUsagePurpose, SanType,
};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
use rustls::ServerConfig;
use std::fs;
use std::net::IpAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub struct CertAuthority {
    ca_cert: Certificate,
    ca_key: KeyPair,
    ca_der: CertificateDer<'static>,
    ca_pem_path: PathBuf,
    cache: DashMap<String, Arc<ServerConfig>>,
}

impl CertAuthority {
    pub fn load_or_create() -> Result<Arc<Self>> {
        let dir = AppConfig::ca_dir();
        Self::load_or_create_in(&dir)
    }

    pub fn load_or_create_in(dir: &Path) -> Result<Arc<Self>> {
        fs::create_dir_all(dir)?;
        let cert_path = dir.join("root_ca.pem");
        let key_path = dir.join("root_ca.key");

        if cert_path.exists() && key_path.exists() {
            let cert_pem = fs::read_to_string(&cert_path)?;
            let key_pem = fs::read_to_string(&key_path)?;
            let ca_key = KeyPair::from_pem(&key_pem).context("解析本地 Root CA 私钥失败")?;
            let ca_params =
                CertificateParams::from_ca_cert_pem(&cert_pem).context("解析本地 Root CA 证书失败")?;
            let ca_cert = ca_params.self_signed(&ca_key)?;

            // Preserve exact DER from the saved PEM file
            let mut reader = std::io::Cursor::new(cert_pem.as_bytes());
            let ca_der = rustls_pemfile::certs(&mut reader)
                .next()
                .ok_or_else(|| anyhow!("root_ca.pem 中未找到有效证书"))??;

            return Ok(Arc::new(Self {
                ca_cert,
                ca_key,
                ca_der,
                ca_pem_path: cert_path,
                cache: DashMap::new(),
            }));
        }

        let mut params = CertificateParams::default();
        params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
        params
            .distinguished_name
            .push(DnType::CommonName, "VP429 Local CA");
        params
            .distinguished_name
            .push(DnType::OrganizationName, "VP429");
        params.key_usages = vec![
            KeyUsagePurpose::KeyCertSign,
            KeyUsagePurpose::CrlSign,
            KeyUsagePurpose::DigitalSignature,
        ];

        let ca_key = KeyPair::generate()?;
        let ca_cert = params.self_signed(&ca_key)?;
        let cert_pem = ca_cert.pem();
        let key_pem = ca_key.serialize_pem();

        fs::write(&cert_path, &cert_pem)?;
        fs::write(&key_path, &key_pem)?;

        let ca_der = ca_cert.der().clone();
        Ok(Arc::new(Self {
            ca_cert,
            ca_key,
            ca_der,
            ca_pem_path: cert_path,
            cache: DashMap::new(),
        }))
    }

    pub fn ca_pem_path(&self) -> &Path {
        &self.ca_pem_path
    }

    #[cfg(test)]
    pub fn ca_der(&self) -> &CertificateDer<'static> {
        &self.ca_der
    }

    /// Generates (or returns cached) a rustls ServerConfig with a leaf cert signed by our Root CA.
    pub fn server_config_for_host(&self, host: &str) -> Result<Arc<ServerConfig>> {
        let key = host.to_ascii_lowercase();
        if let Some(existing) = self.cache.get(&key) {
            return Ok(existing.value().clone());
        }

        let mut params = CertificateParams::default();
        params.distinguished_name.push(DnType::CommonName, &key);
        params.extended_key_usages = vec![ExtendedKeyUsagePurpose::ServerAuth];
        if let Ok(ip) = key.parse::<IpAddr>() {
            params.subject_alt_names = vec![SanType::IpAddress(ip)];
        } else {
            params.subject_alt_names = vec![SanType::DnsName(key.clone().try_into()?)];
        }

        let leaf_key = KeyPair::generate()?;
        let leaf_cert = params.signed_by(&leaf_key, &self.ca_cert, &self.ca_key)?;

        let cert_chain = vec![leaf_cert.der().clone(), self.ca_der.clone()];
        let priv_key = PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(leaf_key.serialize_der()));

        let mut server_cfg = ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(cert_chain, priv_key)?;
        server_cfg.alpn_protocols = vec![b"http/1.1".to_vec()];

        let arc = Arc::new(server_cfg);
        self.cache.insert(key, arc.clone());
        Ok(arc)
    }

    /// Installs the Root CA into the current Windows user's Trusted Root Certification Authorities store.
    #[cfg(windows)]
    pub fn install_windows_user_root(&self) -> Result<String> {
        let output = std::process::Command::new("certutil")
            .args([
                "-user",
                "-addstore",
                "Root",
                &self.ca_pem_path.to_string_lossy(),
            ])
            .output()
            .context("调用 certutil 安装根证书失败")?;

        if output.status.success() {
            // When running elevated, also install into LocalMachine\Root for system-wide trust
            let mut msg = "已安装到当前用户受信任根证书存储区 (CurrentUser\\Root)".to_string();
            if crate::mihomo::is_elevated() {
                if let Ok(lm) = std::process::Command::new("certutil")
                    .args(["-addstore", "Root", &self.ca_pem_path.to_string_lossy()])
                    .output()
                {
                    if lm.status.success() {
                        msg = "已安装到用户与本机受信任根证书存储区 (CurrentUser + LocalMachine\\Root)"
                            .to_string();
                    }
                }
            }
            Ok(msg)
        } else {
            let err = String::from_utf8_lossy(&output.stderr);
            let out = String::from_utf8_lossy(&output.stdout);
            Err(anyhow!("安装根证书失败: {} {}", out.trim(), err.trim()))
        }
    }

    #[cfg(not(windows))]
    pub fn install_windows_user_root(&self) -> Result<String> {
        Ok(format!("请手动信任证书: {:?}", self.ca_pem_path))
    }

    #[cfg(windows)]
    pub fn is_installed_windows(&self) -> bool {
        let Ok(out) = std::process::Command::new("certutil")
            .args(["-user", "-verifystore", "Root", "VP429 Local CA"])
            .output()
        else {
            return false;
        };
        out.status.success()
    }

    #[cfg(not(windows))]
    pub fn is_installed_windows(&self) -> bool {
        false
    }
}
