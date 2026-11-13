//! What the ranges a review window shows leave to read (openspec/specs/review/range-reading/spec.md): the pull request
//! or the scope in its header, every layer of a stack. One read for all of them (`review_plan::reading::layer_readings`):
//! the diffs of every range in one phase each, a `diff-tree --stdin` of their lists and a `cat-file --batch` of the
//! blobs no cache has, then the index asked of their renames range by range; never a plan per range.
//!
//! - **On the channel** of the command: `disabled` when the repository turns the review plan off
//!   (`branchdeck.reviewExperimental`), else the readings as they stand, each with its verdict and the author's word
//!   checked against it (`domain_policy::range_reading::reading_view`) and their sum (`stack_summary`), once more as each
//!   range is read or the index answers it.
//! - **Memory** (`ReviewPlans::readings`): a reading no rename of which waits for the index, nor the index left
//!   unanswered, is kept by its repository and its two commits, which never change: a window that shows the range again
//!   reads no git.
//! - **One read per window** (`Windows::begin_range_readings`): a newer one of the window, its close or its reload
//!   supersedes the one in flight, whose queries of the index are killed; it ends without another event.

use crate::context::AppContext;
use crate::operation::{operation, run_blocking};
use crate::reads::review_plan::PlanReader;
use crate::transport::{Call, Sink};
use domain_policy::range_reading::{reading_view, stack_summary};
use domain_types::error::ApiError;
use domain_types::range_reading::{RangeReading, RangeReadingView, StackReading, Unproven};
use review_plan::PlanSources;
use review_plan::reading::layer_readings;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use sync_utils::child_process::ChildPriority;
use tracing::{debug, instrument};

/// The parameters of `get_range_readings`
#[derive(Debug, Clone, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct GetRangeReadingsParams {
  pub repository_path: String,
  /// The ranges, the bottom of a stack first
  pub ranges: Vec<ReadingRange>,
}

/// A range to read, with the words of the author of its pull request
#[derive(Debug, Clone, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ReadingRange {
  /// Full commit ids, as a review fetch answers them
  pub base: String,
  pub head: String,
  /// The title of its pull request; absent for a range of no pull request, whose author's word is none
  #[serde(default)]
  #[specta(optional)]
  pub title: Option<String>,
  #[serde(default)]
  #[specta(optional)]
  pub labels: Vec<String>,
}

/// What `get_range_readings` tells its page
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum RangeReadingsEvent {
  /// The repository turns the review plan off: no reading
  Disabled,
  /// The readings as they stand, by the place of each range, `None` for one not read yet; their sum, of the layers of a
  /// stack
  Readings { readings: Vec<Option<RangeReadingView>>, stack: StackReading },
}

/// What memory keeps a reading by: the repository and the two commits of the range
pub(crate) type ReadingKey = (String, String, String);

/// A reading is what it would be whenever it is asked: no rename waits for the index, and an index that is there left
/// none unanswered (its clock may have cut the query)
fn is_final(reading: &RangeReading, with_index: bool) -> bool {
  let unanswered = with_index && reading.unproven.iter().any(|file| file.why == Unproven::RenamesUnanswered);
  !reading.pending() && !unanswered
}

/// The readings of `params.ranges` on `events` (module doc), as the read of the window of `call`
#[instrument(skip_all, fields(repository_path = %params.repository_path, ranges = params.ranges.len()))]
pub async fn get_range_readings(ctx: &AppContext, call: &Call, params: GetRangeReadingsParams, events: Sink<RangeReadingsEvent>) -> Result<(), ApiError> {
  let request = ctx.windows.begin_range_readings(&call.window);
  let (reader, windows, call) = (PlanReader::of(ctx), Arc::clone(&ctx.windows), call.clone());
  operation(async move {
    run_blocking(move || {
      let canceled = || request.is_superseded() || !windows.shows(&call);
      read_ranges(&reader, &params, &canceled, &mut |event| {
        if !canceled() && events.send(event).is_err() {
          debug!("the page of the readings went away");
        }
      })
    })
    .await
  })
  .await
}

/// The readings of `params` told to `send`: those memory keeps at once, then the others as they are read. Blocking
pub(crate) fn read_ranges(
  reader: &PlanReader,
  params: &GetRangeReadingsParams,
  canceled: &(dyn Fn() -> bool + Sync),
  send: &mut dyn FnMut(RangeReadingsEvent),
) -> anyhow::Result<()> {
  let repository_path = params.repository_path.as_str();
  if !reader.enabled(repository_path)? {
    send(RangeReadingsEvent::Disabled);
    return Ok(());
  }
  let key = |range: &ReadingRange| -> ReadingKey { (repository_path.to_string(), range.base.clone(), range.head.clone()) };
  let mut readings: Vec<Option<RangeReading>> = params
    .ranges
    .iter()
    .map(|range| reader.plans.readings.get(&key(range)).map(|kept| (*kept).clone()))
    .collect();
  let missing: Vec<usize> = (0..params.ranges.len()).filter(|&index| readings[index].is_none()).collect();
  let event = |readings: &[Option<RangeReading>]| {
    let views: Vec<Option<RangeReadingView>> = readings
      .iter()
      .zip(&params.ranges)
      .map(|(reading, range)| reading.clone().map(|reading| reading_view(reading, range.title.as_deref(), &range.labels)))
      .collect();
    let stack = stack_summary(&views.iter().map(|view| view.as_ref().map(|view| &view.summary)).collect::<Vec<_>>());
    RangeReadingsEvent::Readings { readings: views, stack }
  };
  if missing.is_empty() || readings.iter().any(Option::is_some) {
    send(event(&readings));
  }
  if missing.is_empty() || canceled() {
    return Ok(());
  }
  let cache = reader.object_cache(repository_path);
  let index = reader.indexes.search(canceled, ChildPriority::Normal);
  let with_index = index.is_some();
  let sources = PlanSources {
    cache: &cache,
    file_diffs: reader.file_diffs(),
    index: index.as_deref(),
  };
  let ranges: Vec<(String, String)> = missing
    .iter()
    .map(|&index| (params.ranges[index].base.clone(), params.ranges[index].head.clone()))
    .collect();
  layer_readings(reader.git(), sources, repository_path, &ranges, |at, reading| {
    let index = missing[at];
    if is_final(&reading, with_index) && !canceled() {
      reader.plans.readings.insert(key(&params.ranges[index]), Arc::new(reading.clone()));
    }
    readings[index] = Some(reading);
    send(event(&readings));
  })
}
