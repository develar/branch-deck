//! The generated files of `brd review-plan`, folded (openspec/specs/branch/review-plan/spec.md, "The plan of an agent
//! folds the generated files"): a generator's run over a monorepo writes thousands of files, each once in `files`,
//! `order`, the slices of the unit of its kind and the stops and paths of a step, which made the plan of one branch
//! 11 MB that no agent reads. Each of those lists keeps its generated files up to `GENERATED_SHOWN`, by the rule of a
//! list cut in `bd` (`sync_utils::list_cut::kept`), and says how many it left out; every other entry stays whole. The
//! window reads the whole plan.

use review_plan::steps::{ReviewStep, ReviewSteps, StepCoverage, StepSource};
use review_plan::story::StoryCommit;
use schemars::JsonSchema;
use serde::Serialize;
use std::collections::HashSet;
use sync_utils::list_cut::kept;

/// How many generated files a list of the plan keeps
pub const GENERATED_SHOWN: usize = 20;

/// `items` with their generated ones past the first `GENERATED_SHOWN` left out, and the count of those
pub fn cut_generated<T>(items: Vec<T>, generated: impl Fn(&T) -> bool) -> (Vec<T>, usize) {
  let count = items.iter().filter(|item| generated(item)).count();
  let shown = kept(count, GENERATED_SHOWN);
  let mut seen = 0;
  let kept_items: Vec<T> = items
    .into_iter()
    .filter(|item| {
      if !generated(item) {
        return true;
      }
      seen += 1;
      seen <= shown
    })
    .collect();
  (kept_items, count - shown)
}

/// The steps of `brd review-plan`: those of the plan, each with its stops and paths of generated files cut
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct StepsOutput {
  pub source: StepSource,
  pub told: bool,
  pub steps: Vec<StepOutput>,
  pub coverage: StepCoverage,
  pub commits: Vec<StoryCommit>,
}

/// A step of `brd review-plan`
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct StepOutput {
  #[serde(flatten)]
  pub step: ReviewStep,
  /// Its stops of generated files past the first 20 of them, left out; the `start`, `tests` and `end` of its groups count
  /// the stops it shows
  #[serde(default, skip_serializing_if = "is_zero")]
  pub stops_omitted: usize,
  /// Its paths of generated files past the first 20 of them, left out
  #[serde(default, skip_serializing_if = "is_zero")]
  pub paths_omitted: usize,
}

#[expect(clippy::trivially_copy_pass_by_ref, reason = "the signature serde's skip_serializing_if calls")]
const fn is_zero(count: &usize) -> bool {
  *count == 0
}

/// `steps` with the stops of `generated_units` and the paths of `generated_paths` cut in each step
pub fn fold_steps(steps: ReviewSteps, generated_units: &HashSet<String>, generated_paths: &HashSet<String>) -> StepsOutput {
  StepsOutput {
    source: steps.source,
    told: steps.told,
    steps: steps.steps.into_iter().map(|step| fold_step(step, generated_units, generated_paths)).collect(),
    coverage: steps.coverage,
    commits: steps.commits,
  }
}

fn fold_step(mut step: ReviewStep, generated_units: &HashSet<String>, generated_paths: &HashSet<String>) -> StepOutput {
  let old = std::mem::take(&mut step.stops);
  let is_generated: Vec<bool> = old.iter().map(|stop| generated_units.contains(&stop.unit)).collect();
  let shown = kept(is_generated.iter().filter(|generated| **generated).count(), GENERATED_SHOWN);
  // the place of each stop among those shown: a group counts the stops it shows
  let mut before: Vec<u32> = Vec::with_capacity(old.len() + 1);
  let (mut kept_count, mut seen) = (0u32, 0usize);
  for (stop, generated) in old.into_iter().zip(&is_generated) {
    before.push(kept_count);
    if *generated {
      seen += 1;
      if seen > shown {
        continue;
      }
    }
    kept_count += 1;
    step.stops.push(stop);
  }
  before.push(kept_count);
  let place = |at: u32| before.get(at as usize).copied().unwrap_or(kept_count);
  for group in &mut step.groups {
    group.start = place(group.start);
    group.tests = place(group.tests);
    group.end = place(group.end);
  }
  step.groups.retain(|group| group.end > group.start);
  let (paths, paths_omitted) = cut_generated(std::mem::take(&mut step.paths), |path| generated_paths.contains(path));
  step.paths = paths;
  StepOutput {
    step,
    stops_omitted: seen - seen.min(shown),
    paths_omitted,
  }
}
