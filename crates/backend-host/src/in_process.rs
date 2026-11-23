//! The in-process transport (docs/design/backend-protocol.md, "Architecture"): a client in the process of the backend
//! (the Tauri shell; the bridge of the IDE that runs its own backend) sends each message as the text of its line and
//! reads the frames of its connection as they are, the bytes of a raw answer too. Its drop ends the connection.

use crate::connection::Connection;
use crate::host::Host;
use crate::wire::Frame;
use serde_json::Value;
use std::sync::Arc;

/// A client of the host in its process
pub struct InProcess {
  connection: Arc<Connection>,
}

impl InProcess {
  /// A client of its own (`Host::connect`)
  pub fn client(host: &Host) -> Self {
    Self { connection: host.connect() }
  }

  /// The connection of the app's own windows (`Host::connect_app`)
  pub fn app(host: &Host) -> Self {
    Self { connection: host.connect_app() }
  }

  /// Sends `message`
  pub fn send(&self, message: &Value) {
    self.connection.receive(&message.to_string());
  }

  /// Sends the line `line` as it is
  pub fn send_line(&self, line: &str) {
    self.connection.receive(line);
  }

  /// The next frame of the host, none once the connection ended
  pub async fn next(&self) -> Option<Frame> {
    self.connection.next_frame().await
  }

  pub const fn connection(&self) -> &Arc<Connection> {
    &self.connection
  }
}

impl Drop for InProcess {
  fn drop(&mut self) {
    self.connection.close();
  }
}
