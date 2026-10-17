import type { ReactNode } from "react"
import { Accordion as AccordionPrimitive } from "@base-ui/react/accordion"
import { CollapsibleFileHeader } from "@/components/conflict/CollapsibleFileHeader"
import { LazyFileDiffView } from "@/components/diff/LazyDiffViews"
import type { FileDiff } from "@/lib/bindings"
import type { ConflictDiffViewMode } from "@/stores/settings/diff-view-settings"

export interface FileDiffListProps {
  /** The paths of the files, the headers of the items */
  files: readonly string[]
  /** The diff of each file, in the order of `files`; undefined until it is loaded */
  diffs: readonly FileDiff[] | undefined
  /** What an open item shows while `diffs` is undefined: the load, or why it failed */
  pending: ReactNode
  keyPrefix: string
  diffViewMode?: ConflictDiffViewMode
  /** The file contents of each diff agree with its hunks (a missing commit), so the unchanged lines expand */
  withContents?: boolean
}

/** Accordion of file diffs (collapsed by default) */
export function FileDiffList({ files, diffs, pending, keyPrefix, diffViewMode = "unified", withContents = false }: FileDiffListProps) {
  return (
    <AccordionPrimitive.Root multiple className="flex flex-col gap-2">
      {files.map((file, index) => {
        const diff = diffs?.[index]
        return (
          <AccordionPrimitive.Item key={`${keyPrefix}-${index}`} value={`item-${index}`} className="overflow-hidden rounded-lg border border-border" data-testid="file-diff">
            <CollapsibleFileHeader fileName={file} />
            <AccordionPrimitive.Panel className="border-t border-border">
              {/* mounted only while the item is open */}
              {diff === undefined ? pending : <LazyFileDiffView diff={diff} diffViewMode={diffViewMode} withContents={withContents} />}
            </AccordionPrimitive.Panel>
          </AccordionPrimitive.Item>
        )
      })}
    </AccordionPrimitive.Root>
  )
}
