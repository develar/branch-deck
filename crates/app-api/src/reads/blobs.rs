//! The blobs of the diff views by id: their texts and their highlighting, mostly from the blobs the diffs read
//! (`AppContext::file_diffs`)

use crate::context::AppContext;
use crate::operation::run_blocking;
use domain_types::error::ApiError;
use file_diffs::model::{BlobText, GetBlobSyntaxParams, GetBlobTextsParams};
use tracing::instrument;

/// The texts of the blobs (`FileDiffs::blob_texts`)
#[instrument(skip_all, fields(repository_path = %params.repository_path, blobs = params.blobs.len()))]
pub async fn get_blob_texts(ctx: &AppContext, params: GetBlobTextsParams) -> Result<Vec<BlobText>, ApiError> {
  let (git, file_diffs) = (ctx.git.clone(), ctx.file_diffs.clone());
  run_blocking(move || file_diffs.blob_texts(&git, &params.repository_path, &params.blobs)).await
}

/// The highlighting of the lines of one blob as raw bytes (`FileDiffs::blob_syntax`)
#[instrument(skip_all, fields(id = %params.blob.id, path = %params.file_path))]
pub async fn get_blob_syntax(ctx: &AppContext, params: GetBlobSyntaxParams) -> Result<Vec<u8>, ApiError> {
  let (git, file_diffs) = (ctx.git.clone(), ctx.file_diffs.clone());
  run_blocking(move || file_diffs.blob_syntax(&git, &params)).await
}
