//! The events of app-api to the connections of the backend (`connections.rs`; openspec/specs/repository/backend-protocol/spec.md,
//! "Events: The events of app-api go to every connection"), over hosts that record what each was told

use crate::connections::{ConnectionHosts, Connections, without};
use crate::events::{SETTINGS_CHANGED_EVENT, activity_changed, settings_changed};
use crate::transport::{AppEvents, ConnectionId, WindowId};
use domain_types::settings::SettingsChange;
use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use std::sync::{Arc, Mutex};

/// The events a host was told: the window of each (none: every window), its name
#[derive(Default)]
struct Told(Mutex<Vec<(Option<String>, String)>>);

impl AppEvents for Told {
  fn emit(&self, event: &str, _payload: Value) {
    self.0.lock().unwrap().push((None, event.to_owned()));
  }

  fn emit_to(&self, window: &WindowId, event: &str, _payload: Value) {
    self.0.lock().unwrap().push((Some(window.to_string()), event.to_owned()));
  }
}

impl Told {
  fn taken(&self) -> Vec<(Option<String>, String)> {
    std::mem::take(&mut self.0.lock().unwrap())
  }
}

fn hosts(told: &Arc<Told>) -> ConnectionHosts {
  ConnectionHosts {
    desktop: without::desktop(),
    events: Arc::clone(told) as Arc<dyn AppEvents>,
  }
}

fn change(key: &str) -> SettingsChange {
  SettingsChange {
    version: 3,
    key: key.to_owned(),
    value: json!(1),
    origin: "main".to_owned(),
  }
}

/// An event goes to every connection, one to a window to the connection of that window, one to the main window to the
/// main window of each; a connection that ended hears nothing
#[test]
fn an_event_of_the_app_goes_to_every_connection_and_one_to_a_window_to_its_connection() {
  let (app, ide, other) = (Arc::new(Told::default()), Arc::new(Told::default()), Arc::new(Told::default()));
  let connections = Connections::new(Arc::clone(&app) as Arc<dyn AppEvents>);
  let (ide_id, other_id) = (connections.open(hosts(&ide)), connections.open(hosts(&other)));
  assert_ne!(ide_id, other_id);
  assert_ne!(ide_id, ConnectionId::APP);

  activity_changed(&connections);
  settings_changed(&connections, &change("appSettings"));
  let every = vec![(None, "activity-changed".to_owned()), (None, SETTINGS_CHANGED_EVENT.to_owned())];
  assert_eq!((app.taken(), ide.taken(), other.taken()), (every.clone(), every.clone(), every));

  settings_changed(&connections, &change("repositoryState"));
  let main = |label: &str| vec![(Some(label.to_owned()), SETTINGS_CHANGED_EVENT.to_owned())];
  assert_eq!(
    (app.taken(), ide.taken(), other.taken()),
    (main("main"), main(&format!("main@{ide_id}")), main(&format!("main@{other_id}")))
  );

  connections.emit_to(&WindowId::of(ide_id, "review:3"), "reveal", Value::Null);
  assert_eq!(ide.taken(), [(Some(format!("review:3@{ide_id}")), "reveal".to_owned())]);
  assert_eq!((app.taken(), other.taken()), (Vec::new(), Vec::new()));

  connections.close(ide_id);
  activity_changed(&connections);
  assert_eq!(ide.taken(), Vec::new(), "a connection that ended hears nothing");
  assert_eq!(other.taken().len(), 1);
}

/// A news is claimed once, of any client, by what it tells: the failure of another robot is first though the session
/// opened anew counts its news from 1 again, and a path with a trailing separator is the same repository
#[test]
fn a_news_is_claimed_once_by_what_it_tells() {
  use domain_types::session::NewsClaim;
  let connections = Connections::new(Arc::new(Told::default()));
  let failed = |robot: &str| NewsClaim::RobotFailed { robot_id: robot.to_owned() };
  assert!(connections.claim_news("/repo", failed("r1")), "the first claim");
  assert!(!connections.claim_news("/repo/", failed("r1")), "a later claim, of the same repository");
  assert!(connections.claim_news("/repo", failed("r2")), "another failure");
  assert!(connections.claim_news("/other", failed("r1")), "another repository");
}
