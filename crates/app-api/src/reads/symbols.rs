//! The symbol searches of a review, Go to Declaration and Find Usages (`file_diffs::declarations`, `file_diffs::usages`):
//! in the files of the review from the blobs the diffs read (`AppContext::file_diffs`), then outside the review in the
//! index of the repository the host has (`SymbolIndexes`), waited for as long as it runs. One search per window
//! (`Windows::begin_search`): a newer one of the window, `cancel_symbol_search` of it or its close supersedes the one in
//! flight, whose query of the index is killed, and it answers `canceled`

use crate::context::AppContext;
use crate::operation::{operation, run_blocking};
use crate::transport::Call;
use domain_types::error::ApiError;
use file_diffs::accio::{AccioCli, SymbolIndex};
use file_diffs::model::{DeclarationTarget, FindDeclarationParams, FoundDeclarations, FoundUsages, OutsidePlaceParams};
use std::sync::Arc;
use tracing::instrument;

/// Where a symbol search looks outside the review: the index of this machine in the app (`Accio`), the fixture of a
/// test repository in the e2e server
pub trait SymbolIndexes: Send + Sync {
  /// The index of one search, whose queries are killed once `canceled` turns true; None when there is none
  fn search<'a>(&'a self, canceled: &'a (dyn Fn() -> bool + Sync)) -> Option<Box<dyn SymbolIndex + 'a>>;
}

/// accio, when this machine has it (`AccioCli::discover`), and the repository an index
pub struct Accio;

impl SymbolIndexes for Accio {
  fn search<'a>(&'a self, canceled: &'a (dyn Fn() -> bool + Sync)) -> Option<Box<dyn SymbolIndex + 'a>> {
    AccioCli::discover().map(|accio| Box::new(accio.search(canceled)) as Box<dyn SymbolIndex + 'a>)
  }
}

/// The declarations of the name at `params.site` (`FileDiffs::find_declaration`), as the search of the window of `call`
#[instrument(skip_all, fields(path = %params.site.path, line = params.site.line))]
pub async fn find_declaration(ctx: &AppContext, call: &Call, params: FindDeclarationParams) -> Result<FoundDeclarations, ApiError> {
  let search = ctx.windows.begin_search(&call.window);
  let (git, repository_state, file_diffs, indexes) = (ctx.git.clone(), ctx.repository_state.clone(), ctx.file_diffs.clone(), Arc::clone(&ctx.symbol_indexes));
  run_blocking(move || {
    let cache = repository_state.object_cache(&git, &params.repository_path);
    let canceled = || search.is_superseded();
    let index = indexes.search(&canceled);
    file_diffs.find_declaration(&git, &cache, index.as_deref(), &params)
  })
  .await
}

/// The usages of the name at `params.site` (`FileDiffs::find_usages`), as the search of the window of `call`
#[instrument(skip_all, fields(path = %params.site.path, line = params.site.line))]
pub async fn find_usages(ctx: &AppContext, call: &Call, params: FindDeclarationParams) -> Result<FoundUsages, ApiError> {
  let search = ctx.windows.begin_search(&call.window);
  let (git, file_diffs, indexes) = (ctx.git.clone(), ctx.file_diffs.clone(), Arc::clone(&ctx.symbol_indexes));
  run_blocking(move || {
    let canceled = || search.is_superseded();
    let index = indexes.search(&canceled);
    file_diffs.find_usages(&git, index.as_deref(), &params)
  })
  .await
}

/// A place outside the review at a revision (`FileDiffs::outside_place`): no index, two `cat-file` at most
#[instrument(skip_all, fields(path = %params.path, line = params.line))]
pub async fn get_outside_place(ctx: &AppContext, params: OutsidePlaceParams) -> Result<Option<DeclarationTarget>, ApiError> {
  let (git, file_diffs) = (ctx.git.clone(), ctx.file_diffs.clone());
  run_blocking(move || file_diffs.outside_place(&git, &params)).await
}

/// Supersedes the search of the window of `call`
#[instrument(skip_all, fields(window = %call.window))]
pub async fn cancel_symbol_search(ctx: &AppContext, call: &Call) -> Result<(), ApiError> {
  operation(async move {
    ctx.windows.cancel_search(&call.window);
    Ok(())
  })
  .await
}
