//! The read buffer of a walk (openspec/specs/cli/lease/spec.md, "Disk"): on macOS each thread reads every directory
//! into one buffer of its own for `getattrlistbulk`, made when the thread takes its first directory. A global allocator
//! counts the allocations of the size of that buffer while a walk runs; an integration test, so that it counts in no
//! unit test of the crate. Linux and Windows read with `read_dir`, which asks for no buffer of that size

#![expect(unsafe_code, reason = "a counting allocator")]

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

/// The size of the read buffer of `getattrlistbulk` (`BUFFER` of the macOS reader)
const BUFFER: usize = 128 * 1024;
/// How many threads walk the directories (`WALK_WORKERS`)
const WALK_WORKERS: usize = 8;

static COUNTING: AtomicBool = AtomicBool::new(false);
static BUFFERS: AtomicUsize = AtomicUsize::new(0);

/// `System`, counting each request of `BUFFER` bytes while `COUNTING` is on: `vec![0u8; BUFFER]` asks `alloc_zeroed`
struct Counting;

impl Counting {
  fn count(size: usize) {
    if size == BUFFER && COUNTING.load(Ordering::Relaxed) {
      BUFFERS.fetch_add(1, Ordering::Relaxed);
    }
  }
}

// SAFETY: every call goes to `System` with the arguments of the caller; the count changes no allocation
unsafe impl GlobalAlloc for Counting {
  unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
    Self::count(layout.size());
    // SAFETY: the layout of the caller, as `GlobalAlloc::alloc` requires it
    unsafe { System.alloc(layout) }
  }

  unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
    Self::count(layout.size());
    // SAFETY: the layout of the caller, as `GlobalAlloc::alloc_zeroed` requires it
    unsafe { System.alloc_zeroed(layout) }
  }

  unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
    // SAFETY: a pointer that this allocator gave, with its layout
    unsafe { System.dealloc(ptr, layout) }
  }

  unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
    Self::count(new_size);
    // SAFETY: a pointer that this allocator gave, with its layout and a new size, as `GlobalAlloc::realloc` requires
    unsafe { System.realloc(ptr, layout, new_size) }
  }
}

#[global_allocator]
static ALLOCATOR: Counting = Counting;

/// The read buffers that `walk` allocates
fn buffers_of(walk: impl FnOnce()) -> usize {
  BUFFERS.store(0, Ordering::Relaxed);
  COUNTING.store(true, Ordering::Relaxed);
  walk();
  COUNTING.store(false, Ordering::Relaxed);
  BUFFERS.load(Ordering::Relaxed)
}

#[test]
fn a_walk_allocates_one_read_buffer_per_thread_not_per_directory() {
  let root = tempfile::tempdir().unwrap();
  for index in 0..200 {
    let dir = root.path().join(format!("dir-{index}"));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("a"), b"a").unwrap();
    std::fs::write(dir.join("b"), b"b").unwrap();
  }
  let walks = [fs_walk::Walk::of(root.path())];
  let mut bytes = Vec::new();
  let many = buffers_of(|| bytes = fs_walk::walk_bytes(&walks));
  assert!(bytes[0] > 0, "{bytes:?}");
  assert!(many <= WALK_WORKERS, "{many} read buffers for 201 directories");
  // a thread makes its buffer with its first directory: a walk of one directory starts 8 threads and makes one buffer
  let one = buffers_of(|| {
    fs_walk::tree_size(&root.path().join("dir-0"));
  });
  assert!(one <= 1, "{one} read buffers for one directory");
}
