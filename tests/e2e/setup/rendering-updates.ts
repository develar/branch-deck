/**
 * The page side of a wait for a rendering update, an init script of every context (`base-test.ts`):
 * `window.__renderingUpdate()` resolves after the next rendering update of the page with the time of that update
 * (`document.timeline`, which the test clock does not fake). A test cannot wait for one by `requestAnimationFrame`: the
 * clock the fixture installs (`context.clock.install`) runs its callbacks as timers of 16 ms, and a loaded page runs
 * several of them between two rendering updates, or none in one.
 *
 * The wait is the first report of a `ResizeObserver` made for it on the root element: a new observation of an element
 * with a size is reported in the first rendering update after it, and the observer made last is called after the
 * others of that update. The observation is made in a task of its own, so a call from the delivery of an observer adds
 * no observation to it (that would be a loop of the test's own), and it resolves in the second task after the report,
 * after the tasks the observers of that update posted (`nextTask` of src/lib/observe-size.ts). Its observer carries
 * `Symbol.for("branch-deck.e2eRenderingUpdate")`, which the trace of the observers (`resize-observers.ts`) leaves out
 */
export function renderingUpdates(): void {
  const mark = Symbol.for("branch-deck.e2eRenderingUpdate")
  const nextTask = () =>
    new Promise<void>(resolve => {
      const channel = new MessageChannel()
      channel.port1.onmessage = () => {
        channel.port1.close()
        resolve()
      }
      channel.port2.postMessage(null)
    })
  const renderingUpdate = async (): Promise<number> => {
    await nextTask()
    const at = await new Promise<number>(resolve => {
      const observer = new ResizeObserver(() => {
        observer.disconnect()
        resolve(Number(document.timeline.currentTime))
      })
      Reflect.set(observer, mark, true)
      observer.observe(document.documentElement)
    })
    await nextTask()
    await nextTask()
    return at
  }
  Object.assign(window, { __renderingUpdate: renderingUpdate })
}

/** The window of a page with `renderingUpdates` installed, for a cast in a function that runs in the page */
export interface RenderingUpdates {
  __renderingUpdate: () => Promise<number>
}
