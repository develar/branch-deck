import { useRef, useState } from "react"
import type { ReactNode } from "react"
import { Accordion as AccordionPrimitive } from "@base-ui/react/accordion"
import { CollapsibleFileHeader } from "@/components/conflict/CollapsibleFileHeader"
import { DiffLoadState } from "@/components/conflict/DiffLoadState"
import { LazyFileDiffView } from "@/components/diff/LazyDiffViews"
import type { FileDiff, FileStatus } from "@/lib/bindings"
import type { CommandResult } from "@/lib/command"
import { invokeCommand } from "@/lib/command"
import type { ConflictDiffViewMode } from "@/stores/settings/diff-view-settings"

/** A file of the list */
export interface OnDemandFile {
  /** The path after the change: the key of its diff, and what the copy button copies */
  path: string
  /** The status of the file: its letter and the colour of its name */
  status?: FileStatus
  /** Before the chevron: the line counts; must not be interactive */
  actions?: ReactNode
  /** Shown instead of a diff, which is not loaded (a binary file) */
  notice?: string
}

export interface OnDemandFileDiffListProps {
  files: readonly OnDemandFile[]
  keyPrefix: string
  diffViewMode?: ConflictDiffViewMode
  /** Reads the diff of a file, when its item is first expanded */
  loadDiff: (file: OnDemandFile) => Promise<CommandResult<FileDiff>>
}

type FileDiffState = { kind: "loading" } | { kind: "error"; message: string } | { kind: "loaded"; diff: FileDiff }

/**
 * Accordion of changed files (the uncommitted changes, the files of a commit); the diff of a file is loaded when its
 * item is first expanded.
 */
export function OnDemandFileDiffList({ files, keyPrefix, diffViewMode = "unified", loadDiff }: OnDemandFileDiffListProps) {
  const [expandedItems, setExpandedItems] = useState<string[]>([])
  // keyed by file path
  const [diffStates, setDiffStates] = useState<ReadonlyMap<string, FileDiffState>>(new Map())
  // synchronous guard against a second request while the state update is pending
  const requestedRef = useRef(new Set<string>())

  const setDiffState = (filePath: string, state: FileDiffState) => {
    setDiffStates(previous => new Map(previous).set(filePath, state))
  }

  const loadFileContent = async (file: OnDemandFile) => {
    requestedRef.current.add(file.path)
    setDiffState(file.path, { kind: "loading" })
    const result = await invokeCommand(() => loadDiff(file))
    if (result.status === "ok") {
      setDiffState(file.path, { kind: "loaded", diff: result.data })
      return
    }
    console.error("Failed to load file content:", result.error)
    // a failed load is retried on the next expand
    requestedRef.current.delete(file.path)
    setDiffState(file.path, { kind: "error", message: `Cannot load the diff: ${result.error.message}` })
  }

  const handleValueChange = (value: unknown[]) => {
    const expanded = value as string[]
    const newlyExpanded = expanded.filter(item => !expandedItems.includes(item))
    setExpandedItems(expanded)

    for (const item of newlyExpanded) {
      const file = files[Number.parseInt(item.replace("item-", ""), 10)]
      if (file && file.notice === undefined && !requestedRef.current.has(file.path)) {
        void loadFileContent(file)
      }
    }
  }

  return (
    <AccordionPrimitive.Root multiple value={expandedItems} onValueChange={handleValueChange} className="flex flex-col gap-2">
      {files.map((file, index) => {
        const state = diffStates.get(file.path)
        return (
          <AccordionPrimitive.Item key={`${keyPrefix}-${index}`} value={`item-${index}`} className="overflow-hidden rounded-lg border border-border">
            <CollapsibleFileHeader path={file.path} status={file.status} actions={file.actions} />
            <AccordionPrimitive.Panel className="border-t border-border">
              {file.notice !== undefined && <p className="p-4 text-sm text-muted-foreground">{file.notice}</p>}
              {(state?.kind === "loading" || state?.kind === "error") && <DiffLoadState loading="Loading file content…" error={state.kind === "error" ? state.message : null} />}
              {state?.kind === "loaded" &&
                (state.diff.hunks.length === 0 ? (
                  <p className="p-4 text-sm text-muted-foreground">No changed lines</p>
                ) : (
                  <LazyFileDiffView diff={state.diff} diffViewMode={diffViewMode} />
                ))}
            </AccordionPrimitive.Panel>
          </AccordionPrimitive.Item>
        )
      })}
    </AccordionPrimitive.Root>
  )
}
