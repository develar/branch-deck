//! The dependency order of the files of a review (spec/branch/review-plan.spec.md, "Dependency order"): a file that
//! declares what another file of the review uses comes before it, a test right after its subject.
//!
//! - **The files**: every path the reviewed commits change, once, at its last path: a renamed file at its new path, a
//!   deleted one at its path (the review still shows it), none that the commits add and delete again.
//! - **The graph**, from the syntax summaries the API already read (`SymbolInput`), no git: each kept file as the last
//!   reviewed commit that changes it leaves it, its declarations against the uses and the imports of the others, with
//!   the evidence of the symbol dependencies (`trial_check::symbol_dependencies::file_links`), so a name declared in two
//!   modules links only where an import, a qualified name, a package or a module tells which. A deleted file, a file
//!   of a language the summaries don't read and a degraded one have no edge.
//! - **The order**: the strongly connected components (Tarjan), each placed when every component it uses is, the one
//!   that comes first in the tree first; the files of a cycle in tree order. With no edge at all it is the tree order.
//! - **Tests** (`risk::test_subject`) are left out of the graph when the review changes their subject (`subject_stem`)
//!   and placed right after it, several in tree order; of two subjects of that stem the one the test uses, else the one
//!   nearest in the tree, else the first. A test without its subject in the review is a file like any other.
//! - **Tree order**, as the file tree of the review sorts (`file_diffs::service::compare_tree_paths`): at each level
//!   the directories before the files, then the names without case, a number by its value (`file2` before `file10`).

use crate::risk::{subject_stem, test_subject};
use file_diffs::service::compare_tree_paths;
use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet, BinaryHeap, HashMap};
use trial_check::diff_scan::CommitDiff;
use trial_check::symbol_dependencies::file_links;
use trial_check::symbol_scan::SymbolInput;

/// A changed file after the commits
struct FileState<'a> {
  /// Its blob after the last commit that changes it; `None` when deleted
  blob: Option<&'a str>,
  /// It was there before the first commit that changes it
  existed: bool,
}

/// The paths `diffs` (oldest first) change, each once at its last path, in dependency order
pub fn review_order(input: &SymbolInput, diffs: &[CommitDiff]) -> Vec<String> {
  let mut files: BTreeMap<&str, FileState> = BTreeMap::new();
  for change in diffs.iter().flat_map(|diff| &diff.files) {
    let renamed_from = change.old_path.as_deref().filter(|old_path| *old_path != change.path);
    let earlier = match renamed_from {
      Some(old_path) => files.remove(old_path),
      None => files.remove(change.path.as_str()),
    };
    let state = FileState {
      blob: if change.status == 'D' { None } else { change.new_blob.as_deref() },
      existed: earlier.map_or(change.status != 'A', |earlier| earlier.existed),
    };
    files.insert(&change.path, state);
  }
  files.retain(|_, state| state.blob.is_some() || state.existed);

  let mut paths: Vec<&str> = files.keys().copied().collect();
  paths.sort_by(|a, b| compare_tree_paths(a, b));
  let blobs: Vec<(&str, &str)> = paths.iter().filter_map(|path| Some((*path, files[path].blob?))).collect();
  let rank: HashMap<&str, usize> = paths.iter().enumerate().map(|(rank, path)| (*path, rank)).collect();
  // declaring file -> the files that use it, by rank
  let mut uses: BTreeSet<(usize, usize)> = BTreeSet::new();
  for link in file_links(input, &blobs) {
    if let (Some(&declared), Some(&used)) = (rank.get(link.declared_in.as_str()), rank.get(link.used_in.as_str())) {
      uses.insert((declared, used));
    }
  }
  order_files(&paths, &uses).into_iter().map(str::to_string).collect()
}

/// `paths` (in tree order) in dependency order by `uses` (the rank of a declaring file, of a file that uses it)
pub(crate) fn order_files<'a>(paths: &[&'a str], uses: &BTreeSet<(usize, usize)>) -> Vec<&'a str> {
  let subject_of = subjects(paths, uses);
  let in_graph: Vec<usize> = (0..paths.len()).filter(|file| !subject_of.contains_key(file)).collect();
  let mut successors: Vec<Vec<usize>> = vec![Vec::new(); paths.len()];
  for &(declared, used) in uses {
    if declared != used && !subject_of.contains_key(&declared) && !subject_of.contains_key(&used) {
      successors[declared].push(used);
    }
  }
  let mut tests_of: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
  for (&test, &subject) in &subject_of {
    tests_of.entry(subject).or_default().push(test);
  }

  let mut ordered = Vec::with_capacity(paths.len());
  for component in topological_components(&in_graph, &successors) {
    for file in component {
      ordered.push(paths[file]);
      for &test in tests_of.get(&file).into_iter().flatten() {
        ordered.push(paths[test]);
      }
    }
  }
  ordered
}

/// Test -> its subject, by rank: of the files of `paths` whose stem the test names, the one it uses, else the one
/// nearest in the tree, else the first
fn subjects(paths: &[&str], uses: &BTreeSet<(usize, usize)>) -> BTreeMap<usize, usize> {
  let mut by_stem: HashMap<String, Vec<usize>> = HashMap::new();
  for (file, path) in paths.iter().enumerate() {
    if test_subject(path).is_none() {
      by_stem.entry(subject_stem(path)).or_default().push(file);
    }
  }
  paths
    .iter()
    .enumerate()
    .filter_map(|(test, path)| {
      let candidates = by_stem.get(&test_subject(path)?)?;
      let subject = candidates
        .iter()
        .copied()
        .min_by_key(|&candidate| (!uses.contains(&(candidate, test)), Reverse(shared_directories(path, paths[candidate])), candidate))?;
      Some((test, subject))
    })
    .collect()
}

/// The number of directories two paths share from the root
fn shared_directories(a: &str, b: &str) -> usize {
  fn directories(path: &str) -> impl Iterator<Item = &str> {
    path.rsplit_once('/').map(|(directory, _)| directory.split('/')).into_iter().flatten()
  }
  directories(a).zip(directories(b)).take_while(|(a, b)| a == b).count()
}

/// The strongly connected components of `nodes` (ascending) by `successors`, each in ascending order, a component after
/// every one that has an edge into it; of those ready, the one of the least node first
fn topological_components(nodes: &[usize], successors: &[Vec<usize>]) -> Vec<Vec<usize>> {
  let components = strongly_connected(nodes, successors);
  let mut component_of: HashMap<usize, usize> = HashMap::new();
  for (index, component) in components.iter().enumerate() {
    for &node in component {
      component_of.insert(node, index);
    }
  }
  let mut next: Vec<BTreeSet<usize>> = vec![BTreeSet::new(); components.len()];
  let mut incoming = vec![0_usize; components.len()];
  for &node in nodes {
    for successor in &successors[node] {
      let (from, to) = (component_of[&node], component_of[successor]);
      if from != to && next[from].insert(to) {
        incoming[to] += 1;
      }
    }
  }
  // the least node of a component: its place in the tree
  let first = |component: usize| components[component][0];
  let mut ready: BinaryHeap<Reverse<(usize, usize)>> = (0..components.len())
    .filter(|&component| incoming[component] == 0)
    .map(|component| Reverse((first(component), component)))
    .collect();
  let mut ordered = Vec::with_capacity(components.len());
  while let Some(Reverse((_, component))) = ready.pop() {
    for &successor in &next[component] {
      incoming[successor] -= 1;
      if incoming[successor] == 0 {
        ready.push(Reverse((first(successor), successor)));
      }
    }
    ordered.push(component);
  }
  let mut components = components;
  ordered.into_iter().map(|component| std::mem::take(&mut components[component])).collect()
}

/// Tarjan's strongly connected components of `nodes` by `successors`, without recursion; each sorted ascending
fn strongly_connected(nodes: &[usize], successors: &[Vec<usize>]) -> Vec<Vec<usize>> {
  const UNVISITED: usize = usize::MAX;
  let size = successors.len();
  let mut index = vec![UNVISITED; size];
  let mut low = vec![0; size];
  let mut on_stack = vec![false; size];
  let mut stack: Vec<usize> = Vec::new();
  let mut components: Vec<Vec<usize>> = Vec::new();
  let mut counter = 0;
  for &root in nodes {
    if index[root] != UNVISITED {
      continue;
    }
    // (node, the position of the next successor to visit)
    let mut work: Vec<(usize, usize)> = vec![(root, 0)];
    while let Some(&mut (node, ref mut position)) = work.last_mut() {
      if *position == 0 && index[node] == UNVISITED {
        index[node] = counter;
        low[node] = counter;
        counter += 1;
        stack.push(node);
        on_stack[node] = true;
      }
      if let Some(&successor) = successors[node].get(*position) {
        *position += 1;
        if index[successor] == UNVISITED {
          work.push((successor, 0));
        } else if on_stack[successor] {
          low[node] = low[node].min(index[successor]);
        }
        continue;
      }
      work.pop();
      if let Some(&(parent, _)) = work.last() {
        low[parent] = low[parent].min(low[node]);
      }
      if low[node] == index[node] {
        let mut component = Vec::new();
        while let Some(member) = stack.pop() {
          on_stack[member] = false;
          component.push(member);
          if member == node {
            break;
          }
        }
        component.sort_unstable();
        components.push(component);
      }
    }
  }
  components
}
