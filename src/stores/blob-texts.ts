import { useStore } from "zustand"
import { createStore } from "zustand/vanilla"
import { commands } from "@/lib/bindings"
import type { BlobRequest, BlobText } from "@/lib/bindings"
import { invokeCommand } from "@/lib/command"
import { logError } from "@/lib/errors"
import { getDiffRepositoryPath } from "@/stores/repository"

export type BlobTextEntry = { kind: "loading" } | { kind: "loaded"; blob: BlobText } | { kind: "error"; message: string }

export interface BlobTextsState {
  /**
   * By git blob id, the least recently used first. A blob never changes, so an entry is never stale, and the same id
   * serves every commit, range and window that shows the text
   */
  byId: ReadonlyMap<string, BlobTextEntry>
}

/** The text kept, in UTF-16 code units: the files of a few large reviews */
export const MAX_BLOB_TEXT_LENGTH = 64 * 1024 * 1024
/** The ids of one `get_blob_texts` */
export const BLOB_TEXT_BATCH = 64

export const blobTextsStore = createStore<BlobTextsState>(() => ({ byId: new Map() }))

export function useBlobText(id: string | null): BlobTextEntry | undefined {
  return useStore(blobTextsStore, state => (id === null ? undefined : state.byId.get(id)))
}

/** The ids to read, with the path of a file of the working tree */
let pending = new Map<string, string | null>()
let flushQueued = false

/**
 * Reads the texts of these blobs unless they are loaded or being read; a failed read is tried again. A blob of the
 * working tree names its path, a blob of git none (a plain id). The ids of one task go out together, in batches of
 * `BLOB_TEXT_BATCH`, one batch in flight at a time (a Find over a review of a thousand files starts no 16 `cat-file`
 * processes at once). A loaded text moves to the end, so the shown texts are evicted last.
 */
export function requestBlobTexts(requests: readonly (string | BlobRequest)[]): void {
  const blobs = requests.map(request => (typeof request === "string" ? { id: request, path: null } : request))
  const ids = blobs.map(blob => blob.id)
  const byId = blobTextsStore.getState().byId
  const touched = ids.filter(id => byId.get(id)?.kind === "loaded")
  if (touched.length > 0) {
    blobTextsStore.setState(state => ({
      byId: withEntries(
        state.byId,
        touched.map(id => [id, state.byId.get(id)!]),
      ),
    }))
  }
  for (const { id, path } of blobs) {
    const entry = byId.get(id)
    if (entry === undefined || entry.kind === "error") {
      pending.set(id, path)
    }
  }
  if (pending.size > 0 && !flushQueued) {
    flushQueued = true
    queueMicrotask(flush)
  }
}

/**
 * Keeps a text the app derives from blobs (the result of a merge) under its own id, never a git blob id: the views read
 * it as they read a blob, and it is evicted as one
 */
export function putDerivedText(id: string, text: string): void {
  blobTextsStore.setState(state => ({ byId: evict(withEntries(state.byId, [[id, { kind: "loaded", blob: { kind: "text", id, text } }]]), new Set([id])) }))
}

/** The batches to read, each with its repository: one is read at a time, so a Find over a large review is one process */
const batches: { repositoryPath: string; blobs: BlobRequest[] }[] = []
let reading = false

function flush(): void {
  flushQueued = false
  const blobs = [...pending].map(([id, path]): BlobRequest => ({ id, path }))
  const ids = blobs.map(blob => blob.id)
  pending = new Map()
  const repositoryPath = getDiffRepositoryPath()
  if (!repositoryPath || ids.length === 0) {
    return
  }
  blobTextsStore.setState(state => ({
    byId: withEntries(
      state.byId,
      ids.map(id => [id, { kind: "loading" }]),
    ),
  }))
  for (let start = 0; start < blobs.length; start += BLOB_TEXT_BATCH) {
    batches.push({ repositoryPath, blobs: blobs.slice(start, start + BLOB_TEXT_BATCH) })
  }
  readNext()
}

/** Reads the next batch unless one is being read: at most one `get_blob_texts` in flight, the batches in order */
function readNext(): void {
  if (reading) {
    return
  }
  const next = batches.shift()
  if (next === undefined) {
    return
  }
  reading = true
  const batch = next.blobs.map(blob => blob.id)
  void invokeCommand(() => commands.getBlobTexts(next))
    .then(result => {
      if (result.status === "error") {
        logError("Cannot read the texts of a diff", result.error)
      }
      const byBlob = new Map(result.status === "ok" ? result.data.map(blob => [blob.id, blob]) : [])
      const entries = batch.map((id): [string, BlobTextEntry] => {
        const blob = byBlob.get(id)
        return [id, blob !== undefined ? { kind: "loaded", blob } : { kind: "error", message: result.status === "error" ? result.error.message : "the blob is not in the answer" }]
      })
      blobTextsStore.setState(state => ({ byId: evict(withEntries(state.byId, entries), new Set(batch)) }))
    })
    .finally(() => {
      reading = false
      readNext()
    })
}

function withEntries(byId: ReadonlyMap<string, BlobTextEntry>, entries: readonly [string, BlobTextEntry][]): Map<string, BlobTextEntry> {
  const next = new Map(byId)
  for (const [id, entry] of entries) {
    // re-inserted, so the entry is the most recent
    next.delete(id)
    next.set(id, entry)
  }
  return next
}

function textLength(entry: BlobTextEntry): number {
  return entry.kind === "loaded" && entry.blob.kind === "text" ? entry.blob.text.length : 0
}

/** Drops the oldest loaded texts over the length; the texts just read stay whatever their size */
function evict(byId: Map<string, BlobTextEntry>, keep: ReadonlySet<string>): Map<string, BlobTextEntry> {
  let length = 0
  for (const entry of byId.values()) {
    length += textLength(entry)
  }
  for (const [id, entry] of byId) {
    if (length <= MAX_BLOB_TEXT_LENGTH) {
      break
    }
    if (entry.kind === "loaded" && !keep.has(id)) {
      byId.delete(id)
      length -= textLength(entry)
    }
  }
  return byId
}
