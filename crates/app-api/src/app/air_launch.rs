//! Start in Air (spec/branch/start-in-air.spec.md): the agents of the IDE that has the repository open
//! (`air_launch_targets`), and the launch of a session with the prompt of a robot (`air_launch`), over the local launch
//! socket of Air (`air_launch`, protocol v1). The directory of the instance files is the host's (`AppHost::air_instances`):
//! the user's in the app, none in the e2e server, so no test reaches an IDE of the user. A refusal of the IDE and no IDE
//! at all are answers the dialog words; a connection that fails is an error.

use crate::context::AppContext;
use crate::operation::run_blocking;
use air_launch::client::{Client, ClientError};
use air_launch::discovery::{Instance, find_instance};
use air_launch::protocol::{Launch, LaunchRequest, Refusal, Target, Targets};
use domain_types::error::ApiError;
use std::path::{Path, PathBuf};
use tracing::instrument;

/// What `air_launch_targets` asks
#[derive(Debug, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct AirTargetsParams {
  /// The absolute path of the directory the session works in
  pub project: String,
}

/// The IDE that answered
#[derive(Debug, Clone, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct AirIde {
  /// The product code (`IU`)
  pub product: String,
  /// The build (`IU-263.1234`)
  pub build: String,
  /// The base directory of its project that holds the directory
  pub project: String,
}

/// The agents of the IDE of a directory
#[derive(Debug, serde::Serialize, specta::Type)]
#[serde(tag = "status", rename_all = "camelCase")]
pub enum AirTargetsAnswer {
  /// No running IDE with Air has the directory or a parent of it open
  NoIde,
  Listed {
    ide: AirIde,
    targets: Vec<Target>,
  },
  /// The code of the protocol (`DENIED`), and its English text for a log
  Refused {
    ide: AirIde,
    code: String,
    message: Option<String>,
  },
}

/// How a launch went
#[derive(Debug, serde::Serialize, specta::Type)]
#[serde(tag = "status", rename_all = "camelCase")]
pub enum AirLaunchAnswer {
  NoIde,
  /// The session started; `session_id` is the id of its provider, none for a draft
  #[serde(rename_all = "camelCase")]
  Accepted {
    ide: AirIde,
    session_id: Option<String>,
  },
  Refused {
    ide: AirIde,
    code: String,
    message: Option<String>,
  },
}

/// The IDE of `project` among the instance files of the host, with the client of its socket
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

fn exchange_error(error: &ClientError) -> ApiError {
  ApiError::internal(error.to_string())
}

/// The agents Air can start in the IDE that has `params.project` open, with their models and efforts. The IDE may ask
/// the user first, so the answer can take up to `air_launch::client::RESPONSE_TIMEOUT`
#[instrument(skip_all, fields(project = %params.project))]
pub async fn air_launch_targets(ctx: &AppContext, params: AirTargetsParams) -> Result<AirTargetsAnswer, ApiError> {
  let dir: Option<PathBuf> = ctx.host.air_instances();
  run_blocking(move || -> Result<AirTargetsAnswer, ApiError> {
    let Some((ide, client)) = connect(dir.as_deref(), &params.project) else {
      return Ok(AirTargetsAnswer::NoIde);
    };
    Ok(match client.targets(&params.project).map_err(|error| exchange_error(&error))? {
      Targets::Listed { targets } => AirTargetsAnswer::Listed { ide, targets },
      Targets::Refused(Refusal { code, message }) => AirTargetsAnswer::Refused {
        ide,
        code: code.as_str().to_owned(),
        message,
      },
    })
  })
  .await
}

/// Starts a session in the IDE that has `params.project` open, or opens a draft of it; the IDE may ask the user first
#[instrument(skip_all, fields(project = %params.project, target = %params.launch_target_id, start = ?params.start))]
pub async fn air_launch(ctx: &AppContext, params: LaunchRequest) -> Result<AirLaunchAnswer, ApiError> {
  let dir: Option<PathBuf> = ctx.host.air_instances();
  run_blocking(move || -> Result<AirLaunchAnswer, ApiError> {
    let Some((ide, client)) = connect(dir.as_deref(), &params.project) else {
      return Ok(AirLaunchAnswer::NoIde);
    };
    Ok(match client.launch(&params).map_err(|error| exchange_error(&error))? {
      Launch::Accepted { session_id } => AirLaunchAnswer::Accepted { ide, session_id },
      Launch::Refused(Refusal { code, message }) => AirLaunchAnswer::Refused {
        ide,
        code: code.as_str().to_owned(),
        message,
      },
    })
  })
  .await
}
