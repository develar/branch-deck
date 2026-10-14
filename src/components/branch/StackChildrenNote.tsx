/** The branches stacked on a branch that is about to lose its commits: they stay in place, blocked until it is back */
export function StackChildrenNote({ parent, children }: { parent: string, children: readonly string[] }) {
  return (
    <div className="flex flex-col gap-2 text-sm" data-testid="stack-children-note">
      <p className="font-medium text-warning">
        {children.length === 1 ? `1 branch is stacked on ${parent}` : `${children.length} branches are stacked on ${parent}`}
      </p>
      <p className="text-muted-foreground">
        {`${children.join(", ")} will be blocked until ${parent} has commits again. Stack them on another branch first to keep them building.`}
      </p>
    </div>
  )
}
