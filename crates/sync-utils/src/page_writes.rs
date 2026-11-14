//! The order of the writes of the pages of the windows (openspec/specs/repository/settings/spec.md, "The writes of a page
//! apply newest first"): a page names itself at its load (`page`, an id it draws, new for every load in every host) and
//! numbers its writes (`seq`, from 1 at its load, `nextWriteOrder` of `src/lib/field-writes.ts`); the one writer of what
//! they write (the settings, the git config of a repository) applies a write of a key only when it is newer than the
//! last write of that key of the same page it decided, and an older one is superseded and writes nothing. So the close
//! of Settings may send an edit while the write before it still runs, and whichever comes first, the newer value stays.
//! A page loaded again names itself anew, so its first write is never superseded by the numbers of the page before;
//! the writes of two pages of a window apply in the order they come. The page names itself rather than the writer
//! naming its pages (`Call::page`), which a host learns only when it hears of each load, a reload and a window opened
//! again under the same label.

use std::collections::{HashMap, VecDeque};

/// How many pages of a window the writer remembers the writes of: the newest, and those whose late writes may still come
const PAGES_KEPT: usize = 4;

/// The last write of each key the writer decided, of the latest pages of each window
#[derive(Default)]
pub struct PageWrites {
  windows: HashMap<String, VecDeque<PageSeqs>>,
}

/// A page of a window, by the id it drew at its load, and the last number of each key it wrote
struct PageSeqs {
  page: String,
  seqs: HashMap<String, u32>,
}

/// A write of a page: the window that sent it, the page it named, and its number among the writes of that page
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PageWrite {
  pub window: String,
  pub page: String,
  pub seq: u32,
}

impl PageWrites {
  /// Whether the writer applies `write` of `key`: not when its page wrote the key with this or a later number already. A
  /// write that applies is the last of its key of its page from now, whether or not it then succeeds: a write that
  /// failed supersedes the older ones all the same, as the user's last value was its own
  pub fn applies(&mut self, write: &PageWrite, key: &str) -> bool {
    let pages = self.windows.entry(write.window.clone()).or_default();
    let index = pages.iter().position(|kept| kept.page == write.page).unwrap_or_else(|| {
      pages.push_front(PageSeqs {
        page: write.page.clone(),
        seqs: HashMap::new(),
      });
      pages.truncate(PAGES_KEPT);
      0
    });
    let seqs = &mut pages[index].seqs;
    match seqs.get(key) {
      Some(last) if *last >= write.seq => false,
      _ => {
        seqs.insert(key.to_owned(), write.seq);
        true
      }
    }
  }
}
