/** A handler of a backend event, as `listen` of `@tauri-apps/api/event` gets it */
export type EventHandler = (event: { payload: unknown }) => void

/**
 * The modules a test of the settings stores mocks around the writer: `listen` keeps each handler by its event in
 * `listeners`, for the test to emit; an error is recorded by its title in `notified` (a toast) or `logged`
 */
export const settingsWriterMocks = {
  event: (listeners: Map<string, EventHandler>) => ({
    listen: async (name: string, handler: EventHandler) => {
      listeners.set(name, handler)
      return () => {}
    },
  }),
  errors: (notified: string[], logged: string[] = notified) => ({
    logError: (title: string) => logged.push(title),
    notifyError: (title: string) => notified.push(title),
  }),
}
