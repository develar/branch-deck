import assert from "node:assert/strict"
import { spawnSync } from "node:child_process"
import { mkdtempSync, readFileSync, rmSync, statSync, existsSync } from "node:fs"
import { tmpdir } from "node:os"
import { join } from "node:path"
import { test } from "node:test"

const workflow = readFileSync(new URL("../.github/workflows/build-tauri.yml", import.meta.url), "utf8")

// Execute the actual workflow shell snippets; these steps use no YAML interpolation in their scripts.
function scriptOf(name) {
  const step = workflow.split(`      - name: ${name}\n`)[1]?.split("\n      - ")[0]
  assert.ok(step, `Missing workflow step ${name}`)
  const run = step.split("        run: ")[1]
  assert.ok(run, `Missing script of ${name}`)
  return run.startsWith("|\n")
    ? run
        .slice(2)
        .trimEnd()
        .split("\n")
        .map(line => line.slice(10))
        .join("\n")
    : run.split("\n")[0].trim()
}

test("macOS release requires signing credentials and cleans up its private API key", { skip: process.platform === "win32" }, t => {
  const directory = mkdtempSync(join(tmpdir(), "release-signing-"))
  t.after(() => rmSync(directory, { recursive: true, force: true }))
  const fixture = join(directory, "fixture.p8")
  const generated = spawnSync("openssl", ["genpkey", "-algorithm", "EC", "-pkeyopt", "ec_paramgen_curve:P-256", "-out", fixture], { encoding: "utf8" })
  assert.equal(generated.status, 0, generated.stderr)
  const privateKey = readFileSync(fixture, "utf8")
  const required = ["APPLE_CERTIFICATE", "APPLE_CERTIFICATE_PASSWORD", "APPLE_SIGNING_IDENTITY", "APPLE_API_ISSUER", "APPLE_API_KEY", "APPLE_API_PRIVATE_KEY"]
  const environment = {
    ...process.env,
    ...Object.fromEntries(required.map(name => [name, "fixture-value"])),
    RUNNER_TEMP: directory,
    APPLE_SIGNING_IDENTITY: "Developer ID Application: Fixture (TESTTEAMID)",
    APPLE_API_PRIVATE_KEY: privateKey,
  }
  const target = join(directory, "branch-deck-notarization.p8")
  function execute(name, overrides = {}) {
    return spawnSync("bash", ["-euo", "pipefail", "-c", scriptOf(name)], {
      env: { ...environment, ...overrides },
      encoding: "utf8",
      timeout: 10_000,
    })
  }
  function clean() {
    const result = execute("Remove notarization key")
    assert.equal(result.status, 0, result.stderr)
    assert.equal(existsSync(target), false)
  }

  for (const name of required) {
    const result = execute("Prepare macOS signing and notarization", { [name]: "" })
    assert.equal(result.status, 1, result.stderr)
    assert.ok(result.stdout.includes(`Missing repository secret ${name}`))
    assert.equal(existsSync(target), false)
  }
  for (const identity of ["-", "Apple Development: Fixture (TESTTEAMID)", "Apple Distribution: Fixture (TESTTEAMID)"]) {
    const result = execute("Prepare macOS signing and notarization", { APPLE_SIGNING_IDENTITY: identity })
    assert.equal(result.status, 1, result.stderr)
    assert.ok(result.stdout.includes("must name a Developer ID Application certificate"))
    assert.equal(existsSync(target), false)
  }
  const invalid = execute("Prepare macOS signing and notarization", { APPLE_API_PRIVATE_KEY: "invalid-key" })
  assert.ok(invalid.status > 0, invalid.stderr)
  assert.equal(existsSync(target), true)
  clean()

  const valid = execute("Prepare macOS signing and notarization")
  assert.equal(valid.status, 0, valid.stderr)
  assert.equal(readFileSync(target, "utf8"), privateKey)
  assert.equal(statSync(target).mode & 0o777, 0o600)
  assert.ok(!(valid.stdout + valid.stderr).includes(privateKey))
  clean()
})
