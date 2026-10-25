/**
 * The toast manager of `@/components/ui/toast` as Base UI keeps its toasts, for the unit tests of what toasts: an add
 * with the id of an open toast takes the new options in place, an update merges, a close calls `onClose`. Load it in a
 * `vi.mock` factory: `vi.mock("@/components/ui/toast", async () => (await import("@/test-support/toast-mock")).toastModule)`.
 */

export interface FakeToast {
  id: string
  type?: string
  title?: string
  description?: unknown
  timeout?: number
  actionProps?: { children?: string; onClick: () => void; disabled?: boolean }
  data?: { secondaryAction?: { label: string; onClick: () => void; disabled?: boolean }; running?: boolean }
  onClose?: () => void
  /** Still shown: not closed, by the test, the code or its timeout */
  open: boolean
}

/** Every toast shown, oldest first, each as it is now (an update in place changes it, a close marks it) */
export const toasts: FakeToast[] = []

let sequence = 0

function find(id: string): FakeToast | undefined {
  return toasts.find(toast => toast.id === id && toast.open)
}

export const toastModule = {
  toast: {
    add: (options: Omit<FakeToast, "id" | "open"> & { id?: string }): string => {
      const id = options.id ?? `toast-${++sequence}`
      const open = find(id)
      if (open === undefined) {
        toasts.push({ ...options, id, open: true })
      } else {
        Object.assign(open, options)
      }
      return id
    },
    update: (id: string, updates: Partial<FakeToast>): void => {
      const open = find(id)
      if (open !== undefined) {
        Object.assign(open, updates)
      }
    },
    close: (id?: string): void => {
      for (const toast of toasts.filter(it => it.open && (id === undefined || it.id === id))) {
        toast.open = false
        toast.onClose?.()
      }
    },
  },
}

/** The toasts shown now, oldest first */
export function openToasts(): FakeToast[] {
  return toasts.filter(toast => toast.open)
}

/** What a toast says and offers: its title, its description, its action, its second action (a running one in […]) */
export function describeToast(toast: FakeToast): (string | undefined)[] {
  const label = (text: string | undefined, running: boolean | undefined) => (text === undefined || running !== true ? text : `[${text}]`)
  return [
    toast.title,
    typeof toast.description === "string" ? toast.description : undefined,
    label(toast.actionProps?.children, toast.data?.running),
    label(toast.data?.secondaryAction?.label, toast.data?.secondaryAction?.disabled),
  ]
}

/** Forgets the toasts (a `beforeEach` of the tests, after `resetEventToasts`) */
export function resetToasts(): void {
  toasts.length = 0
  sequence = 0
}
