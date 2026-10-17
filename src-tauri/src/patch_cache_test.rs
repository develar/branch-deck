use crate::patch_cache::{PatchCache, PatchWarmer};
use git_ops::commit_files::{CommitPatch, DiffSubject, FilePatch, PatchSource};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Barrier, Mutex};
use std::time::Duration;

const ID: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

fn patch(text: &str) -> CommitPatch {
  CommitPatch {
    commit_id: ID.to_string(),
    files: vec![FilePatch {
      patch: text.to_string(),
      omitted_lines: None,
    }],
  }
}

fn no_warm() -> PatchCache {
  PatchCache::with_warm(|_, _| {})
}

#[test]
fn concurrent_readers_of_a_patch_run_one_load() {
  let cache = no_warm();
  let subject = DiffSubject::Commit(ID.to_string());
  let loads = AtomicUsize::new(0);
  let start = Barrier::new(8);
  let patches: Vec<Arc<CommitPatch>> = std::thread::scope(|scope| {
    let readers: Vec<_> = (0..8)
      .map(|_| {
        scope.spawn(|| {
          start.wait();
          cache
            .get("/repo", &subject, || {
              loads.fetch_add(1, Ordering::SeqCst);
              // long enough that the other readers come while it loads
              std::thread::sleep(Duration::from_millis(50));
              Ok((patch("diff --git a/x b/x\n"), PatchSource::Git))
            })
            .unwrap()
        })
      })
      .collect();
    readers.into_iter().map(|reader| reader.join().unwrap()).collect()
  });
  assert_eq!(loads.load(Ordering::SeqCst), 1);
  assert!(patches.iter().all(|patch| Arc::ptr_eq(patch, &patches[0])));
}

#[test]
fn memory_is_read_before_the_store_and_git_by_repository_and_subject() {
  let cache = no_warm();
  let subject = DiffSubject::Commit(ID.to_string());
  let first = cache.get("/repo", &subject, || Ok((patch("one"), PatchSource::Disk))).unwrap();
  // a hit never loads
  let again = cache.get("/repo", &subject, || panic!("loaded on a memory hit")).unwrap();
  assert!(Arc::ptr_eq(&first, &again));
  // another repository, or the range of the same head, is another patch
  let other = cache.get("/other", &subject, || Ok((patch("two"), PatchSource::Git))).unwrap();
  assert_eq!(other.files[0].patch, "two");
  let range = DiffSubject::Range {
    base: ID.to_string(),
    head: ID.to_string(),
  };
  let range = cache.get("/repo", &range, || Ok((patch("three"), PatchSource::Git))).unwrap();
  assert_eq!(range.files[0].patch, "three");
}

#[test]
fn a_failed_load_is_not_cached() {
  let cache = no_warm();
  let subject = DiffSubject::Commit(ID.to_string());
  let error = cache.get("/repo", &subject, || Err(anyhow::anyhow!("git failed").context("cannot read"))).unwrap_err();
  assert_eq!(error.message, "cannot read: git failed");
  let patch = cache.get("/repo", &subject, || Ok((patch("one"), PatchSource::Git))).unwrap();
  assert_eq!(patch.files[0].patch, "one");
}

fn ids(names: &[&str]) -> Vec<String> {
  names.iter().map(ToString::to_string).collect()
}

/// One warm at a time per repository: what comes while it runs is warmed next, an id once
#[test]
fn the_warm_of_a_repository_runs_one_at_a_time() {
  let (started_sender, started) = mpsc::channel::<Vec<String>>();
  let (release, released) = mpsc::channel::<()>();
  let released = Mutex::new(released);
  let running = Arc::new(AtomicUsize::new(0));
  let most_running = Arc::new(AtomicUsize::new(0));
  let warmer = {
    let (running, most_running) = (Arc::clone(&running), Arc::clone(&most_running));
    PatchWarmer::new(move |repository_path, ids| {
      assert_eq!(repository_path, "/repo");
      let now = running.fetch_add(1, Ordering::SeqCst) + 1;
      most_running.fetch_max(now, Ordering::SeqCst);
      started_sender.send(ids.to_vec()).unwrap();
      released.lock().unwrap().recv().unwrap();
      running.fetch_sub(1, Ordering::SeqCst);
    })
  };

  warmer.enqueue("/repo", ids(&["a", "b"]));
  assert_eq!(started.recv().unwrap(), ids(&["a", "b"]));
  // while the first warm runs: queued, the ids asked for before left out
  warmer.enqueue("/repo", ids(&["b", "c"]));
  warmer.enqueue("/repo", ids(&["a", "d"]));
  warmer.enqueue("/repo", ids(&["a"]));
  release.send(()).unwrap();
  assert_eq!(started.recv().unwrap(), ids(&["c", "d"]));
  release.send(()).unwrap();

  // the warm ended; a new id starts the next one, an old one starts nothing
  warmer.enqueue("/repo", ids(&["c"]));
  warmer.enqueue("/repo", ids(&["e"]));
  assert_eq!(started.recv().unwrap(), ids(&["e"]));
  release.send(()).unwrap();
  assert!(started.recv_timeout(Duration::from_millis(100)).is_err(), "a warm of ids asked for before");
  assert_eq!(most_running.load(Ordering::SeqCst), 1);
}
