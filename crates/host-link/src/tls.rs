//! The TLS of the link: version 1.3 only, on the crypto provider that the app links through reqwest (`aws-lc-rs`), so
//! no second provider comes in. Each end has a self-signed certificate, and the other end pins its fingerprint; the
//! name in the certificate proves nothing

use rustls::SupportedProtocolVersion;
use rustls::crypto::CryptoProvider;
use rustls::pki_types::{CertificateDer, PrivatePkcs8KeyDer};
use std::sync::Arc;

/// The protocol versions of the link
pub static VERSIONS: &[&SupportedProtocolVersion] = &[&rustls::version::TLS13];

/// The name in the certificate of every end, and the server name of every connection
pub const SERVER_NAME: &str = "bd-host";

/// The crypto provider of the link: the default provider of `aws-lc-rs`
pub fn provider() -> Arc<CryptoProvider> {
  Arc::new(rustls::crypto::aws_lc_rs::default_provider())
}

/// A new self-signed certificate for `SERVER_NAME` and its PKCS#8 key, ECDSA P-256, as DER. Each end checks the
/// fingerprint and not the dates, so the certificate never expires for the link
pub fn self_signed() -> Result<(CertificateDer<'static>, PrivatePkcs8KeyDer<'static>), rcgen::Error> {
  let key = rcgen::KeyPair::generate()?;
  let certificate = rcgen::CertificateParams::new(vec![SERVER_NAME.to_owned()])?.self_signed(&key)?;
  Ok((certificate.der().clone(), PrivatePkcs8KeyDer::from(key.serialize_der())))
}
