import { useEffect } from "react"
import { useStore } from "zustand"
import { createStore } from "zustand/vanilla"
import type { ConflictContents, MergeConflictInfo } from "@/lib/bindings"
import { commands } from "@/lib/bindings"
import { invokeCommand } from "@/lib/command"
import { notifyError } from "@/lib/errors"
import { getDiffRepositoryPath } from "@/stores/repository"

/** The contents of a conflict view: loading, why they did not load, or loaded */
export type ConflictContentsEntry = { kind: "loading" } | { kind: "error"; message: string } | { kind: "loaded"; contents: ConflictContents }

export interface ConflictContentsState {
  /** By repository and the ids of the cherry-pick (`contentsId`), only while a view shows the conflict */
  byConflict: ReadonlyMap<string, ConflictContentsEntry>
}

/**
 * The contents of the conflicts the views of this window show: the merged files, the change of the commit, the diffs
 * of the missing commits (`get_conflict_contents`). A sync reports a conflict without them (`MergeConflictInfo`): in big
 * files they are megabytes, so they are read when a view shows the conflict and dropped when the last one closes.
 */
export const conflictContentsStore = createStore<ConflictContentsState>(() => ({ byConflict: new Map() }))

/** The views that show each conflict, by `contentsId`: its contents are kept while there is one */
const viewers = new Map<string, number>()
/** The conflicts whose contents are being read */
const inFlight = new Set<string>()

interface ConflictIds {
  repositoryPath: string
  commit: string
  parent: string
  target: string
}

function contentsId({ repositoryPath, commit, parent, target }: ConflictIds): string {
  return `${repositoryPath}\0${commit}\0${parent}\0${target}`
}

/**
 * The contents of `conflict` for a view that shows it: read when the first view of it mounts (`undefined` until the
 * read starts), kept while one is mounted. A failure is a sticky toast and the `error` entry, read again when the
 * conflict is shown anew.
 */
export function useConflictContents(conflict: MergeConflictInfo): ConflictContentsEntry | undefined {
  const repositoryPath = getDiffRepositoryPath() ?? ""
  const commit = conflict.commit.hash
  const parent = conflict.originalParent.hash
  const target = conflict.targetBranch.hash
  useEffect(() => showConflict({ repositoryPath, commit, parent, target }), [repositoryPath, commit, parent, target])
  return useStore(conflictContentsStore, state => state.byConflict.get(contentsId({ repositoryPath, commit, parent, target })))
}

/** A view of the conflict mounts: its contents are read unless they are loaded or being read. Returns its unmount */
function showConflict(ids: ConflictIds): () => void {
  const id = contentsId(ids)
  viewers.set(id, (viewers.get(id) ?? 0) + 1)
  if (!inFlight.has(id) && !conflictContentsStore.getState().byConflict.has(id)) {
    void load(ids, id)
  }
  return () => {
    const count = (viewers.get(id) ?? 1) - 1
    if (count > 0) {
      viewers.set(id, count)
      return
    }
    viewers.delete(id)
    conflictContentsStore.setState(state => ({ byConflict: without(state.byConflict, id) }))
  }
}

async function load(ids: ConflictIds, id: string): Promise<void> {
  inFlight.add(id)
  setEntry(id, { kind: "loading" })
  const { repositoryPath, commit, parent, target } = ids
  const result = await invokeCommand(() => commands.getConflictContents({ repositoryPath, commit, parent, target }))
  inFlight.delete(id)
  // no view shows the conflict anymore
  if (!viewers.has(id)) {
    return
  }
  if (result.status === "ok") {
    setEntry(id, { kind: "loaded", contents: result.data })
    return
  }
  setEntry(id, { kind: "error", message: result.error.message })
  notifyError("Cannot load the conflict", result.error)
}

function setEntry(id: string, entry: ConflictContentsEntry): void {
  conflictContentsStore.setState(state => ({ byConflict: new Map(state.byConflict).set(id, entry) }))
}

function without(byConflict: ReadonlyMap<string, ConflictContentsEntry>, id: string): ReadonlyMap<string, ConflictContentsEntry> {
  if (!byConflict.has(id)) {
    return byConflict
  }
  const next = new Map(byConflict)
  next.delete(id)
  return next
}
