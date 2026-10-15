/** `@/lib/command` for store tests: the real module imports the toasts and the Activity store */
export const invokeCommand = <T>(run: () => Promise<T>): Promise<T> => run()
