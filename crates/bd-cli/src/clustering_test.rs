use crate::clustering::{
  ANONYMOUS_MARKER, Cluster, ClusterChange, ClusterInput, ClusterOptions, Clustering, CommitFeatures, Conflict, FileLines, Link, MAX_REPAIR_ROUNDS, Repaired, Warning, anonymize,
  cluster, cluster_change, cluster_with_repair, is_issue_only, name_clusters, subject_words, why,
};
use pretty_assertions::assert_eq;
use serde_json::Value;
use std::collections::{BTreeSet, HashMap, HashSet};
use std::convert::Infallible;
use std::hash::{BuildHasherDefault, DefaultHasher, RandomState};

fn files(files: &[(&str, u32)]) -> Vec<FileLines> {
  files.iter().map(|&(path, lines)| FileLines { path: path.to_string(), lines }).collect()
}

/// A commit that changes one file of 10 lines
fn touching(path: &str) -> CommitFeatures {
  CommitFeatures {
    files: files(&[(path, 10)]),
    ..CommitFeatures::default()
  }
}

fn keyed(key: &str, path: &str) -> CommitFeatures {
  CommitFeatures {
    key: Some(key.to_string()),
    group: Some(key.to_string()),
    ..touching(path)
  }
}

fn prefixed(prefix: &str, path: &str) -> CommitFeatures {
  CommitFeatures {
    prefix: Some(prefix.to_string()),
    group: Some(prefix.to_string()),
    ..touching(path)
  }
}

fn scoped(scope: &str, path: &str) -> CommitFeatures {
  CommitFeatures {
    scope: Some(scope.to_string()),
    ..touching(path)
  }
}

fn mentioning(mut commit: CommitFeatures, keys: &[&str]) -> CommitFeatures {
  commit.mentions = keys.iter().map(|key| (*key).to_string()).collect();
  commit
}

fn input(commits: Vec<CommitFeatures>) -> ClusterInput {
  ClusterInput {
    commits,
    dependencies: Vec::new(),
  }
}

fn groups(clustering: &Clustering) -> Vec<Vec<usize>> {
  clustering.clusters.iter().map(|cluster| cluster.commits.clone()).collect()
}

fn fresh() -> ClusterOptions {
  ClusterOptions {
    fresh: true,
    ..ClusterOptions::default()
  }
}

#[test]
fn hard_units_join_dependencies_fixups_prefixes_and_keys() {
  let mut input = input(vec![
    prefixed("a", "one/x.txt"),
    prefixed("a", "two/y.txt"),
    keyed("ABC-1", "three/z.txt"),
    keyed("ABC-1", "four/w.txt"),
    // a prefixed commit with the key stays in its anchor
    CommitFeatures {
      key: Some("ABC-1".to_string()),
      ..prefixed("b", "five/v.txt")
    },
    CommitFeatures {
      fixup_of: Some(2),
      ..touching("six/u.txt")
    },
    touching("seven/t.txt"),
  ]);
  input.dependencies = vec![[6, 0]];
  let clustering = cluster(&input, &ClusterOptions::default());
  assert_eq!(groups(&clustering), [vec![0, 1, 6], vec![2, 3, 5], vec![4]]);
  assert_eq!(clustering.clusters[0].anchor.as_deref(), Some("a"));
  assert_eq!((clustering.clusters[1].units, clustering.clusters[1].issue_units), (1, 0), "a fixup without the key");
  assert_eq!(clustering.warnings, []);

  // without anchors the prefix joins nothing and the key joins every commit that has it
  let clustering = cluster(&input, &fresh());
  assert_eq!(groups(&clustering), [vec![0, 6], vec![1], vec![2, 3, 4, 5]]);
  assert!(clustering.clusters.iter().all(|cluster| cluster.anchor.is_none()));
}

#[test]
fn an_edge_between_two_anchors_is_a_warning_and_anchors_never_merge() {
  let mut input = input(vec![
    CommitFeatures {
      scope: Some("vm".to_string()),
      ..prefixed("a", "tools/vm/src/lease.rs")
    },
    CommitFeatures {
      scope: Some("vm".to_string()),
      ..prefixed("b", "tools/vm/src/lease.rs")
    },
  ]);
  input.dependencies = vec![[1, 0]];
  let clustering = cluster(&input, &ClusterOptions::default());
  assert_eq!(groups(&clustering), [vec![0], vec![1]]);
  assert_eq!(
    clustering.warnings,
    [Warning::AnchorDependency {
      anchors: ["b".to_string(), "a".to_string()],
      fixup: false
    }]
  );
  assert_eq!(groups(&cluster(&input, &fresh())), [vec![0, 1]]);
}

#[test]
fn the_same_scope_merges_commits_without_keys_and_another_scope_halves_paths() {
  let input = input(vec![
    scoped("vm", "tools/vm/src/lease.rs"),
    scoped("vm", "tools/vm/src/pool.rs"),
    scoped("build", "tools/vm/src/wrapper.rs"),
    scoped("vm", "tools/vm/src/marker.rs"),
  ]);
  let clustering = cluster(&input, &ClusterOptions::default());
  assert_eq!(groups(&clustering), [vec![0, 1, 3], vec![2]]);
  assert_eq!(clustering.clusters[0].links, [Link::Scope("vm".to_string()), Link::Scope("vm".to_string())]);
  assert_eq!(why(&input, &clustering.clusters[0]), "3 units; scope vm in 3 subjects; area tools/vm/src");
}

#[test]
fn a_mention_links_keys_of_one_project_but_hardly_across_projects() {
  let input = input(vec![
    keyed("ABC-1", "one/a.txt"),
    mentioning(keyed("ABC-2", "two/b.txt"), &["ABC-1"]),
    mentioning(keyed("XYZ-9", "three/c.txt"), &["ABC-1"]),
    mentioning(keyed("XYZ-10", "four/d.txt"), &["QQ-5"]),
    mentioning(keyed("XYZ-11", "five/e.txt"), &["QQ-5"]),
  ]);
  let clustering = cluster(&input, &ClusterOptions::default());
  assert_eq!(groups(&clustering), [vec![0, 1], vec![2], vec![3, 4]]);
  assert_eq!(clustering.clusters[0].links, [Link::Mention("ABC-1".to_string())]);
  assert_eq!(clustering.clusters[2].links, [Link::CoMention("QQ-5".to_string())]);
  assert_eq!(why(&input, &clustering.clusters[0]), "2 issue units; ABC-1 named in 1 body; area one");
}

#[test]
fn shared_paths_merge_but_a_feature_of_many_units_or_a_peripheral_file_weighs_little() {
  // the same files: a strong merge
  let same = input(vec![touching("app/core/src/a/b.kt"), touching("app/core/src/a/b.kt")]);
  let clustering = cluster(&same, &ClusterOptions::default());
  assert_eq!(groups(&clustering), [vec![0, 1]]);
  assert_eq!(clustering.clusters[0].links, [Link::Paths("app/core/src".to_string())]);
  assert!(!clustering.clusters[0].weak);

  // a file that every unit changes tells nothing
  let commits = (0..10)
    .map(|index| CommitFeatures {
      files: files(&[("common/list.txt", 10), (&format!("own{index}/file.txt"), 10)]),
      ..CommitFeatures::default()
    })
    .collect();
  let clustering = cluster(&input(commits), &ClusterOptions::default());
  assert_eq!(clustering.clusters.len(), 10);

  // a shared file of 1 line in 101 is peripheral; a shared file of most lines is not
  let with_files = |changes: &[(&str, u32)]| CommitFeatures {
    files: files(changes),
    ..CommitFeatures::default()
  };
  let peripheral = input(vec![
    with_files(&[("x/src/a.kt", 100), ("y/src/s.kt", 1)]),
    with_files(&[("y/src/s.kt", 1), ("z/src/b.kt", 100)]),
    with_files(&[("y/src/s.kt", 100), ("x/src/c.kt", 1)]),
    with_files(&[("y/src/s.kt", 100), ("z/src/d.kt", 1)]),
  ]);
  assert_eq!(groups(&cluster(&peripheral, &ClusterOptions::default())), [vec![0], vec![1], vec![2, 3]]);
}

#[test]
fn the_cap_stops_soft_merges_and_a_tie_goes_to_the_oldest_pair() {
  let input = input(vec![scoped("vm", "a/x.txt"), scoped("vm", "b/y.txt"), scoped("vm", "c/z.txt")]);
  let options = ClusterOptions {
    max_commits: 2,
    ..ClusterOptions::default()
  };
  let first = cluster(&input, &options);
  assert_eq!(groups(&first), [vec![0, 1], vec![2]]);
  assert_eq!(cluster(&input, &options), first, "deterministic");
  assert_eq!(groups(&cluster(&input, &ClusterOptions::default())), [vec![0, 1, 2]]);
}

/// Repair with a check that finds commit 1 conflicting until it shares a hard unit with commit 0
fn repair_until_joined(input: &ClusterInput) -> Repaired {
  cluster_with_repair(input, &ClusterOptions::default(), |clustering| {
    let apart = clustering.unit_of[0] != clustering.unit_of[1];
    Ok::<_, Infallible>(if apart { vec![Conflict { commit: 1, needs: vec![0] }] } else { Vec::new() })
  })
  .unwrap()
}

#[test]
fn repair_joins_a_conflicting_commit_to_what_it_needs() {
  let pair = input(vec![keyed("ABC-1", "one/a.txt"), keyed("XYZ-1", "two/b.txt")]);
  let repaired = repair_until_joined(&pair);
  assert_eq!(
    (groups(&repaired.clustering), repaired.rounds, repaired.conflicts.clone()),
    (vec![vec![0, 1]], 1, Vec::new())
  );
  assert_eq!(repaired.clustering.clusters[0].repair_rounds, [1]);
  assert_eq!(why(&pair, &repaired.clustering.clusters[0]), "1 unit; joined by repair round 1 after a conflict; area one");

  // a conflict inside one cluster of two issue units that build apart (an issue group) joins them too
  let group = input(vec![keyed("ABC-1", "one/a.txt"), mentioning(keyed("ABC-2", "two/b.txt"), &["ABC-1"])]);
  let first = cluster(&group, &ClusterOptions::default());
  assert_eq!((groups(&first), is_issue_only(&group, &first.clusters[0])), (vec![vec![0, 1]], true));
  let repaired = repair_until_joined(&group);
  assert_eq!((repaired.rounds, repaired.clustering.clusters[0].units), (1, 1));
  assert!(!is_issue_only(&group, &repaired.clustering.clusters[0]), "the plan must build them together");

  // two anchors are never joined: the conflict stays
  let anchors = input(vec![prefixed("a", "one/a.txt"), prefixed("b", "two/b.txt")]);
  let conflict = Conflict { commit: 1, needs: vec![0] };
  let repaired = cluster_with_repair(&anchors, &ClusterOptions::default(), |_| Ok::<_, Infallible>(vec![conflict.clone()])).unwrap();
  assert_eq!(
    (groups(&repaired.clustering), repaired.rounds, repaired.conflicts),
    (vec![vec![0], vec![1]], 0, vec![conflict])
  );

  // at most MAX_REPAIR_ROUNDS rounds: each check finds the next commit apart
  let chain = input((0..9).map(|index| keyed(&format!("P{index}-1"), &format!("dir{index}/f.txt"))).collect());
  let mut checks = 0;
  let repaired = cluster_with_repair(&chain, &ClusterOptions::default(), |clustering| {
    checks += 1;
    let together = clustering.clusters[0].commits.len();
    Ok::<_, Infallible>(vec![Conflict {
      commit: together,
      needs: vec![together - 1],
    }])
  })
  .unwrap();
  assert_eq!(
    (repaired.rounds, checks, repaired.clustering.clusters[0].commits.len()),
    (MAX_REPAIR_ROUNDS, MAX_REPAIR_ROUNDS + 1, MAX_REPAIR_ROUNDS + 1)
  );
}

/// A clustering made by hand, without its units
fn manual(clusters: Vec<Cluster>) -> Clustering {
  Clustering {
    clusters,
    warnings: Vec::new(),
    unit_of: Vec::new(),
    unit_anchors: Vec::new(),
  }
}

fn single(commits: Vec<usize>) -> Cluster {
  Cluster {
    commits,
    anchor: None,
    keys: Vec::new(),
    units: 1,
    issue_units: 0,
    links: Vec::new(),
    weak: false,
    repair_rounds: Vec::new(),
  }
}

#[test]
fn names_in_order_of_preference_with_collisions() {
  let worded = |words: &[&str], path: &str| CommitFeatures {
    words: words.iter().map(|word| (*word).to_string()).collect(),
    ..touching(path)
  };
  let input = input(vec![
    prefixed("feat", "a/b.txt"),
    keyed("ABC-1", "a/c.txt"),
    scoped("vm", "tools/vm/a.rs"),
    scoped("vm", "tools/vm/b.rs"),
    scoped("build", "tools/vm/c.rs"),
    worded(&["unload", "tab"], "plugins/air/shared/x/y.kt"),
    worded(&["unload", "cache"], "plugins/air/shared/x/z.kt"),
    keyed("ABC-2", "q/r.txt"),
    keyed("ABC-3", "q/s.txt"),
    keyed("ABC-3", "q/t.txt"),
    touching("lonely.txt"),
    // a case twin of the group `feat`, which is another cluster's
    prefixed("FEAT", "w/v.txt"),
    worded(&["tidy"], "docs/guide.md"),
  ]);
  let clusters = vec![
    Cluster {
      anchor: Some("feat".to_string()),
      ..single(vec![0])
    },
    Cluster {
      keys: vec!["ABC-1".to_string()],
      ..single(vec![1])
    },
    single(vec![2, 3, 4]),
    single(vec![5, 6]),
    Cluster {
      keys: vec!["ABC-2".to_string(), "ABC-3".to_string()],
      ..single(vec![7, 8, 9])
    },
    single(vec![10]),
    Cluster {
      anchor: Some("FEAT".to_string()),
      ..single(vec![11])
    },
    single(vec![12]),
  ];
  let clustering = manual(clusters);
  // an existing virtual branch that none of the commits comes from
  let taken = HashSet::from(["docs-tidy".to_string()]);
  assert_eq!(
    name_clusters(&input, &clustering, &taken),
    ["feat", "ABC-1", "vm", "shared-unload", "ABC-3", "cluster-6", "FEAT-2", "docs-tidy-2"]
  );
}

#[test]
fn a_cluster_takes_the_name_of_the_branch_it_holds_most_of() {
  // with --fresh the prefixes join nothing, but a cluster still takes over the branch its commits come from
  let input = input(vec![
    prefixed("feat", "a/x.txt"),
    prefixed("feat", "a/y.txt"),
    prefixed("feat", "b/z.txt"),
    // a stacked branch: the group is the leaf of the prefix
    CommitFeatures {
      key: Some("ABC-1".to_string()),
      group: Some("c".to_string()),
      ..prefixed("p/c", "c/w.txt")
    },
  ]);
  let clustering = manual(vec![single(vec![0, 1]), single(vec![2]), single(vec![3])]);
  let names = name_clusters(&input, &clustering, &HashSet::new());
  assert_eq!(names, ["feat", "cluster-2", "p/c"]);
  let changes: Vec<ClusterChange> = clustering
    .clusters
    .iter()
    .zip(&names)
    .map(|(cluster, name)| cluster_change(&input, cluster, name).0)
    .collect();
  assert_eq!(
    changes,
    [ClusterChange::Split, ClusterChange::Split, ClusterChange::Kept],
    "the group of a stack path is its leaf"
  );
}

#[test]
fn changes_against_the_groups_now() {
  let input = input(vec![prefixed("a", "a/x.txt"), prefixed("a", "a/y.txt"), prefixed("b", "b/z.txt"), touching("c/v.txt")]);
  let change = |commits: Vec<usize>, name: &str| cluster_change(&input, &single(commits), name).0;
  assert_eq!(change(vec![0, 1], "a"), ClusterChange::Kept);
  assert_eq!(change(vec![0, 1], "renamed"), ClusterChange::Renamed);
  assert_eq!(change(vec![0], "a"), ClusterChange::Split);
  assert_eq!(change(vec![0, 1, 3], "a"), ClusterChange::Merged, "a whole branch with an unassigned commit");
  assert_eq!(change(vec![0, 1, 2], "a"), ClusterChange::Merged);
  assert_eq!(change(vec![0, 2], "a"), ClusterChange::New);
  assert_eq!(change(vec![3], "c"), ClusterChange::New);
}

#[test]
fn a_merge_that_only_adjacency_lifts_over_the_threshold_is_weak() {
  // P is ln 2 / (ln 2 + ln 3) = 0.387: under the threshold alone, over it with the 0.05 of neighbours
  let shared = CommitFeatures {
    files: files(&[("m/src/p/s.kt", 10)]),
    ..CommitFeatures::default()
  };
  let wider = CommitFeatures {
    files: files(&[("m/src/p/s.kt", 10), ("n/src/q/o.kt", 10)]),
    ..CommitFeatures::default()
  };
  let clustering = cluster(&input(vec![shared.clone(), wider.clone()]), &ClusterOptions::default());
  assert_eq!(groups(&clustering), [vec![0, 1]]);
  assert!(clustering.clusters[0].weak);
  assert_eq!(clustering.clusters[0].links, [Link::Paths("m/src/p".to_string())]);
  // not neighbours: no merge
  let apart = cluster(&input(vec![shared, touching("z/far.txt"), wider]), &ClusterOptions::default());
  assert_eq!(groups(&apart), [vec![0], vec![1], vec![2]]);
}

#[test]
fn subject_words_drop_stopwords_and_endings() {
  assert_eq!(
    subject_words("the editor tabs of a file stay in sync after an editor is added"),
    ["editor", "tab", "file", "stay", "sync"]
  );
  assert_eq!(subject_words("A plugin unload removes the closed tabs"), ["plugin", "unload", "clos", "tab"]);
  assert_eq!(subject_words("x y z"), Vec::<String>::new());
}

#[test]
fn anonymize_keeps_the_clustering_and_no_name() {
  let mut input = input(vec![
    CommitFeatures {
      words: vec!["secret".to_string()],
      ..mentioning(keyed("ABC-1", "plugins/secret/src/com/acme/A.kt"), &["ABC-2"])
    },
    keyed("ABC-2", "plugins/secret/test/com/acme/ATest.kt"),
    CommitFeatures {
      fixup_of: Some(0),
      ..prefixed("team/secret", "plugins/other/resources/x.xml")
    },
    scoped("vm", "tools/vm/src/lease.rs"),
    scoped("vm", "tools/vm/src/pool.rs"),
  ]);
  input.dependencies = vec![[4, 3]];
  let fixed = BuildHasherDefault::<DefaultHasher>::default();
  let anonymous = anonymize(&input, &fixed);
  assert_eq!(
    groups(&cluster(&anonymous, &ClusterOptions::default())),
    groups(&cluster(&input, &ClusterOptions::default()))
  );
  assert_eq!(groups(&cluster(&anonymous, &fresh())), groups(&cluster(&input, &fresh())));
  assert_eq!(anonymous.dependencies, input.dependencies);
  assert_eq!(anonymous.commits[2].fixup_of, Some(0));

  let text = serde_json::to_string(&anonymous).unwrap();
  for name in ["plugins", "secret", "acme", "src", "test", "team", "vm", "ABC", "A.kt", "lease"] {
    assert!(!text.contains(name), "{name} in {text}");
  }
  let path = &anonymous.commits[0].files[0].path;
  assert_eq!(path.split('/').filter(|segment| *segment == ANONYMOUS_MARKER).count(), 1, "{path}");
  // the keys of one project share their project
  let project = |key: Option<&String>| key.unwrap().split_once('-').unwrap().0.to_string();
  assert_eq!(project(anonymous.commits[0].key.as_ref()), project(anonymous.commits[1].key.as_ref()));
  assert_ne!(anonymous.commits[0].key, anonymous.commits[1].key);
  assert_eq!(anonymous.commits[0].mentions, [anonymous.commits[1].key.clone().unwrap()]);
  assert_eq!(anonymous.commits[2].prefix.as_deref().unwrap().split('/').count(), 2);
  assert_eq!(anonymous.commits[0].words, Vec::<String>::new());
  assert_ne!(anonymize(&input, &RandomState::new()), anonymous, "a random key changes every name");

  // only the directories of file paths are module markers: the scopes `test` and `gen` and the prefix `src` stay names
  let markers = super::clustering_test::input(vec![scoped("test", "a/x.txt"), scoped("gen", "b/y.txt"), prefixed("src", "c/z.txt")]);
  let anonymous = anonymize(&markers, &fixed);
  assert_ne!(anonymous.commits[0].scope, anonymous.commits[1].scope);
  assert_ne!(anonymous.commits[2].prefix.as_deref(), Some(ANONYMOUS_MARKER));
  assert_eq!(anonymous.commits[0].scope.as_ref().map(String::len), Some(16), "64 bits");
}

/// The anonymized input of a large monorepo branch (`bd suggest --fresh --dump-input`) and the grouping a human made of
/// it (`expected`, by commit position); `apart` pairs expected groups that must stay apart. With --fresh the clustering
/// must find the human grouping: the pairs of commits it puts together must be together there (precision), and it
/// must find most of the pairs there (recall)
#[test]
fn golden_monorepo_branch() {
  let path = concat!(env!("CARGO_MANIFEST_DIR"), "/testdata/suggest/monorepo.json");
  let golden: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{path}: {e}"))).unwrap();
  let input: ClusterInput = serde_json::from_value(golden["input"].clone()).unwrap();
  let expected: Vec<Vec<usize>> = serde_json::from_value(golden["expected"].clone()).unwrap();
  let apart: Vec<[usize; 2]> = serde_json::from_value(golden["apart"].clone()).unwrap();

  let clustering = cluster(&input, &fresh());
  let (precision, recall) = pairwise(&expected, &groups(&clustering));
  eprintln!("golden: {} clusters, precision {precision:.3}, recall {recall:.3}", clustering.clusters.len());
  assert!(precision >= 0.95, "precision {precision:.3}");
  assert!(recall >= 0.85, "recall {recall:.3}");

  let expected_of: HashMap<usize, usize> = expected
    .iter()
    .enumerate()
    .flat_map(|(group, commits)| commits.iter().map(move |&commit| (commit, group)))
    .collect();
  for cluster in &clustering.clusters {
    let found: BTreeSet<usize> = cluster.commits.iter().map(|commit| expected_of[commit]).collect();
    for [a, b] in &apart {
      assert!(!(found.contains(a) && found.contains(b)), "groups {a} and {b} share a cluster: {found:?}");
    }
    let projects: BTreeSet<&str> = cluster.keys.iter().map(|key| key.split_once('-').unwrap().0).collect();
    assert!(projects.len() <= 1, "issue keys of several projects in one cluster: {projects:?}");
  }
}

/// Pairwise precision and recall of `found` against `expected`
fn pairwise(expected: &[Vec<usize>], found: &[Vec<usize>]) -> (f64, f64) {
  let pairs = |groups: &[Vec<usize>]| -> HashSet<(usize, usize)> {
    groups
      .iter()
      .flat_map(|group| group.iter().flat_map(move |&a| group.iter().filter(move |&&b| a < b).map(move |&b| (a, b))))
      .collect()
  };
  let (expected, found) = (pairs(expected), pairs(found));
  let common = f64::from(u32::try_from(expected.intersection(&found).count()).unwrap());
  let ratio = |count: usize| if count == 0 { 1.0 } else { common / f64::from(u32::try_from(count).unwrap()) };
  (ratio(found.len()), ratio(expected.len()))
}
