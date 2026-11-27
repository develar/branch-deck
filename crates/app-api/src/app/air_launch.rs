//! Start in Air (openspec/specs/branch/start-in-air/spec.md): the New Session tab with the prompt of a robot in the IDE
//! that has the repository open (`air_open_draft`), over the local AHP socket of Air as a local app (`air_launch`). The
//! IDE takes the default agent of its New Session button, and the user picks the rest in the tab. The directory of the
//! instance files is the host's (`AppHost::air_instances`): the user's in the app, none in the e2e server, so no test
//! reaches an IDE of the user. A refusal of the IDE and no IDE at all are answers the page words; a connection that fails
//! is an error.

use crate::context::AppContext;
use crate::operation::run_blocking;
use air_launch::client::{Client, ClientError};
use air_launch::discovery::{Instance, find_instance};
use air_launch::protocol::Refusal;
use domain_types::error::ApiError;
use std::path::{Path, PathBuf};
use tracing::instrument;

/// What `air_open_draft` asks
#[derive(Debug, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct AirDraftParams {
  /// The absolute path of the directory the session works in
  pub project: String,
  /// The text of the New Session tab
  pub prompt: String,
}

/// The IDE that answered
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct AirIde {
  /// The product code (`IU`)
  pub product: String,
  /// The build (`IU-263.1234`)
  pub build: String,
  /// The launch directory of the IDE that holds the directory (its host): the directory or one of its parents
  pub project: String,
}

/// How the open of the New Session tab went
#[derive(Debug, PartialEq, Eq, serde::Serialize, specta::Type)]
#[serde(tag = "status", rename_all = "camelCase")]
pub enum AirDraftAnswer {
  /// No running IDE with Air has the directory or a parent of it open
  NoIde,
  /// The IDE opened the New Session tab with the prompt
  Opened { ide: AirIde },
  /// The code of the refusal (`DENIED`, `error.data.code`), and the English text of the IDE for a log
  Refused { ide: AirIde, code: String, message: Option<String> },
}

/// The IDE of `project` among the instance files of the host, asked for their hosts, with the client of its socket
fn connect(dir: Option<&Path>, project: &str) -> Option<(AirIde, Client)> {
  let (instance, base) = find_instance(dir?, Path::new(project))?;
  let Instance { product, build, socket, .. } = instance;
  let ide = AirIde {
    product,
    build,
    project: base.to_string_lossy().into_owned(),
  };
  Some((ide, Client::new(socket)))
}

/// Opens the New Session tab of `params.project` with `params.prompt` in the IDE among the instance files of `dir`
pub(crate) fn open_draft(dir: Option<&Path>, params: &AirDraftParams) -> Result<AirDraftAnswer, ApiError> {
  let Some((ide, client)) = connect(dir, &params.project) else {
    return Ok(AirDraftAnswer::NoIde);
  };
  match client.open_draft(&params.project, &params.prompt) {
    Ok(()) => Ok(AirDraftAnswer::Opened { ide }),
    Err(ClientError::Refused(Refusal { code, message, .. })) => Ok(AirDraftAnswer::Refused { ide, code, message }),
    Err(error) => Err(ApiError::internal(error.to_string())),
  }
}

/// Opens the New Session tab with the prompt in the IDE that has `params.project` open, with the default agent of its
/// New Session button. The IDE may ask the user first, so the answer can take up to `air_launch::client::RESPONSE_TIMEOUT`
#[instrument(skip_all, fields(project = %params.project))]
pub async fn air_open_draft(ctx: &AppContext, params: AirDraftParams) -> Result<AirDraftAnswer, ApiError> {
  let dir: Option<PathBuf> = ctx.host.air_instances();
  run_blocking(move || open_draft(dir.as_deref(), &params)).await
}

#[cfg(test)]
#[path = "air_launch_test.rs"]
mod air_launch_test;
