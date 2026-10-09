import { useRef, useState } from "react"
import { Accordion as AccordionPrimitive } from "@base-ui/react/accordion"
import { DiffModeEnum, DiffView } from "@git-diff-view/react"
import { CircleAlertIcon } from "lucide-react"
import { Badge } from "@/components/ui/badge"
import { Spinner } from "@/components/ui/spinner"
import { CollapsibleFileHeader } from "@/components/conflict/CollapsibleFileHeader"
import { useIsDark } from "@/hooks/useIsDark"
import { commands } from "@/lib/bindings"
import type { FileDiff, UncommittedFileChange } from "@/lib/bindings"
import { subtleBadgeVariant } from "@/lib/ui-types"
import type { UIColor } from "@/lib/ui-types"
import { vcsRequestFactory } from "@/stores/repository"
import type { ConflictDiffViewMode } from "@/stores/settings/conflictViewer"

export interface OnDemandFileDiffListProps {
  files: readonly UncommittedFileChange[]
  keyPrefix: string
  diffViewMode?: ConflictDiffViewMode
}

type FileDiffState = { kind: "loading" } | { kind: "error", message: string } | { kind: "loaded", diff: FileDiff }

/** Accordion of changed files; the diff of a file is loaded from the working tree when its item is first expanded. */
export function OnDemandFileDiffList({ files, keyPrefix, diffViewMode = "unified" }: OnDemandFileDiffListProps) {
  const isDark = useIsDark()
  const [expandedItems, setExpandedItems] = useState<string[]>([])
  // keyed by file path
  const [diffStates, setDiffStates] = useState<ReadonlyMap<string, FileDiffState>>(new Map())
  // synchronous guard against a second request while the state update is pending
  const requestedRef = useRef(new Set<string>())

  const setDiffState = (filePath: string, state: FileDiffState) => {
    setDiffStates(previous => new Map(previous).set(filePath, state))
  }

  const loadFileContent = async (filePath: string) => {
    requestedRef.current.add(filePath)
    setDiffState(filePath, { kind: "loading" })
    try {
      const vcsRequest = vcsRequestFactory.createRequest()
      const result = await commands.getFileContentForDiff({ repositoryPath: vcsRequest.repositoryPath, filePath })
      if (result.status === "error") {
        throw new Error(result.error.message)
      }
      setDiffState(filePath, { kind: "loaded", diff: result.data })
    }
    catch (error) {
      console.error("Failed to load file content:", error)
      // a failed load is retried on the next expand
      requestedRef.current.delete(filePath)
      setDiffState(filePath, { kind: "error", message: `Failed to load content: ${String(error)}` })
    }
  }

  const handleValueChange = (value: unknown[]) => {
    const expanded = value as string[]
    const newlyExpanded = expanded.filter(item => !expandedItems.includes(item))
    setExpandedItems(expanded)

    for (const item of newlyExpanded) {
      const file = files[Number.parseInt(item.replace("item-", ""), 10)]
      if (file && !requestedRef.current.has(file.filePath)) {
        void loadFileContent(file.filePath)
      }
    }
  }

  return (
    <div className="bd-padding-card">
      <AccordionPrimitive.Root multiple value={expandedItems} onValueChange={handleValueChange} className="flex flex-col gap-2">
        {files.map((file, index) => {
          const state = diffStates.get(file.filePath)
          return (
            <AccordionPrimitive.Item
              key={`${keyPrefix}-${index}`}
              value={`item-${index}`}
              className="overflow-hidden rounded-lg border border-border"
            >
              <CollapsibleFileHeader
                fileName={file.filePath}
                actions={<Badge variant={subtleBadgeVariant(getStatusColor(file.status))}>{file.status}</Badge>}
              />
              <AccordionPrimitive.Panel className="border-t border-border">
                {state?.kind === "loading" && (
                  <div className="flex items-center justify-center py-8">
                    <div className="flex items-center gap-3">
                      <Spinner className="text-muted-foreground" />
                      <span className="text-sm text-muted-foreground">Loading file content...</span>
                    </div>
                  </div>
                )}
                {state?.kind === "error" && (
                  <div className="p-4">
                    <div className="flex items-center gap-2 rounded-md border border-destructive/20 bg-destructive/10 p-4">
                      <CircleAlertIcon className="size-4 text-destructive" />
                      <span className="text-sm text-destructive">{state.message}</span>
                    </div>
                  </div>
                )}
                {state?.kind === "loaded" && (
                  <DiffView
                    data={state.diff}
                    diffViewMode={diffViewMode === "unified" ? DiffModeEnum.Unified : DiffModeEnum.Split}
                    diffViewWrap
                    diffViewHighlight
                    diffViewFontSize={12}
                    diffViewTheme={isDark ? "dark" : "light"}
                  />
                )}
              </AccordionPrimitive.Panel>
            </AccordionPrimitive.Item>
          )
        })}
      </AccordionPrimitive.Root>
    </div>
  )
}

function getStatusColor(status: string): UIColor {
  switch (status) {
    case "added":
      return "success"
    case "modified":
      return "info"
    case "deleted":
      return "error"
    case "renamed":
    case "copied":
      return "neutral"
    default:
      return "info"
  }
}
