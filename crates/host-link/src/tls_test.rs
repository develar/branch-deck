use crate::tls::{VERSIONS, provider, self_signed};
use rustls::ServerConfig;
use rustls::pki_types::PrivateKeyDer;

#[test]
fn a_self_signed_certificate_serves_tls_1_3_on_the_provider_of_the_app() {
  let (certificate, key) = self_signed().expect("a self-signed certificate");
  let config = ServerConfig::builder_with_provider(provider())
    .with_protocol_versions(VERSIONS)
    .expect("TLS 1.3 on aws-lc-rs")
    .with_no_client_auth()
    .with_single_cert(vec![certificate], PrivateKeyDer::Pkcs8(key))
    .expect("the key fits the certificate");
  // the default provider of aws-lc-rs has the TLS 1.3 suites, and the link offers no other version
  assert!(config.crypto_provider().cipher_suites.iter().any(|suite| suite.tls13().is_some()));
  assert!(VERSIONS.iter().all(|version| version.version == rustls::ProtocolVersion::TLSv1_3));
}
