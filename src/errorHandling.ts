// @ts-expect-error Idea cannot resolve
import { Toast } from "@nuxt/ui/composables/useToast"

export function reportError(title: string, error: unknown, toast: { add: (toast: Partial<Toast>) => Toast }) {
  toast.add({
    color: "error",
    title: title,
    description: error,
    delay: 10_000,
    progress: false,
  })
}
