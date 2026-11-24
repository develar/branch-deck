//! The channel: TLS 1.3 over one connection, each end pinned by its fingerprint, and the frames on it
//! (openspec/specs/cli/host/spec.md, "The link"). It runs on tokio: TLS by tokio-rustls, the frames by a `Framed` whose
//! codec is `LengthDelimitedCodec`, and each deadline by `tokio::time::timeout`.
//!
//! A frame is a kind byte, a big-endian u32 length and the payload:
//!
//! | Kind | Payload |
//! |---|---|
//! | 0, a message | one JSON value, at most [`MAX_MESSAGE`] bytes |
//! | 1, bytes | a chunk of a byte stream, at most [`CHUNK`] bytes |
//! | 2, the end | nothing: the byte stream before it is complete |
//!
//! One connection carries one request. Every read and every write has a deadline of the channel, so no call waits for
//! ever. A caller that stops a connection drops its channel, or the future that uses it: the connection ends then.
//!
//! The `Framed` collects the frames that the channel sends in its buffer, and writes the buffer to TLS in one write when
//! it holds [`CHUNK`] bytes, at a flush, before a read and at the close. Each write to TLS makes its records at once, so
//! this is what puts many small frames into few TLS records.

use crate::allow::AllowList;
use crate::identity::{Fingerprint, Identity};
use crate::refusal::Code;
use crate::relay;
use crate::tls::{SERVER_NAME, VERSIONS, provider};
use futures_util::{SinkExt as _, StreamExt as _};
use rustls::client::Resumption;
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::crypto::{CryptoProvider, WebPkiSupportedAlgorithms, verify_tls13_signature};
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::server::danger::{ClientCertVerified, ClientCertVerifier};
use rustls::{CertificateError, ClientConfig, DigitallySignedStruct, DistinguishedName, ServerConfig, SignatureScheme};
use serde::Serialize;
use serde::de::DeserializeOwned;
use std::fmt;
use std::future::Future;
use std::io::{self, Read, Write};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt as _};
use tokio::net::TcpStream;
use tokio::time::timeout;
use tokio_rustls::{TlsAcceptor, TlsConnector};
use tokio_util::bytes::{BufMut as _, BytesMut};
use tokio_util::codec::{Decoder, Encoder, Framed, LengthDelimitedCodec};

/// The largest JSON message.
pub const MAX_MESSAGE: usize = 1024 * 1024;

/// The largest chunk of a byte stream.
pub const CHUNK: usize = 64 * 1024;

const KIND_MESSAGE: u8 = 0;
const KIND_BYTES: u8 = 1;
const KIND_END: u8 = 2;

/// The head of a frame: its kind and its length
const HEAD: usize = 5;

/// Why a channel failed. `code` names the refusal of each kind
#[derive(Debug, thiserror::Error)]
pub enum LinkError {
  /// No TCP connection within the deadline, or the connection was refused or reset before the handshake ended
  #[error("no connection: {0}")]
  Unreachable(io::Error),
  /// The other end showed a certificate that this end does not trust: another host behind the address, or a
  /// client that the allow list of the agent does not name
  #[error("{0}")]
  Untrusted(String),
  /// No frame within the read deadline, or a write that did not end within the write deadline
  #[error("the other end did not answer in time")]
  TimedOut,
  /// The other end closed the connection, or sent a frame that is not in the protocol
  #[error("{0}")]
  Protocol(String),
  #[error("{0}")]
  Io(io::Error),
}

impl LinkError {
  /// The refusal code of this failure: `unreachable`, `untrusted`, `timed_out`, or `link` for a lost connection and a
  /// frame outside the protocol
  pub const fn code(&self) -> Code {
    match self {
      Self::Unreachable(_) => Code::Unreachable,
      Self::Untrusted(_) => Code::Untrusted,
      Self::TimedOut => Code::TimedOut,
      Self::Protocol(_) | Self::Io(_) => Code::Link,
    }
  }

  fn from_io(error: io::Error) -> Self {
    if matches!(error.kind(), io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock) {
      return Self::TimedOut;
    }
    if error.kind() == io::ErrorKind::UnexpectedEof {
      return Self::Protocol("the other end closed the connection".to_owned());
    }
    // rustls reports a refused certificate as an io::Error that wraps its own error.
    if let Some(tls) = error.get_ref().and_then(|inner| inner.downcast_ref::<rustls::Error>()) {
      return match tls {
        rustls::Error::InvalidCertificate(_)
        | rustls::Error::AlertReceived(rustls::AlertDescription::BadCertificate | rustls::AlertDescription::AccessDenied | rustls::AlertDescription::CertificateUnknown) => {
          Self::Untrusted(format!("the other end refused the identity or showed one that is not trusted: {tls}"))
        }
        _ => Self::Protocol(format!("TLS: {tls}")),
      };
    }
    Self::Io(error)
  }
}

impl From<io::Error> for LinkError {
  fn from(error: io::Error) -> Self {
    Self::from_io(error)
  }
}

/// A byte stream that TLS runs over, and the TLS stream that the frames run over
pub trait Io: AsyncRead + AsyncWrite + Send + Unpin {}

impl<T: AsyncRead + AsyncWrite + Send + Unpin> Io for T {}

/// The connection under TLS: a TCP socket, or the pipes of the relay (`relay`)
pub type Link = Box<dyn Io>;

/// The TLS configuration of a client of one host: the pin of the host and the identity of the client. A client builds
/// it once and connects with it as often as it needs. Session resumption is off, so each handshake checks both
/// certificates.
#[derive(Clone)]
pub struct ClientTls {
  config: Arc<ClientConfig>,
}

impl fmt::Debug for ClientTls {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.write_str("ClientTls")
  }
}

impl ClientTls {
  /// The configuration that checks that the host shows the certificate of `host`.
  pub fn new(host: Fingerprint, identity: &Identity) -> Result<Self, LinkError> {
    let provider = provider();
    let mut config = ClientConfig::builder_with_provider(Arc::clone(&provider))
      .with_protocol_versions(VERSIONS)
      .map_err(tls_setup)?
      .dangerous()
      .with_custom_certificate_verifier(Arc::new(PinnedHost {
        host,
        signatures: Tls13 {
          algorithms: provider.signature_verification_algorithms,
        },
      }))
      .with_client_auth_cert(identity.cert_chain(), identity.private_key())
      .map_err(tls_setup)?;
    config.resumption = Resumption::disabled();
    Ok(Self { config: Arc::new(config) })
  }
}

/// The TLS setup of an agent: its identity and the verifier of the allow list of its clients, built once. The verifier
/// reads the allow list at each handshake. Each handshake gets a configuration of its own, with a session store of its
/// own, so no session outlives its connection, and a client that left the allow list gets no later connection.
#[derive(Clone)]
pub struct ServerTls {
  provider: Arc<CryptoProvider>,
  verifier: Arc<AllowedClients>,
  identity: Identity,
}

impl fmt::Debug for ServerTls {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.write_str("ServerTls")
  }
}

impl ServerTls {
  /// The setup that accepts a client whose certificate the allow list `allow` names.
  pub fn new(identity: &Identity, allow: PathBuf) -> Result<Self, LinkError> {
    let provider = provider();
    let tls = Self {
      verifier: Arc::new(AllowedClients {
        allow,
        signatures: Tls13 {
          algorithms: provider.signature_verification_algorithms,
        },
      }),
      provider,
      identity: identity.clone(),
    };
    tls.config()?;
    Ok(tls)
  }

  /// The configuration of one handshake.
  fn config(&self) -> Result<Arc<ServerConfig>, LinkError> {
    let config = ServerConfig::builder_with_provider(Arc::clone(&self.provider))
      .with_protocol_versions(VERSIONS)
      .map_err(tls_setup)?
      .with_client_cert_verifier(Arc::clone(&self.verifier) as Arc<dyn ClientCertVerifier>)
      .with_single_cert(self.identity.cert_chain(), self.identity.private_key())
      .map_err(tls_setup)?;
    Ok(Arc::new(config))
  }
}

/// One connection of the protocol, after the handshake.
pub struct Channel {
  /// The frames over the TLS stream
  framed: Framed<Box<dyn Io>, Frames>,
  peer: Fingerprint,
  /// The deadline of each read of a frame
  read_deadline: Duration,
  /// The deadline of each write of the buffer, of a flush and of the close
  write_deadline: Duration,
}

impl fmt::Debug for Channel {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "Channel({})", self.peer.short())
  }
}

/// A frame as it arrives.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Frame {
  Message(Vec<u8>),
  Bytes(Vec<u8>),
  End,
}

impl Channel {
  /// Connects to an agent at `address` and checks that it shows the certificate that `tls` pins. `deadline` bounds
  /// the TCP connect and the handshake, each, and is the deadline of each read and write afterwards.
  ///
  /// On macOS a connect that the Local Network permission refuses goes through the system `nc` instead; see
  /// the `relay` module. The process keeps that decision for the address, so each later connect to it takes the
  /// relay at once.
  pub async fn connect(address: SocketAddr, tls: &ClientTls, deadline: Duration) -> Result<Self, LinkError> {
    let link = link_to(address, deadline).await?;
    Self::connect_over(link, tls, deadline).await
  }

  /// The TLS client handshake over a connection that is open already, such as one of [`link_to`] or a pipe in memory
  /// of a test.
  pub async fn connect_over(link: Link, tls: &ClientTls, deadline: Duration) -> Result<Self, LinkError> {
    let name = ServerName::try_from(SERVER_NAME).map_err(|error| LinkError::Protocol(error.to_string()))?;
    let stream = within_handshake(deadline, TlsConnector::from(Arc::clone(&tls.config)).connect(name, link)).await?;
    // TLS 1.3 ends the handshake of the client before the agent has read the client certificate. A read of the first
    // frame surfaces a refusal there; the peer certificate is known already, which is what both ends check.
    let peer = stream.get_ref().1.peer_certificates().and_then(|certs| certs.first()).map(|cert| Fingerprint::of(cert));
    Self::over(Box::new(stream), peer, deadline)
  }

  /// Takes a TCP connection that the agent accepted, and checks that the client shows a certificate of the allow list
  /// of `tls`. `deadline` bounds the whole handshake, so a client that sends a byte at a time cannot hold it longer,
  /// and is the deadline of each read and write afterwards.
  pub async fn accept(socket: TcpStream, tls: &ServerTls, deadline: Duration) -> Result<Self, LinkError> {
    socket.set_nodelay(true).map_err(LinkError::Io)?;
    Self::accept_over(Box::new(socket), tls, deadline).await
  }

  /// The TLS server handshake over a connection that is open already, such as a pipe in memory of a test.
  pub async fn accept_over(link: Link, tls: &ServerTls, deadline: Duration) -> Result<Self, LinkError> {
    let stream = within_handshake(deadline, TlsAcceptor::from(tls.config()?).accept(link)).await?;
    let peer = stream.get_ref().1.peer_certificates().and_then(|certs| certs.first()).map(|cert| Fingerprint::of(cert));
    Self::over(Box::new(stream), peer, deadline)
  }

  fn over(stream: Box<dyn Io>, peer: Option<Fingerprint>, deadline: Duration) -> Result<Self, LinkError> {
    let peer = peer.ok_or_else(|| LinkError::Untrusted("the other end showed no certificate".to_owned()))?;
    let mut framed = Framed::with_capacity(stream, Frames::new(), CHUNK + HEAD);
    framed.set_backpressure_boundary(CHUNK);
    Ok(Self {
      framed,
      peer,
      read_deadline: deadline,
      write_deadline: deadline,
    })
  }

  /// The fingerprint of the other end, checked by the handshake.
  pub const fn peer(&self) -> Fingerprint {
    self.peer
  }

  /// The deadline of each later read.
  pub const fn set_read_deadline(&mut self, deadline: Duration) {
    self.read_deadline = deadline;
  }

  /// The deadline of each later write.
  pub const fn set_write_deadline(&mut self, deadline: Duration) {
    self.write_deadline = deadline;
  }

  /// The deadline of each write
  pub const fn write_deadline(&self) -> Duration {
    self.write_deadline
  }

  /// Sends one message and flushes.
  pub async fn send<T: Serialize + Sync>(&mut self, message: &T) -> Result<(), LinkError> {
    self.send_all(std::slice::from_ref(message)).await
  }

  /// Sends `messages` one after the other and flushes, so a batch costs one TLS record per 16 KiB and not one per
  /// message. A message over [`MAX_MESSAGE`] sends none of the batch.
  pub async fn send_all<T: Serialize + Sync>(&mut self, messages: &[T]) -> Result<(), LinkError> {
    let encoded = messages.iter().map(message_json).collect::<Result<Vec<_>, _>>()?;
    for json in &encoded {
      self.feed(Out::Message(json)).await?;
    }
    self.flush().await
  }

  /// Adds one message to the buffer without a flush. A later flush, send or read sends it.
  pub async fn queue<T: Serialize + Sync>(&mut self, message: &T) -> Result<(), LinkError> {
    let json = message_json(message)?;
    self.feed(Out::Message(&json)).await
  }

  /// Sends a byte stream as chunks, without a flush. [`Channel::send_end`] completes it.
  pub async fn send_bytes(&mut self, bytes: &[u8]) -> Result<(), LinkError> {
    for chunk in bytes.chunks(CHUNK) {
      self.feed(Out::Bytes(chunk)).await?;
    }
    Ok(())
  }

  /// Sends an empty chunk of a byte stream and flushes it. It keeps the stream alive while its sender has no bytes
  /// yet, such as a client that builds its pack, and the deadline of the reader starts again.
  pub async fn send_keepalive(&mut self) -> Result<(), LinkError> {
    self.feed(Out::Bytes(&[])).await?;
    self.flush().await
  }

  /// Completes a byte stream and flushes.
  pub async fn send_end(&mut self) -> Result<(), LinkError> {
    self.feed(Out::End).await?;
    self.flush().await
  }

  /// Copies all of `reader` as a byte stream, completes it and flushes. Answers the count of bytes. `progress` gets
  /// the count after each chunk. `reader` is a local file or bytes in memory, which a read never holds long
  pub async fn send_stream(&mut self, reader: &mut (dyn Read + Send), progress: &mut (dyn FnMut(u64) + Send)) -> Result<u64, LinkError> {
    let total = self.queue_stream(reader, progress).await?;
    self.flush().await?;
    Ok(total)
  }

  /// Copies all of `reader` as a byte stream and completes it, without a flush. Answers the count of bytes.
  /// `progress` gets the count after each chunk.
  pub async fn queue_stream(&mut self, reader: &mut (dyn Read + Send), progress: &mut (dyn FnMut(u64) + Send)) -> Result<u64, LinkError> {
    let mut buffer = vec![0u8; CHUNK];
    let mut total = 0u64;
    loop {
      let read = match reader.read(&mut buffer) {
        Ok(0) => break,
        Ok(read) => read,
        Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
        Err(error) => return Err(LinkError::Io(error)),
      };
      self.feed(Out::Bytes(&buffer[..read])).await?;
      total += read as u64;
      progress(total);
    }
    self.feed(Out::End).await?;
    Ok(total)
  }

  /// Writes the buffer to TLS, and flushes TLS, within the write deadline.
  pub async fn flush(&mut self) -> Result<(), LinkError> {
    within(self.write_deadline, self.framed.flush()).await
  }

  /// The next frame within the read deadline. The buffer goes out first, so no frame that the other end waits for
  /// stays behind a read.
  pub async fn recv_frame(&mut self) -> Result<Frame, LinkError> {
    if !self.framed.write_buffer().is_empty() {
      self.flush().await?;
    }
    match timeout(self.read_deadline, self.framed.next()).await {
      Err(_) => Err(LinkError::TimedOut),
      Ok(None) => Err(LinkError::Protocol("the other end closed the connection".to_owned())),
      Ok(Some(frame)) => frame,
    }
  }

  /// The next message. A byte frame here is a protocol error.
  pub async fn recv<T: DeserializeOwned>(&mut self) -> Result<T, LinkError> {
    match self.recv_frame().await? {
      Frame::Message(json) => {
        serde_json::from_slice(&json).map_err(|error| LinkError::Protocol(format!("a message that is not in the protocol ({error}): {}", String::from_utf8_lossy(&json))))
      }
      Frame::Bytes(_) | Frame::End => Err(LinkError::Protocol("bytes came where a message was due".to_owned())),
    }
  }

  /// Sends one message and answers the next one, such as a hello and its reply.
  pub async fn exchange<T: Serialize + Sync, R: DeserializeOwned>(&mut self, message: &T) -> Result<R, LinkError> {
    self.send(message).await?;
    self.recv().await
  }

  /// Copies the byte stream that comes next into `writer`, up to its end. Answers the count of bytes. `writer` is a
  /// local file or memory, which a write never holds long
  pub async fn recv_stream(&mut self, writer: &mut (dyn Write + Send)) -> Result<u64, LinkError> {
    let mut total = 0u64;
    loop {
      match self.recv_frame().await? {
        Frame::Bytes(chunk) => {
          writer.write_all(&chunk).map_err(LinkError::Io)?;
          total += chunk.len() as u64;
        }
        Frame::End => return Ok(total),
        Frame::Message(_) => return Err(LinkError::Protocol("a message came inside a byte stream".to_owned())),
      }
    }
  }

  /// Sends the buffer, and ends the connection with a TLS close, within the write deadline.
  pub async fn close(mut self) {
    let deadline = self.write_deadline;
    let _ = timeout(deadline, async {
      let _ = self.framed.flush().await;
      let _ = self.framed.get_mut().shutdown().await;
    })
    .await;
  }

  /// Adds one frame to the buffer within the write deadline; a full buffer goes to TLS first.
  async fn feed(&mut self, frame: Out<'_>) -> Result<(), LinkError> {
    within(self.write_deadline, self.framed.feed(frame)).await
  }

  /// The TLS stream under the frames, for a test that writes bytes outside the frames or counts the writes
  #[cfg(test)]
  pub(crate) fn stream_mut(&mut self) -> &mut Box<dyn Io> {
    self.framed.get_mut()
  }

  /// Puts a stream around the TLS stream, for a test that counts the writes
  #[cfg(test)]
  pub(crate) fn wrap_stream(&mut self, wrap: impl FnOnce(Box<dyn Io>) -> Box<dyn Io>) {
    let stream = std::mem::replace(self.framed.get_mut(), Box::new(tokio::io::empty()));
    *self.framed.get_mut() = wrap(stream);
  }
}

/// `work` within `deadline`: a write that does not end in time is `TimedOut`
async fn within(deadline: Duration, work: impl Future<Output = Result<(), LinkError>>) -> Result<(), LinkError> {
  timeout(deadline, work).await.unwrap_or(Err(LinkError::TimedOut))
}

/// A handshake within `deadline`. A lost connection or a handshake that does not end in time is `Unreachable`
async fn within_handshake<S>(deadline: Duration, handshake: impl Future<Output = io::Result<S>>) -> Result<S, LinkError> {
  match timeout(deadline, handshake).await {
    Ok(Ok(stream)) => Ok(stream),
    Ok(Err(error)) => Err(handshake_error(error)),
    Err(_) => Err(LinkError::Unreachable(io::Error::new(io::ErrorKind::TimedOut, "the handshake did not end in time"))),
  }
}

/// The JSON of `message`, at most [`MAX_MESSAGE`] bytes
fn message_json<T: Serialize>(message: &T) -> Result<Vec<u8>, LinkError> {
  let json = serde_json::to_vec(message).map_err(|error| LinkError::Protocol(error.to_string()))?;
  if json.len() > MAX_MESSAGE {
    return Err(LinkError::Protocol(format!("a message of {} bytes is over the limit of {MAX_MESSAGE}", json.len())));
  }
  Ok(json)
}

/// A frame that the channel sends
#[derive(Clone, Copy, Debug)]
enum Out<'a> {
  Message(&'a [u8]),
  Bytes(&'a [u8]),
  End,
}

/// The codec of the frames: the kind byte, then `LengthDelimitedCodec` for the length and the payload. The reads take
/// the head with the frame, `writes` puts the length before the payload
#[derive(Debug)]
pub(crate) struct Frames {
  reads: LengthDelimitedCodec,
  writes: LengthDelimitedCodec,
}

impl Frames {
  fn new() -> Self {
    Self {
      reads: LengthDelimitedCodec::builder()
        .length_field_offset(1)
        .length_field_type::<u32>()
        .length_adjustment(HEAD.cast_signed())
        .num_skip(0)
        .max_frame_length(MAX_MESSAGE)
        .new_codec(),
      writes: LengthDelimitedCodec::builder().length_field_type::<u32>().max_frame_length(MAX_MESSAGE).new_codec(),
    }
  }
}

impl Decoder for Frames {
  type Item = Frame;
  type Error = LinkError;

  fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Frame>, LinkError> {
    // the head stays in `src` until the whole frame came (`num_skip(0)`), so each call sees the head of the next frame
    let Some(head) = src.get(..HEAD) else {
      return Ok(None);
    };
    let length = u32::from_be_bytes([head[1], head[2], head[3], head[4]]) as usize;
    let limit = match head[0] {
      KIND_MESSAGE => MAX_MESSAGE,
      KIND_BYTES => CHUNK,
      KIND_END => 0,
      kind => return Err(LinkError::Protocol(format!("a frame of the unknown kind {kind}"))),
    };
    if length > limit {
      return Err(LinkError::Protocol(format!("a frame of {length} bytes is over the limit of {limit}")));
    }
    let Some(mut frame) = self.reads.decode(src)? else {
      return Ok(None);
    };
    let payload = frame.split_off(HEAD).to_vec();
    Ok(Some(match frame[0] {
      KIND_MESSAGE => Frame::Message(payload),
      KIND_BYTES => Frame::Bytes(payload),
      _ => Frame::End,
    }))
  }
}

impl Encoder<Out<'_>> for Frames {
  type Error = LinkError;

  fn encode(&mut self, frame: Out<'_>, dst: &mut BytesMut) -> Result<(), LinkError> {
    let (kind, payload): (u8, &[u8]) = match frame {
      Out::Message(json) => (KIND_MESSAGE, json),
      Out::Bytes(chunk) => (KIND_BYTES, chunk),
      Out::End => (KIND_END, &[]),
    };
    dst.put_u8(kind);
    self.writes.encode(payload, dst).map_err(LinkError::Io)
  }
}

/// The connection to `address` within `deadline`: a direct one, or the relay when a direct connect to it got the refusal
/// of the Local Network permission, now or earlier in this process. A failure is `Unreachable`.
pub async fn link_to(address: SocketAddr, deadline: Duration) -> Result<Link, LinkError> {
  let relayed = async |direct: String| match relay::connect(address, deadline).await {
    Ok(relayed) => Ok(Box::new(relayed) as Link),
    Err(relayed) => Err(LinkError::Unreachable(io::Error::new(
      relayed.kind(),
      format!("{direct}; through {}: {relayed}", relay::NC),
    ))),
  };
  if relay::is_remembered(address.ip()) {
    return relayed(format!("an earlier direct connect to {} got the refusal of the Local Network permission", address.ip())).await;
  }
  let connected = timeout(deadline, TcpStream::connect(address))
    .await
    .unwrap_or_else(|_| Err(io::Error::new(io::ErrorKind::TimedOut, format!("no connection within {} ms", deadline.as_millis()))));
  match connected {
    Ok(socket) => {
      socket.set_nodelay(true).map_err(LinkError::Io)?;
      Ok(Box::new(socket))
    }
    Err(error) if relay::is_local_network_refusal(&error) => {
      relay::remember(address.ip());
      relayed(error.to_string()).await
    }
    Err(error) => Err(LinkError::Unreachable(error)),
  }
}

#[expect(clippy::needless_pass_by_value, reason = "a map_err callback takes the error by value")]
fn tls_setup(error: rustls::Error) -> LinkError {
  LinkError::Protocol(format!("cannot set up TLS: {error}"))
}

fn handshake_error(error: io::Error) -> LinkError {
  match LinkError::from_io(error) {
    LinkError::Io(error) if is_connection_loss(&error) => LinkError::Unreachable(error),
    LinkError::TimedOut => LinkError::Unreachable(io::Error::new(io::ErrorKind::TimedOut, "the handshake did not end in time")),
    other => other,
  }
}

fn is_connection_loss(error: &io::Error) -> bool {
  matches!(
    error.kind(),
    io::ErrorKind::ConnectionRefused | io::ErrorKind::ConnectionReset | io::ErrorKind::ConnectionAborted | io::ErrorKind::BrokenPipe
  )
}

/// The signature checks that both verifiers share. Both configurations allow TLS 1.3 only, so a TLS 1.2 signature
/// never arrives.
#[derive(Debug)]
struct Tls13 {
  algorithms: WebPkiSupportedAlgorithms,
}

/// The three signature methods of a verifier of [`Tls13`] in the field `signatures`. `ServerCertVerifier` and
/// `ClientCertVerifier` declare the same three methods, and a trait cannot give both of them a default.
macro_rules! tls13_signatures {
  () => {
    fn verify_tls12_signature(&self, _message: &[u8], _cert: &CertificateDer<'_>, _dss: &DigitallySignedStruct) -> Result<HandshakeSignatureValid, rustls::Error> {
      Err(rustls::Error::General("the link speaks TLS 1.3 only".to_owned()))
    }

    fn verify_tls13_signature(&self, message: &[u8], cert: &CertificateDer<'_>, dss: &DigitallySignedStruct) -> Result<HandshakeSignatureValid, rustls::Error> {
      verify_tls13_signature(message, cert, dss, &self.signatures.algorithms)
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
      self.signatures.algorithms.supported_schemes()
    }
  };
}

/// The verifier of a client: the host must show the pinned certificate.
#[derive(Debug)]
struct PinnedHost {
  host: Fingerprint,
  signatures: Tls13,
}

impl ServerCertVerifier for PinnedHost {
  fn verify_server_cert(
    &self,
    end_entity: &CertificateDer<'_>,
    _intermediates: &[CertificateDer<'_>],
    _server_name: &ServerName<'_>,
    _ocsp_response: &[u8],
    _now: UnixTime,
  ) -> Result<ServerCertVerified, rustls::Error> {
    if Fingerprint::of(end_entity) == self.host {
      Ok(ServerCertVerified::assertion())
    } else {
      Err(rustls::Error::InvalidCertificate(CertificateError::ApplicationVerificationFailure))
    }
  }

  tls13_signatures!();
}

/// The verifier of an agent: the client must show a certificate of the allow list.
#[derive(Debug)]
struct AllowedClients {
  allow: PathBuf,
  signatures: Tls13,
}

impl ClientCertVerifier for AllowedClients {
  fn root_hint_subjects(&self) -> &[DistinguishedName] {
    &[]
  }

  fn verify_client_cert(&self, end_entity: &CertificateDer<'_>, _intermediates: &[CertificateDer<'_>], _now: UnixTime) -> Result<ClientCertVerified, rustls::Error> {
    let allowed = AllowList::read(&self.allow).map_err(|error| rustls::Error::General(error.to_string()))?;
    if allowed.name(&Fingerprint::of(end_entity)).is_some() {
      Ok(ClientCertVerified::assertion())
    } else {
      Err(rustls::Error::InvalidCertificate(CertificateError::ApplicationVerificationFailure))
    }
  }

  tls13_signatures!();
}

#[cfg(test)]
#[path = "channel_test.rs"]
mod channel_test;
