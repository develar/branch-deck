/** A store whose changes an ask is repeated on: a zustand store, any of whose states */
export interface ChangeSource {
  subscribe(listener: () => void): () => void
}

/**
 * Asks `ask` until `settled` holds for its answer, again after each change of `sources`. The changes are watched from
 * before each ask: what an answer waits for may come while it is asked, ahead of the answer (a patch of the session
 * before the answer of a command it sent earlier), and a wait that subscribed only after the answer would wait for a
 * change that already came
 */
export async function askUntilSettled<T>(ask: () => Promise<T>, settled: (answer: T) => boolean, sources: readonly ChangeSource[]): Promise<T> {
  for (;;) {
    const change = watchChange(sources)
    let answer: T
    try {
      answer = await ask()
    } catch (error) {
      change.stop()
      throw error
    }
    if (settled(answer)) {
      change.stop()
      return answer
    }
    await change.changed
  }
}

/** The first change of `sources` since the call (`changed`); `stop` ends the watch */
function watchChange(sources: readonly ChangeSource[]): { readonly changed: Promise<void>; readonly stop: () => void } {
  let unsubscribes: (() => void)[] = []
  const stop = () => {
    unsubscribes.forEach(unsubscribe => unsubscribe())
    unsubscribes = []
  }
  const changed = new Promise<void>(resolve => {
    unsubscribes = sources.map(source =>
      source.subscribe(() => {
        stop()
        resolve()
      }),
    )
  })
  return { changed, stop }
}
