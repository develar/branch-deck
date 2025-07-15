import type { Toast } from "#ui/composables/useToast"

export function reportError(title: string, error: unknown, toast: { add: (toast: Partial<Toast>) => Toast }) {
  toast.add({
    color: "error",
    title: title,
    description: error instanceof Error ? error.message : String(error),
    duration: 0,
    progress: false,
  })
}
