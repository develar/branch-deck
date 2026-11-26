import assert from "node:assert/strict"
import { readFileSync } from "node:fs"
import { test } from "node:test"
import { RULES, TEST_TRACE, normalized, program, real, replay } from "./host-admission.mjs"

test("the normalized words of a command put commit ids and long numbers aside, not words or short numbers", () => {
  assert.equal(normalized("scripts/bench gate --base 593e5c6e5af097b2b4d69dd4ddca78ebb67111ee --out gate-out/kt"), "scripts/bench gate --base <rev> --out gate-out/kt")
  assert.equal(normalized("gate.sh 1a2b3c4 --arm=893f4a213 run-1795688269900"), "gate.sh <rev> --arm=<rev> run-<n>")
  assert.equal(normalized("cargo nextest run -p accio --cargo-profile test-fast"), "cargo nextest run -p accio --cargo-profile test-fast")
  assert.equal(normalized("deadbeefcafe 1234"), "deadbeefcafe 1234", "letters alone are no commit id, a short number stays")
})

test("the program of a command is its first word and subcommand, or the first command of a script of a shell", () => {
  assert.equal(program("scripts/bench gate --base 593e5c6e --out x"), "scripts/bench gate")
  assert.equal(program("cargo nextest run --workspace"), "cargo nextest")
  assert.equal(program("zsh -c T=$1; ./community/tools/bt.cmd //plugins/air:$T --no-cache"), "./community/tools/bt.cmd")
  assert.equal(program("bash -lc 'pnpm e2e tests/a.spec.ts'"), "pnpm e2e")
  assert.equal(program("./gate.sh --base x"), "./gate.sh")
})

test("the replay of today stays near the real waits, and each stage waits less without a longer run", () => {
  const data = JSON.parse(readFileSync(TEST_TRACE, "utf8"))
  const measured = real(data)
  const today = replay(data, RULES.today)
  assert.ok(Math.abs(today.waitHours - measured.waitHours) <= 0.25 * measured.waitHours, `today ${today.waitHours} h against ${measured.waitHours} h`)
  const chain = replay(data, RULES["stage 1 + 2: the chain"])
  assert.ok(chain.waitHours < today.waitHours, `the chain ${chain.waitHours} h against ${today.waitHours} h`)
  assert.ok(chain.stretchP90 <= today.stretchP90 + 0.05, `the chain stretches ${chain.stretchP90} against ${today.stretchP90}`)
})
