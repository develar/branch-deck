#[cfg(any(target_os = "macos", test))]
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// An event-log position of a volume
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Mark {
  pub event_id: u64,
  pub device: u64,
}

/// Whether the event log is complete enough to select subtrees
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Changes {
  All,
  Paths(Vec<PathBuf>),
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

/// The paths under `roots` changed since `mark`, or `All` if the event history cannot be trusted
pub fn changed_since(roots: &[&Path], mark: &Mark) -> Changes {
  platform::changed_since(roots, mark)
}

#[cfg(any(target_os = "macos", test))]
pub(crate) fn paths_under_roots(paths: impl IntoIterator<Item = PathBuf>, roots: &[PathBuf]) -> Vec<PathBuf> {
  paths
    .into_iter()
    .filter(|path| roots.iter().any(|root| path.starts_with(root)))
    .collect::<BTreeSet<_>>()
    .into_iter()
    .collect()
}

#[cfg(target_os = "macos")]
pub(crate) fn needs_full_for_link(path: &Path, renamed: bool) -> bool {
  platform::needs_full_for_link(path, renamed)
}

#[cfg(target_os = "macos")]
#[expect(unsafe_code, reason = "CoreServices FSEvents and CoreFoundation callbacks have no safe Rust bindings")]
#[expect(
  clippy::undocumented_unsafe_blocks,
  reason = "the FSEvents and CoreFoundation FFI is isolated and validates callback pointers before use"
)]
mod platform {
  use super::{Changes, Mark};
  use std::ffi::{CStr, CString, c_char, c_void};
  use std::os::unix::ffi::{OsStrExt, OsStringExt};
  use std::os::unix::fs::MetadataExt;
  use std::path::{Path, PathBuf};
  use std::ptr;

  type CFStringRef = *const c_void;
  type CFArrayRef = *const c_void;
  type CFRunLoopRef = *mut c_void;
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
  const ITEM_CREATED: u32 = 0x100;
  const ITEM_RENAMED: u32 = 0x800;
  const INCOMPLETE: u32 = MUST_SCAN_SUBDIRS | USER_DROPPED | KERNEL_DROPPED | EVENT_IDS_WRAPPED | ROOT_CHANGED | MOUNT | UNMOUNT;
  const MAX_EVENTS: usize = 1_000_000;

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

  struct EventData {
    run_loop: CFRunLoopRef,
    paths: Vec<PathBuf>,
    history_phase: HistoryPhase,
    incomplete: bool,
    hardlinks: bool,
  }

  #[derive(Debug, Clone, Copy, PartialEq, Eq)]
  enum HistoryPhase {
    AwaitingHistory,
    DrainingLiveEvents,
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
    fn FSEventStreamScheduleWithRunLoop(stream: FSEventStreamRef, run_loop: CFRunLoopRef, mode: CFStringRef);
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
    fn CFRunLoopGetCurrent() -> CFRunLoopRef;
    fn CFRunLoopRunInMode(mode: CFStringRef, seconds: f64, return_after_source_handled: u8) -> i32;
    fn CFRunLoopStop(run_loop: CFRunLoopRef);
  }

  pub(super) fn mark(path: &Path) -> Option<Mark> {
    let device = std::fs::metadata(path).ok()?.dev();
    let event_id = unsafe { FSEventsGetCurrentEventId() };
    Some(Mark { event_id, device })
  }

  pub(super) fn changed_since(roots: &[&Path], mark: &Mark) -> Changes {
    if roots.is_empty() {
      return Changes::Paths(Vec::new());
    }
    let Some(roots) = roots.iter().map(|root| super::canonical_path(root)).collect::<Option<Vec<_>>>() else {
      return Changes::All;
    };
    let Some(watch_paths) = roots.iter().map(|root| existing_ancestor(root)).collect::<Option<Vec<_>>>() else {
      return Changes::All;
    };
    let current_id = unsafe { FSEventsGetCurrentEventId() };
    if mark.event_id == 0
      || mark.event_id > current_id
      || watch_paths
        .iter()
        .any(|root| std::fs::metadata(root).map_or(true, |metadata| metadata.dev() != mark.device))
    {
      return Changes::All;
    }
    if mark.event_id == current_id {
      return Changes::Paths(Vec::new());
    }
    let root_refs: Vec<&Path> = watch_paths.iter().map(PathBuf::as_path).collect();
    let Some(paths) = paths_array(&root_refs) else {
      return Changes::All;
    };
    let data = Box::new(EventData {
      run_loop: ptr::null_mut(),
      paths: Vec::new(),
      history_phase: HistoryPhase::AwaitingHistory,
      incomplete: false,
      hardlinks: false,
    });
    let data = Box::into_raw(data);
    let mut context = FSEventStreamContext {
      version: 0,
      info: data.cast(),
      retain: None,
      release: None,
      copy_description: None,
    };
    let flags = NO_DEFER | FILE_EVENTS;
    let stream = unsafe { FSEventStreamCreate(ptr::null(), Some(event), ptr::from_mut(&mut context), paths.array, mark.event_id, 0.0, flags) };
    if stream.is_null() {
      paths.release();
      unsafe { drop(Box::from_raw(data)) };
      return Changes::All;
    }
    let run_loop = unsafe { CFRunLoopGetCurrent() };
    unsafe { (*data).run_loop = run_loop };
    unsafe {
      FSEventStreamScheduleWithRunLoop(stream, run_loop, kCFRunLoopDefaultMode);
    }
    let started = unsafe { FSEventStreamStart(stream) != 0 };
    if started {
      unsafe {
        FSEventStreamFlushAsync(stream);
        CFRunLoopRunInMode(kCFRunLoopDefaultMode, 0.5, 0);
      }
      if unsafe { (*data).history_phase == HistoryPhase::DrainingLiveEvents } {
        unsafe { CFRunLoopRunInMode(kCFRunLoopDefaultMode, 0.01, 0) };
      }
      unsafe {
        FSEventStreamStop(stream);
      }
    }
    unsafe {
      FSEventStreamInvalidate(stream);
      FSEventStreamRelease(stream);
      paths.release();
    }
    let mut data = unsafe { Box::from_raw(data) };
    if !started || data.history_phase != HistoryPhase::DrainingLiveEvents || data.incomplete || data.hardlinks {
      return Changes::All;
    }
    Changes::Paths(super::paths_under_roots(std::mem::take(&mut data.paths), &roots))
  }

  pub(super) fn needs_full_for_link(path: &Path, renamed: bool) -> bool {
    std::fs::symlink_metadata(path).is_ok_and(|metadata| (metadata.is_dir() && renamed) || (metadata.is_file() && metadata.nlink() > 1))
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

  unsafe extern "C" fn event(_stream: FSEventStreamRef, info: *mut c_void, count: usize, event_paths: *mut c_void, event_flags: *const u32, _event_ids: *const u64) {
    if info.is_null() || count == 0 {
      return;
    }
    let data = unsafe { &mut *info.cast::<EventData>() };
    if event_flags.is_null() || event_paths.is_null() {
      data.incomplete = true;
      return;
    }
    let flags = unsafe { std::slice::from_raw_parts(event_flags, count) };
    let paths = event_paths.cast::<*const c_char>();
    for (index, flag) in flags.iter().copied().enumerate() {
      if flag & HISTORY_DONE != 0 {
        if data.history_phase == HistoryPhase::AwaitingHistory {
          data.history_phase = HistoryPhase::DrainingLiveEvents;
          unsafe { CFRunLoopStop(data.run_loop) };
        }
        continue;
      }
      if flag & INCOMPLETE != 0 || data.paths.len() >= MAX_EVENTS {
        data.incomplete = true;
        continue;
      }
      let path = unsafe { *paths.add(index) };
      if path.is_null() {
        data.incomplete = true;
        continue;
      }
      let bytes = unsafe { CStr::from_ptr(path) }.to_bytes().to_vec();
      let path = PathBuf::from(std::ffi::OsString::from_vec(bytes));
      if flag & (ITEM_CREATED | ITEM_RENAMED) != 0 {
        data.hardlinks |= super::needs_full_for_link(&path, flag & ITEM_RENAMED != 0);
      }
      data.paths.push(path);
    }
  }
}

#[cfg(not(target_os = "macos"))]
mod platform {
  use super::{Changes, Mark};
  use std::path::Path;

  pub(super) fn mark(_path: &Path) -> Option<Mark> {
    None
  }

  pub(super) fn changed_since(_roots: &[&Path], _mark: &Mark) -> Changes {
    Changes::All
  }
}
