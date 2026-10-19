import { ConflictingFilesSection } from "@/components/conflict/ConflictingFilesSection"
import { SectionPanel } from "@/components/shared/NestedPanel"
import type { ConflictFile, WordResolutionSummary } from "@/lib/bindings"
import type { MergeSource } from "@/lib/diff/file-merge-model"
import { useFileMerges } from "@/stores/file-merges"
import { useDiffWhitespacePolicy } from "@/stores/settings/diff-view-settings"

export interface ResolvedMergeViewerProps {
  /** The original commit */
  commit: string
  /** The copy the sync wrote */
  copy: string
  resolved: WordResolutionSummary
  /** The subject of the commit, for the right column */
  subject: string
}

/**
 * What the sync did with a conflict of git that its words resolve (`branchdeck.resolveByWords`): the merge of each file
 * in three columns, the middle the result the sync committed (spec/commit/commit-row.spec.md, "Resolved by words").
 * The files come with the sync that resolved them; a copy reused by its note has the counts only, and its files come
 * with the merges.
 */
export function ResolvedMergeViewer({ commit, copy, resolved, subject }: ResolvedMergeViewerProps) {
  const source: MergeSource = { kind: "cherryPick", parent: resolved.parent, target: resolved.target, commit, resolved: copy }
  const entry = useFileMerges(source, useDiffWhitespacePolicy())
  const files: ConflictFile[] =
    resolved.files.length > 0
      ? resolved.files.map(file => ({ file: file.path, conflictCount: file.conflicts }))
      : entry?.kind === "loaded"
        ? entry.files.map(file => ({ file: file.path, conflictCount: 0 }))
        : []
  return (
    <SectionPanel
      island={false}
      data-testid="resolved-by-words"
      header={{ title: "Resolved by Words", count: resolved.fileCount, itemSingular: "file", itemPlural: "files", badgeColor: "success" }}
    >
      <div className="flex flex-col gap-3 bd-padding-card">
        <p className="text-xs text-muted-foreground">
          Git stops at {resolved.conflicts === 1 ? "1 conflict" : `${resolved.conflicts} conflicts`} here, and the words of each resolve it: the sync copied the commit with the
          result, as IntelliJ resolves simple conflicts. Settings → Repository turns this off.
        </p>
        <ConflictingFilesSection files={files} source={source} labels={{ left: "the branch below", right: `“${subject}”` }} />
      </div>
    </SectionPanel>
  )
}
