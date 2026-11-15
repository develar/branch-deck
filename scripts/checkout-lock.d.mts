/** Takes the lock of a checkout for one run of `pnpm e2e` (`scripts/checkout-lock.mjs`); throws while another run holds it */
export function takeCheckoutLock(lock: string, pid: number, options?: { isAlive?: (pid: number) => boolean; afterRead?: () => void }): void
/** Drops the lock of the run `pid`; a lock of another run stays */
export function releaseCheckoutLock(lock: string, pid: number): void
