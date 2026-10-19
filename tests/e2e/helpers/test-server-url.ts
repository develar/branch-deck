/**
 * The mock server of this run. A second checkout (a lease worktree) runs its own on another port:
 * `E2E_PORT=3031 pnpm e2e`. The pages reach it as their own origin.
 */
export const TEST_SERVER_PORT = Number(process.env.E2E_PORT ?? 3030)
export const TEST_SERVER_URL = `http://localhost:${TEST_SERVER_PORT}`
