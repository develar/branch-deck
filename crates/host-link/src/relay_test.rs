use super::*;
use pretty_assertions::assert_eq;
#[cfg(target_os = "macos")]
use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
#[cfg(target_os = "macos")]
use tokio::net::TcpListener;

const DEADLINE: Duration = Duration::from_secs(5);

#[test]
fn only_a_host_unreachable_error_on_macos_takes_the_relay() {
  let unreachable = io::Error::from(io::ErrorKind::HostUnreachable);
  assert_eq!(is_local_network_refusal(&unreachable), cfg!(target_os = "macos") && Path::new(NC).exists());
  assert!(!is_local_network_refusal(&io::Error::from(io::ErrorKind::ConnectionRefused)));
}

/// The real `nc`, so the test runs in real time: a deadline that no run reaches
#[cfg(target_os = "macos")]
#[tokio::test]
async fn the_relay_carries_bytes_both_ways_and_ends_with_the_peer() {
  let listener = TcpListener::bind("127.0.0.1:0").await.expect("a port");
  let address = listener.local_addr().expect("the address");
  let echo = tokio::spawn(async move {
    let (mut socket, _) = listener.accept().await.expect("the relayed connection");
    let mut received = [0u8; 5];
    socket.read_exact(&mut received).await.expect("the bytes");
    socket.write_all(&received).await.expect("the echo");
    socket.shutdown().await.expect("the end");
    received
  });
  let mut relayed = connect(address, DEADLINE).await.expect("relayed");
  relayed.write_all(b"hello").await.expect("written");
  let mut back = Vec::new();
  timeout(DEADLINE, relayed.read_to_end(&mut back)).await.expect("in time").expect("the echo and the end");
  assert_eq!(back, b"hello");
  assert_eq!(&echo.await.expect("the echo task"), b"hello");
}

#[cfg(target_os = "macos")]
#[tokio::test]
async fn a_port_without_a_listener_is_an_error_of_nc() {
  let closed = std::net::TcpListener::bind("127.0.0.1:0").expect("a port").local_addr().expect("the address");
  let error = connect(closed, DEADLINE).await.expect_err("refused");
  assert_eq!(error.kind(), io::ErrorKind::HostUnreachable, "{error}");
  assert!(error.to_string().starts_with("nc: "), "{error}");
}

#[tokio::test]
async fn a_missing_nc_is_an_error_that_names_it() {
  let error = connect_with("/nonexistent/nc", "127.0.0.1:9".parse().expect("an address"), DEADLINE)
    .await
    .expect_err("missing");
  assert!(error.to_string().contains("/nonexistent/nc"), "{error}");
}

#[test]
fn local_network_privacy_is_a_cause_only_of_host_unreachable_on_a_local_address_that_the_relay_cannot_pass() {
  let reason = || Some("there is no /usr/bin/nc".to_owned());
  let lan: IpAddr = "192.168.1.248".parse().expect("an address");
  assert_eq!(
    cause(io::ErrorKind::HostUnreachable, lan, reason()).as_deref(),
    Some(
      "macOS Local Network privacy can refuse 192.168.1.248, because there is no /usr/bin/nc. Then the app that started this process needs the Local Network permission (System Settings, Privacy & Security, Local Network)"
    )
  );
  for local in ["10.0.0.2", "172.16.0.1", "169.254.3.4", "fe80::1", "fd33:e057:95e:4150::1"] {
    let ip = local.parse().expect("an address");
    assert!(cause(io::ErrorKind::HostUnreachable, ip, reason()).is_some(), "{local}");
  }
  assert_eq!(cause(io::ErrorKind::HostUnreachable, lan, None), None, "the relay passes the refusal");
  assert_eq!(cause(io::ErrorKind::ConnectionRefused, lan, reason()), None, "an agent that does not listen");
  let public = "8.8.8.8".parse().expect("an address");
  assert_eq!(cause(io::ErrorKind::HostUnreachable, public, reason()), None, "not the local network");
}

#[cfg(target_os = "macos")]
#[test]
fn the_relay_of_this_mac_passes_a_refusal_of_the_local_network_permission() {
  assert_eq!(unavailable(), None);
  let refused = LinkError::Unreachable(io::Error::from(io::ErrorKind::HostUnreachable));
  assert_eq!(local_network_cause(&refused, "192.168.1.248:7742".parse().expect("an address")), None);
}
