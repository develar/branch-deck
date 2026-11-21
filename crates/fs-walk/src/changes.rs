//! What changed under some trees since a mark of the event log of their volume (`FSEvents` on macOS), for the disk
//! report (openspec/specs/repository/disk-usage/spec.md): one replay of the history for every query of a volume whose
//! marks lie close together, each event given to the queries under whose roots it lies and whose mark it follows. A
//! query is answered or refused with the reason; a replay that does not end by the deadline refuses its queries as
//! `Late`. Elsewhere there is no history: every query is `Unavailable`.
//!
//! The new mark of a measure is taken before the replay starts and kept only after its walk: a write that the log has
//! not shown yet has a later id than that mark, so the next replay names it, and no marker event is needed.

#[cfg(any(target_os = "macos", test))]
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// An event-log position of a volume
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Mark {
  pub event_id: u64,
  pub device: u64,
}

/// Why the history names no changes of a query
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HistoryRefusal {
  /// The replay did not end by the deadline: the history is likely complete, only slow (an overloaded `fseventsd`)
  Late,
  /// The log lost events under the roots (`MustScanSubDirs`, dropped events, wrapped ids, a mount, a moved root)
  Dropped,
  /// The mark is of another volume than the roots
  OtherDevice,
  /// The mark is no event id
  NoMark,
  /// The mark is past the current event of the log: it came from another boot or volume
  Future,
  /// The platform keeps no history, or the stream could not start
  Unavailable,
}

/// The trees of a query and the mark of their last measure
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoryQuery {
  pub roots: Vec<PathBuf>,
  pub mark: Mark,
}

/// A canonical absolute path even when its final directories have not been created
pub fn canonical_path(path: &Path) -> Option<PathBuf> {
  let absolute = if path.is_absolute() {
    path.to_path_buf()
  } else {
    std::env::current_dir().ok()?.join(path)
  };
  let mut missing = Vec::new();
  let mut ancestor = absolute.as_path();
  loop {
    if let Ok(mut canonical) = std::fs::canonicalize(ancestor) {
      for component in missing.iter().rev() {
        canonical.push(component);
      }
      return Some(canonical);
    }
    missing.push(ancestor.file_name()?.to_os_string());
    ancestor = ancestor.parent()?;
  }
}

/// The current event position of the volume containing `path`; unavailable unless the platform has `FSEvents`
pub fn mark(path: &Path) -> Option<Mark> {
  platform::mark(path)
}

/// What the history names under the roots of a query
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Change {
  /// A file or a directory changed
  Path,
  /// The events under the directory were lost (`MustScanSubDirs`, a mount): what it holds is to measure again
  Subtree,
}

/// The paths changed under the roots of each query since its mark, given to `on_change` with the index of the query as
/// the history names them (a path may come more than once, and a path of a query that is refused later may come before
/// its refusal); the answer of each query. The replays of all queries end by `deadline`
pub fn changed_since(queries: &[HistoryQuery], deadline: Duration, on_change: &mut dyn FnMut(usize, &Path, Change)) -> Vec<Result<(), HistoryRefusal>> {
  platform::changed_since(queries, deadline, on_change)
}

/// The queries of one replay with their canonical roots: it gives each event to the queries it belongs to. A replay of
/// a busy volume brings millions of events faster than a slow reader takes them, and the events a reader does not take in
/// time are lost (`UserDropped`): a lookup of the ancestors of a path, not a scan of the roots
#[cfg(any(target_os = "macos", test))]
pub(crate) struct Router {
  /// The index of each query among those of `changed_since` and its mark
  queries: Vec<(usize, u64)>,
  /// The position in `queries` of the query of each root, by the bytes of the root
  roots: HashMap<Vec<u8>, usize>,
  /// Each root with the position of its query, for a lost subtree above roots
  root_paths: Vec<(PathBuf, usize)>,
  /// The refusal of each query of the replay so far, by the position in `queries`
  pub(crate) refused: Vec<Option<HistoryRefusal>>,
  pub(crate) done: bool,
  /// The highest event id the replay reached, and how many events it gave: for the log of a replay
  pub(crate) reached: u64,
  pub(crate) events: u64,
}

/// The flags of an event that `Router::event` reads
#[cfg(any(target_os = "macos", test))]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct EventFlags {
  /// `HistoryDone`: the replay reached the present
  pub history_done: bool,
  /// The events under the path were lost (`MustScanSubDirs`, with or without the drop that caused it), or a volume was
  /// mounted there
  pub rescan: bool,
  /// Wrapped ids, a drop that names no subtree: the history of the stream is not whole
  pub lost: bool,
}

#[cfg(any(target_os = "macos", test))]
impl Router {
  /// `queries`: the index of each query, its roots (canonical) and the event id of its mark
  pub(crate) fn new(queries: Vec<(usize, Vec<PathBuf>, u64)>) -> Self {
    let refused = vec![None; queries.len()];
    let mut roots = HashMap::new();
    let mut root_paths = Vec::new();
    for (position, (_, query_roots, _)) in queries.iter().enumerate() {
      for root in query_roots {
        roots.insert(root.as_os_str().as_encoded_bytes().to_vec(), position);
        root_paths.push((root.clone(), position));
      }
    }
    Self {
      queries: queries.into_iter().map(|(index, _, mark)| (index, mark)).collect(),
      roots,
      root_paths,
      refused,
      done: false,
      reached: 0,
      events: 0,
    }
  }

  /// The index of each query among those of `changed_since`
  #[cfg_attr(not(target_os = "macos"), expect(dead_code, reason = "the replay of macOS asks it, and no test"))]
  pub(crate) fn indexes(&self) -> Vec<usize> {
    self.queries.iter().map(|(index, _)| *index).collect()
  }

  /// The event `id` of `path` with its `flags`: a change under the roots of a query whose mark it follows goes to
  /// `on_change`, a lost subtree as one to measure again; a loss of the whole history refuses every query
  pub(crate) fn event(&mut self, path: &Path, id: u64, flags: EventFlags, on_change: &mut dyn FnMut(usize, &Path, Change)) {
    if flags.history_done {
      self.done = true;
      return;
    }
    self.reached = self.reached.max(id);
    self.events += 1;
    let bytes = path.as_os_str().as_encoded_bytes();
    let separator = u8::try_from(std::path::MAIN_SEPARATOR).unwrap_or(b'/');
    if flags.lost || (flags.rescan && bytes.iter().all(|byte| *byte == separator)) {
      tracing::debug!(path = %path.display(), id, "the history lost events");
      self.refused.iter_mut().for_each(|refused| *refused = Some(HistoryRefusal::Dropped));
      return;
    }
    let change = if flags.rescan { Change::Subtree } else { Change::Path };
    // the roots that hold the path: the path itself and each of its ancestors
    let mut end = bytes.len();
    let mut last = None;
    loop {
      if let Some(&position) = self.roots.get(&bytes[..end])
        && last != Some(position)
      {
        last = Some(position);
        let (index, mark) = self.queries[position];
        if id > mark && self.refused[position].is_none() {
          on_change(index, path, change);
        }
      }
      match bytes[..end].iter().rposition(|byte| *byte == separator) {
        Some(at) if at > 0 => end = at,
        _ => break,
      }
    }
    // a lost subtree above roots: each root under it is to measure again
    if flags.rescan {
      for (root, position) in &self.root_paths {
        if root.as_path() != path && root.starts_with(path) {
          let (index, mark) = self.queries[*position];
          if id > mark && self.refused[*position].is_none() {
            on_change(index, root, Change::Subtree);
          }
        }
      }
    }
  }
}

#[cfg(target_os = "macos")]
#[expect(unsafe_code, reason = "CoreServices FSEvents and CoreFoundation callbacks have no safe Rust bindings")]
#[expect(
  clippy::undocumented_unsafe_blocks,
  reason = "the FSEvents and CoreFoundation FFI is isolated and validates callback pointers before use"
)]
mod platform {
  use super::{Change, EventFlags, HistoryQuery, HistoryRefusal, Mark, Router};
  use std::cell::RefCell;
  use std::ffi::{CStr, CString, c_char, c_void};
  use std::os::unix::ffi::{OsStrExt, OsStringExt};
  use std::os::unix::fs::MetadataExt;
  use std::path::{Path, PathBuf};
  use std::ptr;
  use std::time::{Duration, Instant};

  type CFStringRef = *const c_void;
  type CFArrayRef = *const c_void;
  type FSEventStreamRef = *mut c_void;

  const UTF8: u32 = 0x0800_0100;
  const FILE_EVENTS: u32 = 0x10;
  const NO_DEFER: u32 = 0x02;
  const MUST_SCAN_SUBDIRS: u32 = 0x01;
  const USER_DROPPED: u32 = 0x02;
  const KERNEL_DROPPED: u32 = 0x04;
  const EVENT_IDS_WRAPPED: u32 = 0x08;
  const HISTORY_DONE: u32 = 0x10;
  const ROOT_CHANGED: u32 = 0x20;
  const MOUNT: u32 = 0x40;
  const UNMOUNT: u32 = 0x80;
  /// Marks further apart than this many events replay apart: the replay from the oldest would make every query of the
  /// stream wait for a history that the others do not need
  const MARK_GAP: u64 = 20_000_000;

  #[repr(C)]
  struct FSEventStreamContext {
    version: isize,
    info: *mut c_void,
    retain: Option<unsafe extern "C" fn(*const c_void) -> *const c_void>,
    release: Option<unsafe extern "C" fn(*const c_void)>,
    copy_description: Option<unsafe extern "C" fn(*const c_void) -> CFStringRef>,
  }

  #[repr(C)]
  struct CFArrayCallBacks {
    _fields: [usize; 5],
  }

  /// The receiver of the changes of a replay
  type OnChange<'b> = &'b mut dyn FnMut(usize, &Path, Change);

  /// What the callback of a stream reaches: the router of its queries and the receiver of their paths, which the
  /// streams of a replay share on the one thread of the run loop
  struct EventData<'a, 'b> {
    router: Router,
    on_change: &'a RefCell<OnChange<'b>>,
    since: u64,
    watch: usize,
    /// How long after the start the history was done
    done_in: Option<Duration>,
    started_at: Instant,
  }

  /// A query to replay: its index, its canonical roots, the paths to watch for them and its mark
  struct Replay {
    index: usize,
    roots: Vec<PathBuf>,
    watch: Vec<PathBuf>,
    mark: Mark,
  }

  #[link(name = "CoreServices", kind = "framework")]
  unsafe extern "C" {
    fn FSEventsGetCurrentEventId() -> u64;
    fn FSEventStreamCreate(
      allocator: *const c_void,
      callback: Option<unsafe extern "C" fn(FSEventStreamRef, *mut c_void, usize, *mut c_void, *const u32, *const u64)>,
      context: *mut FSEventStreamContext,
      paths: CFArrayRef,
      since_when: u64,
      latency: f64,
      flags: u32,
    ) -> FSEventStreamRef;
    fn FSEventStreamScheduleWithRunLoop(stream: FSEventStreamRef, run_loop: *mut c_void, mode: CFStringRef);
    fn FSEventStreamStart(stream: FSEventStreamRef) -> u8;
    fn FSEventStreamFlushAsync(stream: FSEventStreamRef);
    fn FSEventStreamStop(stream: FSEventStreamRef);
    fn FSEventStreamInvalidate(stream: FSEventStreamRef);
    fn FSEventStreamRelease(stream: FSEventStreamRef);
  }

  #[link(name = "CoreFoundation", kind = "framework")]
  unsafe extern "C" {
    static kCFRunLoopDefaultMode: CFStringRef;
    fn CFStringCreateWithCString(allocator: *const c_void, value: *const c_char, encoding: u32) -> CFStringRef;
    static kCFTypeArrayCallBacks: CFArrayCallBacks;
    fn CFArrayCreate(allocator: *const c_void, values: *const *const c_void, count: isize, callbacks: *const CFArrayCallBacks) -> CFArrayRef;
    fn CFRelease(value: *const c_void);
    fn CFRunLoopGetCurrent() -> *mut c_void;
    fn CFRunLoopRunInMode(mode: CFStringRef, seconds: f64, return_after_source_handled: u8) -> i32;
  }

  pub(super) fn mark(path: &Path) -> Option<Mark> {
    let device = std::fs::metadata(path).ok()?.dev();
    let event_id = unsafe { FSEventsGetCurrentEventId() };
    Some(Mark { event_id, device })
  }

  pub(super) fn changed_since(queries: &[HistoryQuery], deadline: Duration, on_change: &mut dyn FnMut(usize, &Path, Change)) -> Vec<Result<(), HistoryRefusal>> {
    let until = Instant::now() + deadline;
    let current_id = unsafe { FSEventsGetCurrentEventId() };
    let mut answers: Vec<Result<(), HistoryRefusal>> = vec![Ok(()); queries.len()];
    let mut replays: Vec<Replay> = Vec::new();
    for (index, query) in queries.iter().enumerate() {
      let mark = query.mark;
      if mark.event_id == 0 {
        answers[index] = Err(HistoryRefusal::NoMark);
        continue;
      }
      if mark.event_id > current_id {
        answers[index] = Err(HistoryRefusal::Future);
        continue;
      }
      let roots: Option<Vec<PathBuf>> = query.roots.iter().map(|root| super::canonical_path(root)).collect();
      let watch: Option<Vec<PathBuf>> = roots.as_ref().and_then(|roots| roots.iter().map(|root| existing_ancestor(root)).collect());
      let (Some(roots), Some(watch)) = (roots, watch) else {
        answers[index] = Err(HistoryRefusal::Unavailable);
        continue;
      };
      if watch.iter().any(|path| std::fs::metadata(path).map_or(true, |metadata| metadata.dev() != mark.device)) {
        answers[index] = Err(HistoryRefusal::OtherDevice);
      } else if mark.event_id < current_id && !roots.is_empty() {
        replays.push(Replay { index, roots, watch, mark });
      }
    }
    replays.sort_by_key(|replay| (replay.mark.device, replay.mark.event_id));
    let mut groups: Vec<Vec<Replay>> = Vec::new();
    for replay in replays {
      match groups.last_mut() {
        Some(group) if group[0].mark.device == replay.mark.device && replay.mark.event_id - group[0].mark.event_id <= MARK_GAP => group.push(replay),
        _ => groups.push(vec![replay]),
      }
    }
    let on_change = RefCell::new(on_change);
    let streams: Vec<(Vec<PathBuf>, u64, Router)> = groups
      .into_iter()
      .map(|group| {
        let since = group[0].mark.event_id;
        let mut watch: Vec<PathBuf> = group.iter().flat_map(|replay| replay.watch.iter().cloned()).collect();
        watch.sort();
        watch.dedup();
        (
          watch,
          since,
          Router::new(group.into_iter().map(|replay| (replay.index, replay.roots, replay.mark.event_id)).collect()),
        )
      })
      .collect();
    for (index, refusal) in replay(streams, until, &on_change) {
      if let Some(refusal) = refusal {
        answers[index] = Err(refusal);
      }
    }
    answers
  }

  /// A stream over its paths from its mark, all on the run loop of this thread at once, until each history is done or
  /// `until`: each query of their routers by its index, with its refusal
  fn replay(streams: Vec<(Vec<PathBuf>, u64, Router)>, until: Instant, on_change: &RefCell<OnChange<'_>>) -> Vec<(usize, Option<HistoryRefusal>)> {
    let started_at = Instant::now();
    let mut datas: Vec<Box<EventData<'_, '_>>> = Vec::new();
    let mut running: Vec<(FSEventStreamRef, PathsArray, bool)> = Vec::new();
    let mut answers = Vec::new();
    for (watch, since, router) in streams {
      let indexes = router.indexes();
      let watch_refs: Vec<&Path> = watch.iter().map(PathBuf::as_path).collect();
      let Some(paths) = paths_array(&watch_refs) else {
        answers.extend(indexes.into_iter().map(|index| (index, Some(HistoryRefusal::Unavailable))));
        continue;
      };
      let mut data = Box::new(EventData {
        router,
        on_change,
        since,
        watch: watch.len(),
        done_in: None,
        started_at,
      });
      let mut context = FSEventStreamContext {
        version: 0,
        info: ptr::from_mut(&mut *data).cast(),
        retain: None,
        release: None,
        copy_description: None,
      };
      let stream = unsafe { FSEventStreamCreate(ptr::null(), Some(event), ptr::from_mut(&mut context), paths.array, since, 0.0, NO_DEFER | FILE_EVENTS) };
      if stream.is_null() {
        paths.release();
        answers.extend(indexes.into_iter().map(|index| (index, Some(HistoryRefusal::Unavailable))));
        continue;
      }
      unsafe { FSEventStreamScheduleWithRunLoop(stream, CFRunLoopGetCurrent(), kCFRunLoopDefaultMode) };
      let started = unsafe { FSEventStreamStart(stream) != 0 };
      if started {
        unsafe { FSEventStreamFlushAsync(stream) };
      }
      running.push((stream, paths, started));
      datas.push(data);
    }
    // the histories come in batches, a source of the loop each: run it in short steps until every `HistoryDone` or the
    // deadline, as the replay of accio does
    while datas.iter().zip(&running).any(|(data, (_, _, started))| *started && !data.router.done) && Instant::now() < until {
      unsafe { CFRunLoopRunInMode(kCFRunLoopDefaultMode, 0.01, 1) };
    }
    let current = unsafe { FSEventsGetCurrentEventId() };
    for (data, (stream, paths, started)) in datas.into_iter().zip(running) {
      unsafe {
        if started {
          FSEventStreamStop(stream);
        }
        FSEventStreamInvalidate(stream);
        FSEventStreamRelease(stream);
      }
      paths.release();
      let indexes = data.router.indexes();
      if !started {
        answers.extend(indexes.into_iter().map(|index| (index, Some(HistoryRefusal::Unavailable))));
        continue;
      }
      let late = !data.router.done;
      tracing::debug!(
        since = data.since,
        reached = data.router.reached,
        current,
        events = data.router.events,
        watch = data.watch,
        done_ms = data.done_in.map(|done| done.as_millis()),
        late,
        "the replay of the history"
      );
      let EventData { router, .. } = *data;
      answers.extend(
        indexes
          .into_iter()
          .zip(router.refused)
          .map(|(index, refused)| (index, refused.or_else(|| late.then_some(HistoryRefusal::Late)))),
      );
    }
    answers
  }

  fn existing_ancestor(path: &Path) -> Option<PathBuf> {
    let mut ancestor = path;
    loop {
      if ancestor.exists() {
        return std::fs::canonicalize(ancestor).ok();
      }
      ancestor = ancestor.parent()?;
    }
  }

  struct PathsArray {
    strings: Vec<CFStringRef>,
    array: CFArrayRef,
  }

  impl PathsArray {
    fn release(self) {
      unsafe {
        CFRelease(self.array);
        for string in self.strings {
          CFRelease(string);
        }
      }
    }
  }

  fn paths_array(roots: &[&Path]) -> Option<PathsArray> {
    let mut strings = Vec::with_capacity(roots.len());
    for root in roots {
      let Ok(path) = CString::new(root.as_os_str().as_bytes()) else {
        release_strings(&strings);
        return None;
      };
      let string = unsafe { CFStringCreateWithCString(ptr::null(), path.as_ptr(), UTF8) };
      if string.is_null() {
        release_strings(&strings);
        return None;
      }
      strings.push(string);
    }
    let Ok(count) = isize::try_from(strings.len()) else {
      release_strings(&strings);
      return None;
    };
    let array = unsafe { CFArrayCreate(ptr::null(), strings.as_ptr(), count, ptr::addr_of!(kCFTypeArrayCallBacks)) };
    if array.is_null() {
      release_strings(&strings);
      return None;
    }
    Some(PathsArray { strings, array })
  }

  fn release_strings(strings: &[CFStringRef]) {
    unsafe {
      for string in strings {
        CFRelease(*string);
      }
    }
  }

  unsafe extern "C" fn event(_stream: FSEventStreamRef, info: *mut c_void, count: usize, event_paths: *mut c_void, event_flags: *const u32, event_ids: *const u64) {
    if info.is_null() || count == 0 {
      return;
    }
    let data = unsafe { &mut *info.cast::<EventData<'_, '_>>() };
    if event_flags.is_null() || event_paths.is_null() || event_ids.is_null() {
      data.router.refused.iter_mut().for_each(|refused| *refused = Some(HistoryRefusal::Dropped));
      return;
    }
    let flags = unsafe { std::slice::from_raw_parts(event_flags, count) };
    let ids = unsafe { std::slice::from_raw_parts(event_ids, count) };
    let paths = event_paths.cast::<*const c_char>();
    for (index, (&flag, &id)) in flags.iter().zip(ids).enumerate() {
      let path = unsafe { *paths.add(index) };
      let path = if path.is_null() {
        PathBuf::from("/")
      } else {
        PathBuf::from(std::ffi::OsString::from_vec(unsafe { CStr::from_ptr(path) }.to_bytes().to_vec()))
      };
      // a drop comes with `MustScanSubDirs` and the subtree whose events it lost; one without it loses all
      let flags = EventFlags {
        history_done: flag & HISTORY_DONE != 0,
        rescan: flag & (MUST_SCAN_SUBDIRS | MOUNT | UNMOUNT | ROOT_CHANGED) != 0,
        lost: flag & EVENT_IDS_WRAPPED != 0 || (flag & (USER_DROPPED | KERNEL_DROPPED) != 0 && flag & MUST_SCAN_SUBDIRS == 0),
      };
      data.router.event(&path, id, flags, &mut *data.on_change.borrow_mut());
      if flags.history_done && data.done_in.is_none() {
        data.done_in = Some(data.started_at.elapsed());
      }
    }
  }
}

#[cfg(not(target_os = "macos"))]
mod platform {
  use super::{Change, HistoryQuery, HistoryRefusal, Mark};
  use std::path::Path;
  use std::time::Duration;

  #[expect(clippy::missing_const_for_fn, reason = "the same signature as the FSEvents reader of macOS, which is no const fn")]
  pub(super) fn mark(_path: &Path) -> Option<Mark> {
    None
  }

  pub(super) fn changed_since(queries: &[HistoryQuery], _deadline: Duration, _on_change: &mut dyn FnMut(usize, &Path, Change)) -> Vec<Result<(), HistoryRefusal>> {
    vec![Err(HistoryRefusal::Unavailable); queries.len()]
  }
}
