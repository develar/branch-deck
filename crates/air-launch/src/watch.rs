//! The watch of the rows of an IDE over one connection: the root `subscribe` first (it needs no consent), then
//! `listSessions` page by page (its first call asks the consent), then the root notices one by one, with no timeout,
//! until the IDE closes the connection or a `Stopper` stops it. A notice of another method is skipped.

use crate::client::{ClientError, Connection, Stopper};
use crate::protocol::{ListSessionsParams, ListSessionsResult, RootChange, SessionSummary};
use serde_json::Value;

/// The rows of the root of one IDE and their changes
#[derive(Debug)]
pub struct RootWatch {
  connection: Connection,
  /// A frame that is no notice of the root ended the watch
  ended: bool,
}

impl RootWatch {
  /// Subscribes the root of `connection`
  pub(crate) fn open(mut connection: Connection) -> Result<Self, ClientError> {
    connection.call::<Value>("subscribe", &())?;
    Ok(Self { connection, ended: false })
  }

  /// Every row of the hosts of the IDE, newest first, page by page; the first call asks the consent of the user
  pub fn list_sessions(&mut self) -> Result<Vec<SessionSummary>, ClientError> {
    let mut rows = Vec::new();
    let mut cursor: Option<String> = None;
    loop {
      let page: ListSessionsResult = self.connection.call("listSessions", &ListSessionsParams { cursor: cursor.as_deref() })?;
      rows.extend(page.items);
      match page.next_cursor {
        // a cursor that does not move would ask the same page forever
        Some(next) if cursor.as_ref() != Some(&next) => cursor = Some(next),
        _ => return Ok(rows),
      }
    }
  }

  /// What stops the watch from another thread: the `next` that waits then answers none
  ///
  /// # Errors
  /// The socket cannot be shared
  pub fn stopper(&self) -> Result<Stopper, ClientError> {
    self.connection.stopper()
  }
}

impl Iterator for RootWatch {
  type Item = Result<RootChange, ClientError>;

  /// The next change of the rows; none after a close (also a close of `Stopper::stop`) or an error
  fn next(&mut self) -> Option<Self::Item> {
    while !self.ended {
      let notice = match self.connection.next_notification()? {
        Ok(notice) => notice,
        Err(error) => {
          self.ended = true;
          return Some(Err(error));
        }
      };
      match RootChange::of(&notice) {
        Ok(Some(change)) => return Some(Ok(change)),
        Ok(None) => {}
        Err(reason) => {
          self.ended = true;
          return Some(Err(ClientError::BadResponse(reason)));
        }
      }
    }
    None
  }
}
