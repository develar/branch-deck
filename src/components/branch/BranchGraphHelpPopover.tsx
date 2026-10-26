import { LaneNodeDot } from "@/components/branch/StackLanes"
import { HelpPopover } from "@/components/shared/HelpPopover"
import type { LandingNodeKind } from "@/lib/bindings"
import { landingNodeLooks } from "@/presenters/landing-node"

const kinds = Object.keys(landingNodeLooks) as LandingNodeKind[]

/** The legend of the nodes beside the branch names, after the "Branch Name" head of the table and in an island header */
export function BranchGraphHelpPopover() {
  return (
    <HelpPopover title="Branch Graph" data-testid="branch-graph-help-icon">
      <p>Each dot says where the branch is on its way into master. Filled: the next step is yours.</p>
      <dl className="grid grid-cols-[auto_minmax(0,1fr)] items-center gap-x-2 gap-y-1" data-testid="branch-graph-legend">
        {kinds.map(kind => (
          <div key={kind} className="contents">
            <dt>
              <LaneNodeDot kind={kind} />
            </dt>
            <dd>{landingNodeLooks[kind].legend}</dd>
          </div>
        ))}
      </dl>
      <p>Lines join a branch to the branches built on it; hover a row to see what it is built on.</p>
    </HelpPopover>
  )
}
