import { useDockStatus } from "@/hooks/useDockStatus"

/** The Dock progress and badge of the main window (`useDockStatus`); renders nothing, so its tick renders only itself */
export function DockStatus() {
  useDockStatus()
  return null
}
