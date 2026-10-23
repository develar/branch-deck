import { MenuItemButton } from "@/components/shared/MenuItemButton"
import { stopWaitingItem, useSurfaceOperation } from "@/stores/operations"

/**
 * Stop Waiting of the operation `surface` started, while its command waits for another holder of the repository lock;
 * nothing otherwise. For a surface whose own label already says whom it waits for (an inline form, a button)
 */
export function StopWaitingButton({ surface }: { surface: string }) {
  const item = stopWaitingItem(useSurfaceOperation(surface))
  return item === null ? null : <MenuItemButton item={item} variant="outline" data-testid="stop-waiting" />
}

/**
 * Whom the operation `surface` started waits for while another holder has the repository lock ("Waiting for `bd pick`
 * (pid 4242, for 12 s)"), with its Stop Waiting; nothing otherwise. For a surface that shows its own busy state (the
 * squash dialog, the commit message editor, the move form)
 */
export function LockWaitNotice({ surface }: { surface: string }) {
  const operation = useSurfaceOperation(surface)
  const item = stopWaitingItem(operation)
  if (item === null) {
    return null
  }
  return (
    <span className="flex min-w-0 items-center gap-2 text-xs text-muted-foreground" data-testid="lock-wait-notice">
      <span className="min-w-0 truncate">{operation?.lockWait}</span>
      <MenuItemButton item={item} variant="outline" data-testid="stop-waiting" />
    </span>
  )
}
