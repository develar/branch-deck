import { useBaselineOnRemote } from "@/hooks/useBaselineOnRemote"

/**
 * The branches stacked on a branch that is about to lose its commits: they stay stacked on it and are built on the
 * baseline until it has commits again (a `(parent/child)` prefix names no branch below the parent)
 */
export function StackChildrenNote({ parent, stacked }: { parent: string; stacked: readonly string[] }) {
  const baseline = useBaselineOnRemote()
  const one = stacked.length === 1
  return (
    <div className="flex flex-col gap-2 text-sm" data-testid="stack-children-note">
      <p className="font-medium text-warning">{one ? `1 branch is stacked on ${parent}` : `${stacked.length} branches are stacked on ${parent}`}</p>
      <p className="text-muted-foreground">
        {`${stacked.join(", ")} ${one ? "stays" : "stay"} stacked on ${parent} and ${one ? "is" : "are"} built on ${baseline} until ${parent} has commits again.`}
      </p>
    </div>
  )
}
