//! The watch of a session of Start in Air: what each line of the IDE tells the page (`follow`), and which watches a
//! stop, a newer watch and a page that went away end (`AirWatches`)

use super::{AirSessionStatus, AirWatchEnd, AirWatches, follow};
use crate::transport::{EventSink, SinkGone, WindowId};
use air_launch::client::ClientError;
use air_launch::protocol::{Activity, Refusal, RefusalCode, Session, WatchLine};
use pretty_assertions::assert_eq;
use std::sync::Mutex;

/// The statuses a page heard; `gone`: the page went away, a send fails
#[derive(Default)]
struct Page {
  heard: Mutex<Vec<AirSessionStatus>>,
  gone: bool,
}

impl EventSink<AirSessionStatus> for Page {
  fn send(&self, event: AirSessionStatus) -> Result<(), SinkGone> {
    if self.gone {
      return Err(SinkGone);
    }
    self.heard.lock().map_err(|_poisoned| SinkGone)?.push(event);
    Ok(())
  }
}

#[expect(clippy::unnecessary_wraps, reason = "a line of a watch, as the client yields it")]
fn session(activity: Activity, title: Option<&str>) -> Result<WatchLine, ClientError> {
  Ok(WatchLine::Session {
    session: Session {
      session_id: "s-1".to_owned(),
      title: title.map(str::to_owned),
      activity,
      updated_at: Some(1_000),
    },
  })
}

fn status(activity: Activity, title: Option<&str>) -> AirSessionStatus {
  AirSessionStatus {
    title: title.map(str::to_owned),
    activity,
  }
}

#[test]
fn a_watch_tells_the_page_each_status_then_how_it_ended() {
  let page = Page::default();
  let lines = vec![
    session(Activity::Processing, Some("Fix it")),
    session(Activity::NeedsInput, None),
    Ok(WatchLine::End { reason: "timeout".to_owned() }),
  ];
  let end = follow(lines.into_iter(), &|| false, &page).unwrap();
  assert_eq!(end, AirWatchEnd::Ended { reason: "timeout".to_owned() });
  assert_eq!(
    *page.heard.lock().unwrap(),
    vec![status(Activity::Processing, Some("Fix it")), status(Activity::NeedsInput, None)]
  );

  let refused = Ok(WatchLine::Refused(Refusal {
    code: RefusalCode::NoSession,
    message: Some("gone".to_owned()),
  }));
  let end = follow(vec![session(Activity::Unread, None), refused].into_iter(), &|| false, &Page::default()).unwrap();
  assert_eq!(
    end,
    AirWatchEnd::Refused {
      code: "NO_SESSION".to_owned(),
      message: Some("gone".to_owned())
    }
  );
}

#[test]
fn a_close_is_closed_unless_the_page_stopped_the_watch() {
  let lines = || vec![session(Activity::Processing, None)].into_iter();
  assert_eq!(follow(lines(), &|| false, &Page::default()).unwrap(), AirWatchEnd::Closed);
  assert_eq!(follow(lines(), &|| true, &Page::default()).unwrap(), AirWatchEnd::Stopped);
  // a read the stop broke is no error
  let broken = || vec![Err(ClientError::Closed)].into_iter();
  assert_eq!(follow(broken(), &|| true, &Page::default()).unwrap(), AirWatchEnd::Stopped);
  follow(broken(), &|| false, &Page::default()).unwrap_err();
}

#[test]
fn a_page_that_went_away_stops_the_watch_at_its_first_status() {
  let page = Page { gone: true, ..Page::default() };
  let lines = vec![session(Activity::Processing, None), session(Activity::Ready, None)];
  assert_eq!(follow(lines.into_iter(), &|| false, &page).unwrap(), AirWatchEnd::Stopped);
}

/// A watch connected to a socket nobody answers on: its `next` waits until a stop
#[cfg(unix)]
fn waiting_watch(dir: &std::path::Path, name: &str) -> (air_launch::client::Watch, std::os::unix::net::UnixListener) {
  let socket = dir.join(format!("{name}.sock"));
  let listener = std::os::unix::net::UnixListener::bind(&socket).unwrap();
  let watch = air_launch::client::Client::new(&socket).watch("/work/idea", name).unwrap();
  (watch, listener)
}

#[cfg(unix)]
#[test]
fn a_stop_a_newer_watch_of_the_session_and_a_page_that_went_away_end_the_watches_of_a_window() {
  let dir = tempfile::Builder::new().prefix("air").tempdir().unwrap();
  let watches = AirWatches::default();
  let main = WindowId::main();
  let review = WindowId::new("review-1");

  // the page stops its watch
  let (mut stopped, _listener) = waiting_watch(dir.path(), "s-1");
  let running = watches.start(&main, "s-1", stopped.stopper().unwrap());
  watches.stop(&main, "s-1".to_owned());
  assert!(running.stopped.load(std::sync::atomic::Ordering::SeqCst));
  assert!(stopped.next().is_none());

  // a newer watch of the same session stops the one before; the end of the older one keeps the newer
  let (mut older, _older_listener) = waiting_watch(dir.path(), "s-2");
  let (newer, _newer_listener) = waiting_watch(dir.path(), "s-3");
  let first = watches.start(&main, "s-2", older.stopper().unwrap());
  let second = watches.start(&main, "s-2", newer.stopper().unwrap());
  assert!(older.next().is_none());
  watches.end(&main, "s-2", first.ticket);
  assert!(!second.stopped.load(std::sync::atomic::Ordering::SeqCst));

  // the page of the main window goes away: its watches stop, those of another window go on
  let (other, _other_listener) = waiting_watch(dir.path(), "s-4");
  let elsewhere = watches.start(&review, "s-4", other.stopper().unwrap());
  watches.forget_window(&main);
  assert!(second.stopped.load(std::sync::atomic::Ordering::SeqCst));
  assert!(!elsewhere.stopped.load(std::sync::atomic::Ordering::SeqCst));
}
