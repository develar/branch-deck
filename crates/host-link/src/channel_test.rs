use super::*;
use crate::proto::{Reply, Request};
use pretty_assertions::assert_eq;
use std::pin::Pin;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::task::{Context, Poll};
use tokio::io::ReadBuf;
use tokio::task::JoinHandle;
use tokio::time::Instant;

/// The deadline of each connect and read of a test that is not about a deadline: no run reaches it, and the tests on
/// paused time never wait for it
const DEADLINE: Duration = Duration::from_mins(1);

/// How many bytes a pipe in memory holds before a write waits for the reader
const PIPE: usize = 256 * 1024;

/// An agent whose connections are pipes in memory, so a test on paused time waits for no socket of the system: a
/// connect over loopback TCP completes in the kernel, and a paused clock would run past its deadline meanwhile
struct Agent {
  identity: Identity,
  tls: ServerTls,
  allow: PathBuf,
  _dir: tempfile::TempDir,
}

/// A connection of `client` over `link` that pins `host`.
async fn connect(link: Link, host: Fingerprint, client: &Identity, deadline: Duration) -> Result<Channel, LinkError> {
  Channel::connect_over(link, &ClientTls::new(host, client)?, deadline).await
}

fn agent(clients: &[&Identity]) -> Agent {
  let dir = tempfile::tempdir().expect("a temporary directory");
  let allow = dir.path().join("clients");
  let mut list = AllowList::default();
  for client in clients {
    list.add(client.fingerprint(), "test").expect("a name");
  }
  std::fs::write(&allow, list.text()).expect("the allow list");
  let identity = Identity::generate().expect("the agent identity");
  Agent {
    tls: ServerTls::new(&identity, allow.clone()).expect("the agent TLS"),
    identity,
    allow,
    _dir: dir,
  }
}

/// A client, and an agent that allows it
fn allowed() -> (Identity, Agent) {
  let client = Identity::generate().expect("the client identity");
  let agent = agent(&[&client]);
  (client, agent)
}

impl Agent {
  /// Accepts one connection on a task and runs `serve` on it, which gives the channel back for its close. Answers the
  /// end of the client, and the handle that answers the result of the handshake.
  fn serve<F, S>(&self, serve: S) -> (Link, JoinHandle<Result<Fingerprint, String>>)
  where
    S: FnOnce(Channel) -> F + Send + 'static,
    F: Future<Output = Channel> + Send,
  {
    let (client, agent) = tokio::io::duplex(PIPE);
    let tls = self.tls.clone();
    let served = tokio::spawn(async move {
      let channel = Channel::accept_over(Box::new(agent), &tls, DEADLINE).await.map_err(|error| error.to_string())?;
      let peer = channel.peer();
      serve(channel).await.close().await;
      Ok(peer)
    });
    (Box::new(client), served)
  }
}

/// Reads the hello of a test client.
async fn expect_hello(channel: &mut Channel) {
  let hello: Request = channel.recv().await.expect("the hello");
  assert_eq!(hello, Request::hello("test"));
}

/// Reads the hello of a test client and answers `Packed`.
async fn answer_hello(mut channel: Channel) -> Channel {
  expect_hello(&mut channel).await;
  channel.send(&Reply::Packed).await.expect("the reply");
  channel
}

#[tokio::test(start_paused = true)]
async fn a_pinned_client_and_an_allowed_client_exchange_messages_and_bytes() {
  let (client, agent) = allowed();
  let (link, served) = agent.serve(|mut channel| async move {
    expect_hello(&mut channel).await;
    let mut received = Vec::new();
    assert_eq!(channel.recv_stream(&mut received).await.expect("the bytes"), 200_000);
    assert!(received.iter().enumerate().all(|(index, byte)| u64::from(*byte) == index as u64 % 251));
    let done = Reply::Done {
      message: "both in one write".to_owned(),
    };
    channel.send_all(&[Reply::Packed, done]).await.expect("the replies");
    channel
  });
  let mut channel = connect(link, agent.identity.fingerprint(), &client, DEADLINE).await.expect("connected");
  assert_eq!(channel.peer(), agent.identity.fingerprint());
  channel.send(&Request::hello("test")).await.expect("sent");
  let bytes: Vec<u8> = (0..200_000u32).map(|index| u8::try_from(index % 251).expect("below 251")).collect();
  let mut chunks = 0;
  let sent = channel.send_stream(&mut bytes.as_slice(), &mut |_| chunks += 1).await.expect("streamed");
  assert_eq!(sent, 200_000);
  assert_eq!(chunks, 200_000_usize.div_ceil(CHUNK));
  assert_eq!(channel.recv::<Reply>().await.expect("the reply"), Reply::Packed);
  assert_eq!(
    channel.recv::<Reply>().await.expect("the second reply"),
    Reply::Done {
      message: "both in one write".to_owned()
    }
  );
  assert_eq!(served.await.expect("the agent task"), Ok(client.fingerprint()));
}

/// A TLS stream that counts the writes of the channel: the writes of the buffer between two flushes count as one, so a
/// write that TLS takes in parts is still one write of the channel.
struct CountedWrites {
  inner: Box<dyn Io>,
  writes: Arc<AtomicUsize>,
  wrote: bool,
}

impl AsyncRead for CountedWrites {
  fn poll_read(mut self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &mut ReadBuf<'_>) -> Poll<io::Result<()>> {
    Pin::new(&mut self.inner).poll_read(cx, buf)
  }
}

impl AsyncWrite for CountedWrites {
  fn poll_write(mut self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &[u8]) -> Poll<io::Result<usize>> {
    self.wrote = true;
    Pin::new(&mut self.inner).poll_write(cx, buf)
  }

  fn poll_write_vectored(mut self: Pin<&mut Self>, cx: &mut Context<'_>, bufs: &[io::IoSlice<'_>]) -> Poll<io::Result<usize>> {
    self.wrote = true;
    Pin::new(&mut self.inner).poll_write_vectored(cx, bufs)
  }

  fn is_write_vectored(&self) -> bool {
    self.inner.is_write_vectored()
  }

  fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
    if std::mem::take(&mut self.wrote) {
      self.writes.fetch_add(1, Ordering::SeqCst);
    }
    Pin::new(&mut self.inner).poll_flush(cx)
  }

  fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
    Pin::new(&mut self.inner).poll_shutdown(cx)
  }
}

/// `channel` with a count of its TLS writes.
fn count_writes(channel: &mut Channel) -> Arc<AtomicUsize> {
  let writes = Arc::new(AtomicUsize::new(0));
  let counted = Arc::clone(&writes);
  channel.wrap_stream(move |inner| {
    Box::new(CountedWrites {
      inner,
      writes: counted,
      wrote: false,
    })
  });
  writes
}

/// Many small files of a fetch share one TLS write, a full buffer writes at once, and a read sends the buffer first.
#[tokio::test(start_paused = true)]
async fn the_frames_go_to_tls_in_one_write_per_full_buffer_and_at_a_flush() {
  let (client, agent) = allowed();
  let (link, served) = agent.serve(|mut channel| async move {
    expect_hello(&mut channel).await;
    let mut files = 0;
    while let Ok(Reply::File { size, .. }) = channel.recv::<Reply>().await {
      let mut bytes = Vec::new();
      assert_eq!(channel.recv_stream(&mut bytes).await.expect("the bytes"), size);
      files += 1;
    }
    let mut received = Vec::new();
    assert_eq!(channel.recv_stream(&mut received).await.expect("the big stream"), 200_000);
    channel.send(&Reply::Fetched { files, bytes: 0 }).await.expect("the counts");
    let asked: Request = channel.recv().await.expect("a request after the counts");
    assert_eq!(asked, Request::Runs);
    channel.send(&Reply::Packed).await.expect("the answer");
    channel
  });
  let mut channel = connect(link, agent.identity.fingerprint(), &client, DEADLINE).await.expect("connected");
  let writes = count_writes(&mut channel);
  channel.queue(&Request::hello("test")).await.expect("queued");
  for index in 0..200 {
    let text = format!("file {index}\n");
    channel
      .queue(&Reply::File {
        path: format!("out/{index}.txt"),
        size: text.len() as u64,
        executable: false,
        kind: crate::proto::FileKind::Staged,
      })
      .await
      .expect("queued");
    channel.queue_stream(&mut text.as_bytes(), &mut |_| {}).await.expect("queued");
  }
  assert_eq!(writes.load(Ordering::SeqCst), 0, "the buffer holds less than a chunk");
  // A message where a file is due ends the files of the agent.
  channel.queue(&Reply::Packed).await.expect("queued");
  channel.flush().await.expect("flushed");
  assert_eq!(writes.load(Ordering::SeqCst), 1, "the hello, 200 files and their ends in one write");

  let bytes = vec![7u8; 200_000];
  channel.send_stream(&mut bytes.as_slice(), &mut |_| {}).await.expect("streamed");
  assert_eq!(
    writes.load(Ordering::SeqCst),
    1 + 200_000_usize.div_ceil(CHUNK),
    "one write per full chunk, and the last chunk with its end at the flush"
  );
  assert_eq!(channel.recv::<Reply>().await.expect("the counts"), Reply::Fetched { files: 200, bytes: 0 });

  channel.queue(&Request::Runs).await.expect("queued");
  assert_eq!(channel.recv::<Reply>().await.expect("the answer"), Reply::Packed, "the read sent the queued request");
  assert_eq!(served.await.expect("the agent task"), Ok(client.fingerprint()));
}

/// Both ends keep their TLS configurations between connections, and neither resumes a session. So a client that the
/// allow list no longer names gets no second connection with the same configurations.
#[tokio::test(start_paused = true)]
async fn a_kept_configuration_resumes_no_session_and_a_removed_client_is_refused() {
  let (client, agent) = allowed();
  let tls = ClientTls::new(agent.identity.fingerprint(), &client).expect("the client TLS");
  let exchange = async |link: Link| Channel::connect_over(link, &tls, DEADLINE).await?.exchange::<_, Reply>(&Request::hello("test")).await;
  let (link, served) = agent.serve(answer_hello);
  assert_eq!(exchange(link).await.expect("the first connection"), Reply::Packed);
  assert_eq!(served.await.expect("the agent task"), Ok(client.fingerprint()));

  std::fs::write(&agent.allow, AllowList::default().text()).expect("an empty allow list");
  let (link, served) = agent.serve(answer_hello);
  let refused = exchange(link).await;
  assert!(matches!(refused, Err(LinkError::Untrusted(_) | LinkError::Protocol(_))), "{refused:?}");
  let handshake = served.await.expect("the agent task");
  assert!(handshake.is_err(), "{handshake:?}");
}

#[tokio::test(start_paused = true)]
async fn a_host_with_another_certificate_is_untrusted() {
  let (client, agent) = allowed();
  let (link, _served) = agent.serve(async |channel| channel);
  let impostor = Identity::generate().expect("another identity").fingerprint();
  let refused = connect(link, impostor, &client, DEADLINE).await.expect_err("refused");
  assert!(matches!(refused, LinkError::Untrusted(_)), "{refused:?}");
  assert_eq!(refused.code(), Code::Untrusted);
}

#[tokio::test(start_paused = true)]
async fn a_client_outside_the_allow_list_gets_no_reply() {
  let allowed = Identity::generate().expect("an allowed identity");
  let stranger = Identity::generate().expect("a stranger");
  let agent = agent(&[&allowed]);
  let (link, served) = agent.serve(async |_: Channel| -> Channel { panic!("the agent served a stranger") });
  // TLS 1.3 ends the handshake of the client before the agent checks its certificate, so the refusal arrives at the
  // first read.
  let outcome = match connect(link, agent.identity.fingerprint(), &stranger, DEADLINE).await {
    Ok(mut channel) => channel.exchange::<_, Reply>(&Request::hello("test")).await,
    Err(error) => Err(error),
  };
  assert!(matches!(outcome, Err(LinkError::Untrusted(_) | LinkError::Protocol(_))), "{outcome:?}");
  let handshake = served.await.expect("the agent task");
  assert!(handshake.is_err(), "{handshake:?}");
}

/// A port without a listener refuses the TCP connect at once, so the test needs no deadline and runs in real time
#[tokio::test]
async fn a_port_without_an_agent_is_unreachable() {
  let client = Identity::generate().expect("the client identity");
  let closed = std::net::TcpListener::bind("127.0.0.1:0").expect("a port").local_addr().expect("the address");
  let tls = ClientTls::new(client.fingerprint(), &client).expect("the client TLS");
  let refused = Channel::connect(closed, &tls, DEADLINE).await.expect_err("refused");
  assert!(matches!(refused, LinkError::Unreachable(_)), "{refused:?}");
  assert_eq!(refused.code(), Code::Unreachable);
  assert_eq!(LinkError::TimedOut.code(), Code::TimedOut);
}

/// The deadline of the handshake, on paused time: an agent that never answers the handshake ends it exactly at its
/// deadline
#[tokio::test(start_paused = true)]
async fn a_silent_agent_cuts_the_handshake_at_its_deadline() {
  let client = Identity::generate().expect("the client identity");
  let (link, _silent) = tokio::io::duplex(PIPE);
  let started = Instant::now();
  let timed_out = connect(Box::new(link), client.fingerprint(), &client, Duration::from_millis(300))
    .await
    .expect_err("timed out");
  assert!(matches!(timed_out, LinkError::Unreachable(_)), "{timed_out:?}");
  assert_eq!(started.elapsed(), Duration::from_millis(300));
}

/// The read deadline, on paused time: an agent that says nothing after the handshake ends the read at its deadline with
/// `timed_out`
#[tokio::test(start_paused = true)]
async fn a_read_of_a_silent_agent_ends_at_its_deadline() {
  let (client, agent) = allowed();
  let (said, silent_until) = tokio::sync::oneshot::channel::<()>();
  let (link, served) = agent.serve(async move |channel| {
    let _ = silent_until.await;
    channel
  });
  let mut channel = connect(link, agent.identity.fingerprint(), &client, DEADLINE).await.expect("connected");
  channel.set_read_deadline(Duration::from_secs(120));
  let started = Instant::now();
  let read = channel.recv::<Reply>().await;
  assert!(matches!(read, Err(LinkError::TimedOut)), "{read:?}");
  assert_eq!(started.elapsed(), Duration::from_secs(120));
  let _ = said.send(());
  let _ = served.await;
}

/// A frame over the limit of its kind, and a frame of a kind that the protocol does not have, end the read with a protocol
/// error, which is the refusal `link`
#[tokio::test(start_paused = true)]
async fn a_frame_over_its_limit_or_of_an_unknown_kind_is_a_protocol_error() {
  let (client, agent) = allowed();
  let over = |limit: usize| u32::try_from(limit + 1).expect("small");
  for (kind, length, said) in [
    (KIND_BYTES, over(CHUNK), "a frame of 65537 bytes is over the limit of 65536"),
    (KIND_MESSAGE, over(MAX_MESSAGE), "a frame of 1048577 bytes is over the limit of 1048576"),
    (KIND_END, 1, "a frame of 1 bytes is over the limit of 0"),
    (3, 0, "a frame of the unknown kind 3"),
  ] {
    let (link, served) = agent.serve(async move |mut channel| {
      let outcome = channel.recv_frame().await.expect_err("a protocol error");
      assert!(matches!(&outcome, LinkError::Protocol(message) if message == said), "{outcome:?}");
      assert_eq!(outcome.code(), Code::Link);
      channel
    });
    let mut channel = connect(link, agent.identity.fingerprint(), &client, DEADLINE).await.expect("connected");
    let mut head = [kind, 0, 0, 0, 0];
    head[1..].copy_from_slice(&length.to_be_bytes());
    channel.stream_mut().write_all(&head).await.expect("written");
    channel.stream_mut().flush().await.expect("flushed");
    served.await.expect("the agent task").expect("the handshake");
  }
}

/// The handshake and the frames through the `nc` relay, which a client on macOS takes when the Local Network permission
/// refuses its own connect. The real `nc` connects to a TCP agent, so the test runs in real time
#[cfg(target_os = "macos")]
#[tokio::test]
async fn a_relayed_connection_carries_the_handshake_and_the_frames() {
  let (client, agent) = allowed();
  let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.expect("a port");
  let address = listener.local_addr().expect("the address");
  let tls = agent.tls.clone();
  let served = tokio::spawn(async move {
    let (socket, _) = listener.accept().await.expect("the relayed connection");
    let channel = Channel::accept(socket, &tls, DEADLINE).await.expect("the handshake");
    let peer = channel.peer();
    answer_hello(channel).await.close().await;
    peer
  });
  let relayed = relay::connect(address, DEADLINE).await.expect("relayed");
  let tls = ClientTls::new(agent.identity.fingerprint(), &client).expect("the client TLS");
  let mut channel = Channel::connect_over(Box::new(relayed), &tls, DEADLINE).await.expect("connected");
  assert_eq!(channel.exchange::<_, Reply>(&Request::hello("test")).await.expect("the reply"), Reply::Packed);
  assert_eq!(served.await.expect("the agent task"), client.fingerprint());
}

/// A connect to an address whose direct connect got the refusal of the Local Network permission takes the relay at
/// once. The address is `::1`, which the other tests do not use, because the process keeps the decision.
#[cfg(target_os = "macos")]
#[tokio::test]
async fn a_remembered_refusal_of_an_address_takes_the_relay_at_once() {
  let client = Identity::generate().expect("the client identity");
  let tls = ClientTls::new(client.fingerprint(), &client).expect("the client TLS");
  let closed = std::net::TcpListener::bind("[::1]:0").expect("a port").local_addr().expect("the address");
  let direct = Channel::connect(closed, &tls, DEADLINE).await.expect_err("nothing listens");
  assert!(
    matches!(&direct, LinkError::Unreachable(error) if error.kind() == io::ErrorKind::ConnectionRefused),
    "{direct:?}"
  );

  relay::remember(closed.ip());
  let relayed = Channel::connect(closed, &tls, DEADLINE).await.expect_err("nothing listens");
  assert!(
    relayed
      .to_string()
      .starts_with("no connection: an earlier direct connect to ::1 got the refusal of the Local Network permission; through /usr/bin/nc: nc: "),
    "{relayed}"
  );
  assert!(!relay::is_remembered("127.0.0.1".parse().expect("an address")));
}
