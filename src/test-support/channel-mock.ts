/** `@tauri-apps/api/core` for store tests: a `Channel` whose `onmessage` the mocked command calls with its events */
export class Channel<T = unknown> {
  /** The id Stop Waiting names the channel by (`cancel_lock_wait`) */
  readonly id = 0
  onmessage: (event: T) => void = () => {}
}
