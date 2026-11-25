use crate::session::{ArchivedChanges, FactsChanges, PatronusChange, PullRequestsChange, RobotsChange, TableChanges, Version, ViewPatch};
use pretty_assertions::assert_eq;
use std::collections::BTreeMap;

/// A patch reads back from the wire as the session sent it, with every part it changed whose collections are empty (a
/// listing of the pull requests that changed only its head, which every answer of a repository that is not on GitHub
/// sends): a part that leaves out an empty collection takes it as empty, else the mirror of the IDE (`ide-host`) drops
/// the patch, reads the view anew and loses the news of the patches that came meanwhile
#[test]
fn a_patch_of_parts_with_empty_collections_reads_back_from_the_wire() {
  let patch = ViewPatch {
    base: Version { epoch: 1, seq: 2 },
    next: Version { epoch: 1, seq: 3 },
    changes: TableChanges {
      archived: Some(ArchivedChanges::default()),
      patronus: Some(PatronusChange::default()),
      facts: Some(FactsChanges {
        robots: Some(RobotsChange::default()),
        pull_requests: Some(PullRequestsChange::default()),
        rules: Some(BTreeMap::new()),
        ..FactsChanges::default()
      }),
      ..TableChanges::default()
    },
  };
  let wire = serde_json::to_string(&patch).unwrap();
  let read: ViewPatch = serde_json::from_str(&wire).unwrap_or_else(|error| panic!("{error}: {wire}"));
  assert_eq!(read, patch);
}
