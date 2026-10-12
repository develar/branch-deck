import { useMemo } from "react"
import { Accordion as AccordionPrimitive } from "@base-ui/react/accordion"
import { DiffModeEnum, DiffView } from "@git-diff-view/react"
import type { DiffViewProps } from "@git-diff-view/react"
import { CollapsibleFileHeader } from "@/components/conflict/CollapsibleFileHeader"
import { ConflictMarkerExtension } from "@/components/conflict/ConflictMarkerExtension"
import type { ConflictLineData, ConflictMarkerInfo } from "@/components/conflict/ConflictMarkerExtension"
import { useIsDark } from "@/hooks/useIsDark"
import type { ConflictCommit, FileDiff } from "@/lib/bindings"
import type { ConflictDiffViewMode } from "@/stores/settings/conflictViewer"

export interface FileDiffListProps {
  fileDiffs: readonly FileDiff[]
  keyPrefix: string
  conflictMarkerCommits?: Record<string, ConflictCommit>
  diffViewMode?: ConflictDiffViewMode
}

/** Accordion of file diffs (collapsed by default); conflict markers in added lines get an annotation. */
export function FileDiffList({ fileDiffs, keyPrefix, conflictMarkerCommits, diffViewMode = "unified" }: FileDiffListProps) {
  return (
    <AccordionPrimitive.Root multiple className="flex flex-col gap-2">
      {fileDiffs.map((diff, index) => (
        <AccordionPrimitive.Item
          key={`${keyPrefix}-${index}`}
          value={`item-${index}`}
          className="overflow-hidden rounded-lg border border-border"
        >
          <CollapsibleFileHeader fileName={diff.newFile.fileName} />
          <AccordionPrimitive.Panel className="border-t border-border">
            <ConflictFileDiffView diff={diff} conflictMarkerCommits={conflictMarkerCommits} diffViewMode={diffViewMode} />
          </AccordionPrimitive.Panel>
        </AccordionPrimitive.Item>
      ))}
    </AccordionPrimitive.Root>
  )
}

interface ConflictFileDiffViewProps {
  diff: FileDiff
  conflictMarkerCommits: Record<string, ConflictCommit> | undefined
  diffViewMode: ConflictDiffViewMode
}

/** Mounted only while the accordion item is open; `diff` must keep its identity, `DiffView` re-parses on a new `data` object. */
function ConflictFileDiffView({ diff, conflictMarkerCommits, diffViewMode }: ConflictFileDiffViewProps) {
  const isDark = useIsDark()
  const extendData = useMemo(() => getConflictExtendData(diff, conflictMarkerCommits), [diff, conflictMarkerCommits])

  return (
    <DiffView<ConflictLineData>
      data={diff}
      diffViewMode={diffViewMode === "unified" ? DiffModeEnum.Unified : DiffModeEnum.Split}
      diffViewWrap
      diffViewHighlight
      diffViewFontSize={12}
      diffViewTheme={isDark ? "dark" : "light"}
      extendData={extendData}
      renderExtendLine={renderConflictMarker}
    />
  )
}

// module-level, so the `DiffView` store keeps the same render function across renders
const renderConflictMarker: NonNullable<DiffViewProps<ConflictLineData>["renderExtendLine"]> = ({ data }) => <ConflictMarkerExtension data={data} />

/** Extracts the marker kind and the referenced branch/commit from a line with a conflict marker */
function extractConflictInfo(line: string): ConflictMarkerInfo | null {
  if (line.includes("<<<<<<< ")) {
    const branch = line.replace("<<<<<<< ", "").trim()
    return { type: "conflict-start", branch, tooltip: `Current branch: ${branch}` }
  }
  if (line.includes("||||||| ")) {
    const base = line.replace("||||||| ", "").trim()
    return { type: "conflict-base", base, tooltip: `Common ancestor: ${base}` }
  }
  if (line.includes("=======")) {
    return { type: "conflict-separator", tooltip: "Conflict separator" }
  }
  if (line.includes(">>>>>>> ")) {
    const commit = line.replace(">>>>>>> ", "").trim()
    return { type: "conflict-end", commit, tooltip: `Incoming commit: ${commit}` }
  }
  return null
}

type ExtendData = NonNullable<DiffViewProps<ConflictLineData>["extendData"]>

/** `extendData` keyed by new-file line number for every added line holding a conflict marker */
function getConflictExtendData(diff: FileDiff, conflictMarkerCommits: Record<string, ConflictCommit> | undefined): ExtendData {
  const newFile: Record<string, { data: ConflictLineData }> = {}

  for (const hunk of diff.hunks) {
    let newFileLineNumber = 0
    let inHunk = false

    for (const line of hunk.split("\n")) {
      // the hunk header gives the starting line number
      if (line.startsWith("@@")) {
        const match = /@@ -\d+(?:,\d+)? \+(\d+)(?:,(\d+))? @@/.exec(line)
        if (match?.[1]) {
          newFileLineNumber = Number.parseInt(match[1], 10)
          inHunk = true
        }
        continue
      }

      if (!inHunk) {
        continue
      }

      if (line.startsWith("+") && !line.startsWith("+++")) {
        const conflictInfo = extractConflictInfo(line.substring(1))
        if (conflictInfo) {
          newFile[newFileLineNumber] = { data: { tooltip: conflictInfo.tooltip, conflictInfo, conflictMarkerCommits } }
        }
        newFileLineNumber++
      }
      else if (line.startsWith("-") && !line.startsWith("---")) {
        // a deleted line doesn't advance the new file line number
      }
      else if (!line.startsWith("\\")) {
        // context line
        newFileLineNumber++
      }
    }
  }

  return { oldFile: {}, newFile }
}
