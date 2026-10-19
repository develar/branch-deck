/*
 * The token pages on the main thread, by blob id (docs/design/diff-view.md, "Token cache on the main thread"): a blob
 * never changes, so a page serves every commit, range and diff of the window that shows the blob.
 */

export interface TokenPage {
  /** Started from a guessed state: the worker sends the exact page, or confirms this one */
  exact: boolean
  /** `lineStarts[i]` is the first run pair of line i of the page */
  lineStarts: Uint32Array
  /** Pairs of a UTF-16 offset in the line and a style */
  runs: Uint32Array
}

/** The bytes of token arrays kept; the least recently used blob goes out first */
export const MAX_TOKEN_BYTES = 32 * 1024 * 1024

type Listener = (blobId: string, page: number) => void

function pageBytes(page: TokenPage): number {
  return page.lineStarts.byteLength + page.runs.byteLength
}

export class TokenCache {
  private readonly blobs = new Map<string, Map<number, TokenPage>>()
  private readonly listeners = new Set<Listener>()
  private bytes = 0

  constructor(private readonly maxBytes = MAX_TOKEN_BYTES) {}

  get(blobId: string, page: number): TokenPage | undefined {
    return this.blobs.get(blobId)?.get(page)
  }

  /** Marks a blob as used by a paint: it goes out last */
  touch(blobId: string): void {
    const pages = this.blobs.get(blobId)
    if (pages !== undefined) {
      this.blobs.delete(blobId)
      this.blobs.set(blobId, pages)
    }
  }

  put(blobId: string, page: number, value: TokenPage): void {
    let pages = this.blobs.get(blobId)
    if (pages === undefined) {
      pages = new Map()
    }
    const old = pages.get(page)
    if (old !== undefined) {
      this.bytes -= pageBytes(old)
    }
    pages.set(page, value)
    this.bytes += pageBytes(value)
    this.blobs.delete(blobId)
    this.blobs.set(blobId, pages)
    this.evict(blobId)
    this.notify(blobId, page)
  }

  /** The provisional page is exact: the worker found the same runs */
  confirm(blobId: string, page: number): void {
    const value = this.get(blobId, page)
    if (value !== undefined && !value.exact) {
      this.blobs.get(blobId)!.set(page, { ...value, exact: true })
    }
  }

  /** Called with each page that arrives; returns the unsubscribe */
  subscribe(listener: Listener): () => void {
    this.listeners.add(listener)
    return () => this.listeners.delete(listener)
  }

  private notify(blobId: string, page: number): void {
    for (const listener of this.listeners) {
      listener(blobId, page)
    }
  }

  private evict(keep: string): void {
    for (const [blobId, pages] of this.blobs) {
      if (this.bytes <= this.maxBytes) {
        return
      }
      if (blobId !== keep) {
        for (const page of pages.values()) {
          this.bytes -= pageBytes(page)
        }
        this.blobs.delete(blobId)
      }
    }
  }
}
