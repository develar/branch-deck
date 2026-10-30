//! A place outside the review that the review plan names (spec/branch/review-plan.spec.md, "Possible Breakage"): a use
//! of a changed API that the change left outside, by its path and line at the head of the range, as the view of its
//! file opens it over the review, the name selected. One `cat-file --batch-check` names the blob, one `cat-file --batch`
//! reads its text, none when the memory of the diffs has it.

use crate::declarations::{outside_file, word_columns};
use crate::model::{BlobRequest, DeclarationTarget, DiffSide, OutsidePlaceParams};
use crate::service::FileDiffs;
use anyhow::Result;
use git_executor::git_command_executor::GitCommandExecutor;
use tracing::instrument;

impl FileDiffs {
  /// The file at `params.path` of `params.revision` as a declaration outside the review, at the line, its first whole
  /// word `params.name` selected (none: the start of the line); `None` when the revision has no text file there or the
  /// line is past its end
  #[instrument(skip(self, git), fields(path = %params.path, line = params.line))]
  pub fn outside_place(&self, git: &GitCommandExecutor, params: &OutsidePlaceParams) -> Result<Option<DeclarationTarget>> {
    let revision = params.revision.as_str();
    let ids = self.outside_ids(git, &params.repository_path, Some(revision), &[params.path.as_str()])?;
    let Some(id) = ids.get(&params.path) else { return Ok(None) };
    let texts = self.texts_by_id(git, &params.repository_path, &[BlobRequest { id: id.clone(), path: None }])?;
    let Some(text) = texts.get(id) else { return Ok(None) };
    let Some(line) = params.line.checked_sub(1).filter(|line| (*line as usize) < text.split('\n').count()) else {
      return Ok(None);
    };
    let column = params.name.as_deref().and_then(|name| word_columns(text, line, name).next());
    let length = params.name.as_deref().map_or(0, |name| u32::try_from(name.encode_utf16().count()).unwrap_or(0));
    Ok(Some(DeclarationTarget {
      path: params.path.clone(),
      side: DiffSide::New,
      line,
      column: column.unwrap_or(0),
      end_column: column.map_or(0, |column| column + length),
      kind: "usage".to_string(),
      container: None,
      outside: Some(outside_file(Some(revision), id, text)),
    }))
  }
}
