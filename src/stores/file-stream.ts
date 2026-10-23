import { Channel } from "@tauri-apps/api/core"
import { createStore } from "zustand/vanilla"
import type { StoreApi } from "zustand/vanilla"
import { invokeCommand } from "@/lib/command"
import type { CommandResult } from "@/lib/command"
import { logError } from "@/lib/errors"

/*
 * A store of the files that a command streams through a channel (`get_file_diffs`, `get_file_merges`): the headers
 * first, then the content of each file as it is known, then the scopes of a text the backend parsed late.
 */

export type FileStreamEntry<F> =
  | { kind: "loading" }
  | { kind: "error"; message: string }
  /**
   * The files whose content arrived, in the order of git; `complete` once the stream ended. A file of the list whose
   * content has not arrived yet is pending while the entry is not complete
   */
  | { kind: "loaded"; files: readonly F[]; complete: boolean }

export interface FileStreamState<F> {
  /** By key, the least recently loaded first */
  byKey: ReadonlyMap<string, FileStreamEntry<F>>
}

/** The events of a file stream; `index` points into the headers of `files` */
export type FileStreamEvent<H, C> = { kind: "files"; files: H[] } | { kind: "content"; index: number; content: C } | { kind: "scopes"; index: number }

/** The contents that arrive within this time go into the store in one update */
const BATCH_MS = 16

export interface FileStream<F extends { path: string }, E> {
  store: StoreApi<FileStreamState<F>>
  /**
   * Reads the files of a key through `open` unless they are loaded or being read; a failed read is tried again. With
   * `refresh` the loaded files are read again, and they stay shown until the new read ends; a refresh asked while the
   * key is read reads once more after that read (the last `open` asked), so a change during a read is never lost.
   */
  load: (key: string, open: (channel: Channel<E>) => Promise<CommandResult<null>>, refresh?: boolean) => void
}

/**
 * A file stream that keeps the last `maxEntries` complete reads. `scopesOf` gives the file with the scopes of a
 * `scopes` event, null when the file has no text.
 */
export function createFileStream<H extends { path: string }, C, F extends H & { content: C }, E extends FileStreamEvent<H, C>>(
  maxEntries: number,
  failure: string,
  scopesOf: (file: F, event: Extract<E, { kind: "scopes" }>) => F | null,
): FileStream<F, E> {
  const store = createStore<FileStreamState<F>>(() => ({ byKey: new Map() }))
  // the keys whose stream runs: a key is read once at a time
  const reading = new Set<string>()
  // the refreshes asked while their key is read: the read after it, by the last `open` asked
  const trailing = new Map<string, (channel: Channel<E>) => Promise<CommandResult<null>>>()

  const setEntry = (key: string, entry: FileStreamEntry<F>, evict: boolean) =>
    store.setState(state => {
      const next = new Map(state.byKey)
      next.delete(key)
      next.set(key, entry)
      if (evict) {
        const complete = [...next].filter(([, value]) => value.kind === "loaded" && value.complete)
        for (const [old] of complete.slice(0, Math.max(complete.length - maxEntries, 0))) {
          next.delete(old)
        }
      }
      return { byKey: next }
    })

  const load = (key: string, open: (channel: Channel<E>) => Promise<CommandResult<null>>, refresh = false) => {
    const entry = store.getState().byKey.get(key)
    if (reading.has(key)) {
      if (refresh) {
        trailing.set(key, open)
      }
      return
    }
    if (entry?.kind === "loaded" && !refresh) {
      setEntry(key, entry, false)
      return
    }
    reading.add(key)
    // a refresh shows the loaded files until the new read ends
    const streams = entry?.kind !== "loaded"
    if (streams) {
      setEntry(key, { kind: "loading" }, false)
    }

    let headers: readonly H[] | null = null
    const files: (F | undefined)[] = []
    let timer: ReturnType<typeof setTimeout> | undefined
    const flush = (complete: boolean) => {
      clearTimeout(timer)
      timer = undefined
      if (headers === null || (!streams && !complete)) {
        return
      }
      setEntry(key, { kind: "loaded", files: files.filter((file): file is F => file !== undefined), complete }, complete)
    }
    const schedule = () => {
      timer ??= setTimeout(() => flush(false), BATCH_MS)
    }
    const channel = new Channel<E>()
    channel.onmessage = event => {
      switch (event.kind) {
        case "files":
          headers = event.files
          flush(false)
          break
        case "content": {
          const header = headers?.[event.index]
          if (header !== undefined) {
            files[event.index] = { ...header, content: event.content } as F
            schedule()
          }
          break
        }
        case "scopes": {
          const file = files[event.index]
          const labelled = file === undefined ? null : scopesOf(file, event as Extract<E, { kind: "scopes" }>)
          if (labelled !== null) {
            files[event.index] = labelled
            schedule()
          }
          break
        }
      }
    }
    void invokeCommand(() => open(channel)).then(result => {
      reading.delete(key)
      if (result.status === "error") {
        logError(failure, result.error)
        if (headers === null) {
          // a refresh keeps what it shows; a first read says why it has nothing
          if (streams) {
            setEntry(key, { kind: "error", message: result.error.message }, true)
          }
          readTrailing(key)
          return
        }
      }
      flush(true)
      readTrailing(key)
    })
  }

  /** The refresh asked while the read of `key` ran, now that it ended */
  function readTrailing(key: string): void {
    const open = trailing.get(key)
    if (open !== undefined) {
      trailing.delete(key)
      load(key, open, true)
    }
  }

  return { store, load }
}

/**
 * The file of an entry: undefined when the stream has none for the path, "pending" while its key is read and its
 * content has not arrived
 */
export function streamedFileOf<F extends { path: string }>(entry: FileStreamEntry<F> | undefined, path: string): F | "pending" | undefined {
  if (entry?.kind === "error") {
    return undefined
  }
  if (entry?.kind !== "loaded") {
    return "pending"
  }
  return entry.files.find(file => file.path === path) ?? (entry.complete ? undefined : "pending")
}
