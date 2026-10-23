//! Naming certificates the way Android's system store does.

use emulabzip_core::{Error, Result};
use md5::{Digest, Md5};

/// OpenSSL's `subject_hash_old`: the first four bytes of the MD5 of the DER subject, read little
/// endian. Android names system certificates `<hash>.0`.
pub fn subject_hash_old(cert_der: &[u8]) -> Result<String> {
    let (_, cert) =
        x509_parser::parse_x509_certificate(cert_der).map_err(|e| Error::parse("certificate", e.to_string()))?;
    let digest = Md5::digest(cert.subject().as_raw());
    Ok(format!("{:08x}", u32::from_le_bytes([digest[0], digest[1], digest[2], digest[3]])))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> rcgen::Certificate {
        let key = rcgen::KeyPair::generate().unwrap();
        let mut params = rcgen::CertificateParams::new(Vec::<String>::new()).unwrap();
        params.distinguished_name.push(rcgen::DnType::CommonName, "EmuLabZip CA test");
        params.is_ca = rcgen::IsCa::Ca(rcgen::BasicConstraints::Unconstrained);
        params.self_signed(&key).unwrap()
    }

    #[test]
    fn hashes_the_subject_not_the_whole_certificate() {
        let (a, b) = (sample(), sample());
        // Same subject, different keys and serials: the name must be the same.
        assert_eq!(subject_hash_old(a.der()).unwrap(), subject_hash_old(b.der()).unwrap());
        assert_eq!(subject_hash_old(a.der()).unwrap().len(), 8);
    }

    #[test]
    fn matches_openssl() {
        let cert = sample();
        let ours = subject_hash_old(cert.der()).unwrap();
        let dir = std::env::temp_dir().join(format!("emulabzip-hash-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("ca.pem");
        std::fs::write(&file, cert.pem()).unwrap();
        // Skipped quietly where OpenSSL is not installed.
        if let Ok(out) = std::process::Command::new("openssl")
            .args(["x509", "-subject_hash_old", "-noout", "-in"])
            .arg(&file)
            .output()
            && out.status.success()
        {
            assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), ours);
        }
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn rejects_garbage() {
        assert!(subject_hash_old(b"not a certificate").is_err());
    }
}
