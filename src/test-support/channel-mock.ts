/** `@tauri-apps/api/core` for store tests: a `Channel` whose `onmessage` the mocked command calls with its events */
export class Channel<T = unknown> {
  /** The id Tauri gives a channel */
  readonly id = 0
  onmessage: (event: T) => void = () => {}
}
