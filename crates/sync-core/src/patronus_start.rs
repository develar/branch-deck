//! The words of a mode and of what a robot lands, for `bd` and the guards of the app. Whether a robot still runs and
//! pushes is `domain_policy::holds`, which robot belongs to a branch `domain_policy::robot_match`, what they land and
//! landed the repository view (`repository_model::patronus_view`), what a start replaces and the modes a profile
//! allows `domain_policy::land`.

use domain_types::patronus::PatronusPushMode;
use domain_types::repository_view::Carried;

/// "auth", "auth and login", "auth, login and oauth"
pub fn join_names(names: &[String]) -> String {
  match names {
    [] => String::new(),
    [only] => only.clone(),
    [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
  }
}

/// How many commit ids a refusal lists
const LISTED_COMMITS: usize = 3;

/// What a robot lands of `subject`, as a refusal says it: "login" when only the stack tells, else "2 commits of login"
pub fn carried_words(subject: &str, carried: Option<&Carried>) -> String {
  match carried {
    None => subject.to_string(),
    Some(carried) if carried.commits.len() == 1 => format!("1 commit of {subject}"),
    Some(carried) => format!("{} commits of {subject}", carried.commits.len()),
  }
}

/// The ids of the carried commits for `bd`, after the sentence: " (1a2b3c4d, 5e6f7a8b amended since)", the first three;
/// empty when only the stack tells
pub fn carried_ids(carried: Option<&Carried>) -> String {
  let Some(carried) = carried else {
    return String::new();
  };
  let mut listed: Vec<String> = carried
    .commits
    .iter()
    .take(LISTED_COMMITS)
    .map(|id| {
      let short = &id[..id.len().min(8)];
      if carried.amended.contains(id) {
        format!("{short} amended since")
      } else {
        short.to_string()
      }
    })
    .collect();
  if carried.commits.len() > LISTED_COMMITS {
    listed.push("…".to_string());
  }
  format!(" ({})", listed.join(", "))
}

/// "Safe Push", "Dry Run", as the app names a mode
pub const fn push_mode_name(mode: PatronusPushMode) -> &'static str {
  match mode {
    PatronusPushMode::DryRun => "Dry Run",
    PatronusPushMode::Rebase => "Safe Push (rebase)",
    PatronusPushMode::RebaseAutosquash => "Safe Push",
    PatronusPushMode::SquashAll => "Safe Push (squash)",
    PatronusPushMode::Merge => "Safe Merge",
    PatronusPushMode::Unknown => "Robot",
  }
}
