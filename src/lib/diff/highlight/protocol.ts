/*
 * The messages between a view and the highlight worker (docs/design/diff-view.md, "Protocol"). A worker keeps the texts
 * and the grammar states of the blobs pinned to it, and answers pages of compact runs as transferables.
 */

/** The lines of a page of tokens */
export const PAGE_LINES = 256

/** In view first, then one viewport around it, then the rest */
export type Priority = 0 | 1 | 2

/** The pages of a blob a view needs */
export interface PageWant {
  blobId: string
  pages: readonly number[]
  priority: Priority
}

export type HighlightRequest =
  /** Once per blob and worker; again after a `missing` answer */
  | { type: "text"; blobId: string; language: string; text: string }
  /** The pages a view needs now: they replace what the view asked for before */
  | { type: "tokens"; view: string; wants: readonly PageWant[] }

export type HighlightAnswer =
  /**
   * The runs of a page: `runs` holds pairs of a UTF-16 offset in its line and a style (the role in the low 4 bits, the
   * font style bits above), a run up to the next one or the end of the line; `lineStarts[i]` is the first pair of line
   * i of the page. A provisional page started from a guessed state; an exact one follows.
   */
  | { type: "page"; blobId: string; page: number; exact: boolean; lineStarts: Uint32Array; runs: Uint32Array }
  /** The exact runs of a provisional page are the same: the page is exact now */
  | { type: "exact"; blobId: string; page: number }
  /** The worker has no text of the blob (never sent, or dropped from its LRU): send it again */
  | { type: "missing"; blobId: string }
  /** The blob has no grammar, or its grammar failed: it stays plain text */
  | { type: "failed"; blobId: string; reason: string }

/** The bits of the role in a style: 63 roles, room for the roles of the tree-sitter highlighting of the backend */
export const ROLE_MASK = 0x3f
/** The font style bits of a style, shifted: italic 1, bold 2, underline 4, strikethrough 8 */
export const FONT_STYLE_SHIFT = 6
