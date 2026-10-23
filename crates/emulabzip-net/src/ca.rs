//! The root certificate authority that signs the certificates of intercepted hosts.

use std::fs;
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use emulabzip_core::{Error, Result};
use lru::LruCache;
use md5::{Digest, Md5};
use parking_lot::Mutex;
use rcgen::{
    BasicConstraints, CertificateParams, DistinguishedName, DnType, ExtendedKeyUsagePurpose, IsCa, Issuer, KeyPair,
    KeyUsagePurpose, SerialNumber,
};
use rustls::crypto::ring::{default_provider, sign::any_supported_type};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
use rustls::server::{ClientHello, ResolvesServerCert};
use rustls::sign::CertifiedKey;
use time::{Duration, OffsetDateTime};

pub const CERT_FILE: &str = "emulabzip-ca.pem";
pub const KEY_FILE: &str = "emulabzip-ca-key.pem";

fn err(what: &'static str, e: impl std::fmt::Display) -> Error {
    Error::parse(what, e.to_string())
}

/// Root CA plus a cache of the leaf certificates it has issued.
pub struct CertificateAuthority {
    cert_pem: String,
    cert_der: CertificateDer<'static>,
    issuer: Issuer<'static, KeyPair>,
    /// One key serves every leaf: generating a key per host is the costly part of interception
    /// and gains nothing for a CA that only ever signs for the local machine.
    leaf_key: KeyPair,
    leaf_signing_key: Arc<dyn rustls::sign::SigningKey>,
    cache: Mutex<LruCache<String, Arc<CertifiedKey>>>,
    dir: Option<PathBuf>,
}

impl std::fmt::Debug for CertificateAuthority {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CertificateAuthority").field("dir", &self.dir).finish_non_exhaustive()
    }
}

/// The folder where the CA is kept: `~/Library/Application Support/EmuLabZip` on macOS,
/// `%APPDATA%\EmuLabZip` on Windows and `~/.local/share/EmuLabZip` on Linux.
pub fn default_dir() -> PathBuf {
    dirs::data_dir().unwrap_or_else(std::env::temp_dir).join("EmuLabZip")
}

fn random_serial() -> Result<SerialNumber> {
    // A fresh key's DER is random material we already know how to make.
    let der = KeyPair::generate().map_err(|e| err("serial number", e))?.serialize_der();
    let mut bytes: Vec<u8> = der.iter().rev().take(16).copied().collect();
    bytes[0] &= 0x7f; // keep it positive
    Ok(SerialNumber::from_slice(&bytes))
}

fn hostname() -> String {
    std::env::var("HOSTNAME").or_else(|_| std::env::var("COMPUTERNAME")).unwrap_or_else(|_| "local".into())
}

impl CertificateAuthority {
    /// A new CA that lives only in memory.
    pub fn generate() -> Result<Self> {
        let key = KeyPair::generate().map_err(|e| err("CA key", e))?;
        let mut params = CertificateParams::default();
        let mut dn = DistinguishedName::new();
        dn.push(DnType::CommonName, format!("EmuLabZip CA ({})", hostname()));
        dn.push(DnType::OrganizationName, "EmuLabZip");
        dn.push(DnType::OrganizationalUnitName, "https://github.com/LuisPeixoto/EmuLabZip");
        params.distinguished_name = dn;
        params.is_ca = IsCa::Ca(BasicConstraints::Constrained(0));
        params.key_usages =
            vec![KeyUsagePurpose::KeyCertSign, KeyUsagePurpose::CrlSign, KeyUsagePurpose::DigitalSignature];
        let now = OffsetDateTime::now_utc();
        params.not_before = now - Duration::days(2);
        params.not_after = now + Duration::days(365 * 10);
        params.serial_number = Some(random_serial()?);
        let cert = params.self_signed(&key).map_err(|e| err("CA certificate", e))?;
        Self::from_parts(cert.pem(), key, None)
    }

    /// Loads the CA from `dir`, creating and saving a new one when it is missing.
    pub fn load_or_create(dir: impl AsRef<Path>) -> Result<Self> {
        let dir = dir.as_ref();
        let (cert_path, key_path) = (dir.join(CERT_FILE), dir.join(KEY_FILE));
        if cert_path.exists() && key_path.exists() {
            let cert_pem = fs::read_to_string(&cert_path)?;
            let key = KeyPair::from_pem(&fs::read_to_string(&key_path)?).map_err(|e| err("CA key", e))?;
            return Self::from_parts(cert_pem, key, Some(dir.to_path_buf()));
        }
        fs::create_dir_all(dir)?;
        let ca = Self::generate()?;
        fs::write(&cert_path, &ca.cert_pem)?;
        write_private(&key_path, &ca.issuer.key().serialize_pem())?;
        tracing::info!(path = %cert_path.display(), "created a new root certificate");
        Ok(Self { dir: Some(dir.to_path_buf()), ..ca })
    }

    fn from_parts(cert_pem: String, key: KeyPair, dir: Option<PathBuf>) -> Result<Self> {
        let cert_der = rustls_pemfile::certs(&mut cert_pem.as_bytes())
            .next()
            .ok_or_else(|| Error::parse("CA certificate", "the PEM holds no certificate"))?
            .map_err(|e| err("CA certificate", e))?;
        let issuer = Issuer::from_ca_cert_pem(&cert_pem, key).map_err(|e| err("CA certificate", e))?;
        let leaf_key = KeyPair::generate().map_err(|e| err("leaf key", e))?;
        let leaf_der = PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(leaf_key.serialize_der()));
        let leaf_signing_key = any_supported_type(&leaf_der).map_err(|e| err("leaf key", e))?;
        Ok(Self {
            cert_pem,
            cert_der,
            issuer,
            leaf_key,
            leaf_signing_key,
            cache: Mutex::new(LruCache::new(NonZeroUsize::new(2048).expect("non-zero"))),
            dir,
        })
    }

    pub fn cert_pem(&self) -> &str {
        &self.cert_pem
    }

    pub fn cert_der(&self) -> &[u8] {
        self.cert_der.as_ref()
    }

    pub fn dir(&self) -> Option<&Path> {
        self.dir.as_deref()
    }

    pub fn cert_path(&self) -> Option<PathBuf> {
        self.dir.as_ref().map(|d| d.join(CERT_FILE))
    }

    /// A certificate for `host` (a DNS name or an IP address), signed by this CA.
    pub fn leaf_for(&self, host: &str) -> Result<Arc<CertifiedKey>> {
        let host = host.trim_start_matches('[').trim_end_matches(']').to_ascii_lowercase();
        if let Some(hit) = self.cache.lock().get(&host) {
            return Ok(hit.clone());
        }
        let mut params = CertificateParams::new(vec![host.clone()]).map_err(|e| err("host name", e))?;
        let mut dn = DistinguishedName::new();
        dn.push(DnType::CommonName, host.clone());
        dn.push(DnType::OrganizationName, "EmuLabZip");
        params.distinguished_name = dn;
        params.extended_key_usages = vec![ExtendedKeyUsagePurpose::ServerAuth];
        params.key_usages = vec![KeyUsagePurpose::DigitalSignature, KeyUsagePurpose::KeyEncipherment];
        let now = OffsetDateTime::now_utc();
        params.not_before = now - Duration::days(2);
        // Apple platforms refuse leaf certificates that are valid for more than 825 days.
        params.not_after = now + Duration::days(365);
        params.serial_number = Some(random_serial()?);
        params.use_authority_key_identifier_extension = true;
        let cert = params.signed_by(&self.leaf_key, &self.issuer).map_err(|e| err("leaf certificate", e))?;
        let chain = vec![cert.der().clone(), self.cert_der.clone()];
        let certified = Arc::new(CertifiedKey::new(chain, self.leaf_signing_key.clone()));
        self.cache.lock().put(host, certified.clone());
        Ok(certified)
    }

    /// The TLS settings for talking to a client as `fallback_host`: the certificate follows the
    /// name the client asks for, and HTTP/2 is offered before HTTP/1.1.
    pub fn server_config(self: &Arc<Self>, fallback_host: &str) -> Result<Arc<rustls::ServerConfig>> {
        let mut config = rustls::ServerConfig::builder_with_provider(Arc::new(default_provider()))
            .with_safe_default_protocol_versions()
            .map_err(|e| err("TLS versions", e))?
            .with_no_client_auth()
            .with_cert_resolver(Arc::new(SniResolver { ca: self.clone(), fallback: fallback_host.to_string() }));
        config.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];
        Ok(Arc::new(config))
    }
}

#[derive(Debug)]
struct SniResolver {
    ca: Arc<CertificateAuthority>,
    fallback: String,
}

impl ResolvesServerCert for SniResolver {
    fn resolve(&self, hello: ClientHello<'_>) -> Option<Arc<CertifiedKey>> {
        let host = hello.server_name().unwrap_or(&self.fallback);
        self.ca.leaf_for(host).ok()
    }
}

/// OpenSSL's `subject_hash_old`: the first four bytes of the MD5 of the DER subject, read little
/// endian. Android names system certificates `<hash>.0`.
pub fn subject_hash_old(cert_der: &[u8]) -> Result<String> {
    let (_, cert) = x509_parser::parse_x509_certificate(cert_der).map_err(|e| err("certificate", e))?;
    let digest = Md5::digest(cert.subject().as_raw());
    Ok(format!("{:08x}", u32::from_le_bytes([digest[0], digest[1], digest[2], digest[3]])))
}

fn write_private(path: &Path, contents: &str) -> Result<()> {
    #[cfg(unix)]
    {
        use std::io::Write;
        use std::os::unix::fs::OpenOptionsExt;
        let mut f = fs::OpenOptions::new().write(true).create(true).truncate(true).mode(0o600).open(path)?;
        f.write_all(contents.as_bytes())?;
    }
    #[cfg(not(unix))]
    fs::write(path, contents)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mints_and_caches_leaves() {
        let ca = CertificateAuthority::generate().unwrap();
        let a = ca.leaf_for("Example.com").unwrap();
        let b = ca.leaf_for("example.com").unwrap();
        assert!(Arc::ptr_eq(&a, &b));
        assert_eq!(a.cert.len(), 2);
        ca.leaf_for("127.0.0.1").unwrap();
        ca.leaf_for("[::1]").unwrap();
    }

    #[test]
    fn persists_and_reloads_the_same_ca() {
        let dir = std::env::temp_dir().join(format!("emulabzip-ca-{}", std::process::id()));
        let first = CertificateAuthority::load_or_create(&dir).unwrap();
        let again = CertificateAuthority::load_or_create(&dir).unwrap();
        assert_eq!(first.cert_pem(), again.cert_pem());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = fs::metadata(dir.join(KEY_FILE)).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o600, "the private key must not be readable by others");
        }
        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn android_hash_matches_openssl() {
        let ca = CertificateAuthority::generate().unwrap();
        let ours = subject_hash_old(ca.cert_der()).unwrap();
        assert_eq!(ours.len(), 8);
        let dir = std::env::temp_dir().join(format!("emulabzip-hash-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let file = dir.join("ca.pem");
        fs::write(&file, ca.cert_pem()).unwrap();
        if let Ok(out) = std::process::Command::new("openssl")
            .args(["x509", "-subject_hash_old", "-noout", "-in"])
            .arg(&file)
            .output()
            && out.status.success()
        {
            assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), ours);
        }
        fs::remove_dir_all(dir).ok();
    }
}
