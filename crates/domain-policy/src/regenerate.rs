//! The generated files the words merged that the copy of a branch or of a whole stack holds
//! (openspec/specs/branch/branch-row/spec.md, "Status cell: A copy with a generated file merged by words says
//! Regenerate"; openspec/specs/repository/outgoing/spec.md, "Push: A push of a copy with a generated file merged by words
//! asks first"; docs/design/git-operations.md, "Resolve by words"). A sync resolves a conflict in a generated file by
//! words (`sync_core::regenerate`): the copy is clean as text, but its file is no output of its generator, which only
//! the copy has (the user's tree has the commit the copy lacks). What clears it gives the copy that commit: a stack of
//! the root of what lands onto its branch, or the move of the commit, unless it lands first; a later commit of the branch
//! that writes the file also does.
//!
//! - `row_fact`: what the Status cell says of it, none for a branch in master or parked, which lands nothing
//! - `fix_of`: the fix of the first file whose copy lacks a commit (`LandingFix::for_need`)
//! - `confirmation`: what a push or a start of the copy asks first, every time until the copy no longer holds it

use domain_types::conflict::RegenerateFile;
use domain_types::landing::{Confirmation, LandingFix, Subject};
use domain_types::next_step::RowFact;

/// What the Status cell of a branch whose copy holds `files` says, with `fix`; none without files, and none for a branch
/// in master (`landed`) or parked, which lands nothing
#[must_use]
pub fn row_fact(files: &[RegenerateFile], fix: Option<&LandingFix>, landed: bool, parked: bool) -> Option<RowFact> {
  (!files.is_empty() && !landed && !parked).then(|| RowFact::Regenerate {
    files: files.to_vec(),
    fix: fix.cloned(),
  })
}

/// How what lands from `root` (`lands`: the branches it lands, the root first) gives the copy the commit it lacks, of the
/// first of `files` whose copy lacks one: a restack of the root onto the branch of that commit, unless that branch is
/// built on what lands (`ancestors_of`: the branches a branch is built on), then the move of the commit into the branch
/// whose copy merged the file. None when no copy lacks a known commit
#[must_use]
pub fn fix_of<'a>(files: &[RegenerateFile], root: &str, lands: &[&str], ancestors_of: impl Fn(&str) -> Vec<&'a str>) -> Option<LandingFix> {
  files.iter().find_map(|file| {
    let lacks = file.lacks.as_ref()?;
    let onto = lacks.branch.as_str();
    let built_on_lands = !onto.is_empty() && ancestors_of(onto).iter().any(|ancestor| lands.contains(ancestor));
    Some(LandingFix::for_need(lacks, &file.commit.branch, root, built_on_lands))
  })
}

/// What a push or a start of the copy of `subject` that holds `files` asks first; none without files
#[must_use]
pub fn confirmation(subject: &Subject, files: &[RegenerateFile]) -> Option<Confirmation> {
  (!files.is_empty()).then(|| Confirmation::Regenerate {
    subject: subject.clone(),
    files: files.to_vec(),
  })
}
