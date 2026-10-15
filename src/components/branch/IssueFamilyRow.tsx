import { CircleDotIcon } from "lucide-react"
import { LinkedText } from "@/components/commit/LinkedText"

export interface IssueFamilyRowProps {
  issueKey: string
  /** The branches nested under the row */
  branchCount: number
}

/** Heading row of the branches of an issue that has no branch of its own (all its commits were split off). */
export function IssueFamilyRow({ issueKey, branchCount }: IssueFamilyRowProps) {
  return (
    <tr data-testid="issue-family-row" data-issue-key={issueKey}>
      {/* the icon where the folder buttons are (after the lanes of a table with stacks), its name at the branch names */}
      <td colSpan={4} className="py-(--bd-cell-y) pr-6 pl-(--bd-tree-x)">
        <div className="flex items-center gap-2">
          {/* the size of the folder button of a branch row, so the family guide lines up */}
          <span className="flex size-6 shrink-0 items-center justify-center text-muted-foreground">
            <CircleDotIcon className="size-4" />
          </span>
          <LinkedText text={issueKey} className="text-sm font-medium" linkClassName="text-foreground hover:underline" />
          <span className="text-xs text-muted-foreground">{`${branchCount} branches`}</span>
        </div>
      </td>
    </tr>
  )
}
