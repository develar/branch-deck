//! The grammar of what the user types, answered by Rust so the page keeps none of its own: the check of a typed branch
//! name and the grammar of a commit body (the grammar the body of a pull request is written from). No git, no state

use domain_types::branch_names::{BranchNameCheck, ValidateBranchNameParams, check_branch_name};
use domain_types::commit::CommitBodyParams;
use domain_types::commit_body::CommitBody;
use sync_utils::commit_body::message_body;
use tracing::instrument;

/// `domain_types::branch_names::check_branch_name`
#[instrument(skip_all)]
#[expect(clippy::needless_pass_by_value, reason = "the registry hands a body its arguments by value")]
pub fn validate_branch_name(params: ValidateBranchNameParams) -> BranchNameCheck {
  check_branch_name(&params.name, params.rules)
}

/// `sync_utils::commit_body::commit_body` of the lines after the subject line
#[instrument(skip_all)]
#[expect(clippy::needless_pass_by_value, reason = "the registry hands a body its arguments by value")]
pub fn commit_body(params: CommitBodyParams) -> CommitBody {
  sync_utils::commit_body::commit_body(message_body(&params.message))
}
