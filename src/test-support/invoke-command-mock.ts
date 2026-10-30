/** `@/lib/command` for store tests: the real module imports the errors of the page, which import the toasts */
export const invokeCommand = <T>(run: () => Promise<T>): Promise<T> => run()
