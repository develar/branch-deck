# Releases

`release-please.yml` creates a release pull request. Merging it creates the version tag and a draft GitHub release;
`build-tauri.yml` builds its macOS, Linux and Windows assets, writes `latest.json`, then publishes the release only
after every build and verification succeeds. A failed build leaves the release as a draft. Retry it with **Build
Tauri Assets → Run workflow**, using its existing tag.

## macOS signing and notarization

The macOS release requires an active Apple Developer Program membership, a **Developer ID Application** certificate
and an App Store Connect **team API key** with Developer access. In App Store Connect, open **Users and Access →
Integrations → App Store Connect API → Team Keys** to create the key. Apple allows its `.p8` private key to be
downloaded only once. Keep a private backup of the certificate, its private key and the API key outside the repository.

Add these repository secrets under **Settings → Secrets and variables → Actions**:

| Secret | Value |
| --- | --- |
| `APPLE_CERTIFICATE` | Base64 of the exported `.p12`, including the certificate and its private key. |
| `APPLE_CERTIFICATE_PASSWORD` | The nonempty password protecting the `.p12`. |
| `APPLE_SIGNING_IDENTITY` | The full identity, for example `Developer ID Application: Your Name (TEAMID)`. |
| `APPLE_API_ISSUER` | Issuer ID shown above the team API keys. |
| `APPLE_API_KEY` | Key ID of the App Store Connect API key. |
| `APPLE_API_PRIVATE_KEY` | The complete text of the downloaded `.p8`, including its BEGIN/END lines and real newlines. |

For a certificate held in Keychain Access, export it with its private key as `.p12`, protected by a password. Encode
the exported file for `APPLE_CERTIFICATE` with `openssl base64 -A -in certificate.p12 | pbcopy`. The identity is shown
by `security find-identity -v -p codesigning`. An Apple Development or Apple Distribution certificate cannot replace
Developer ID Application for distribution outside the Mac App Store.

Both automatic releases and manual retries use the same secrets. The macOS job checks that all six are set and that
the identity is Developer ID Application before installing build dependencies. It writes the API private key to a
file with owner-only permissions under `RUNNER_TEMP`, passes its path as `APPLE_API_KEY_PATH`, and deletes it even
when the build fails. The Apple credentials are supplied only to macOS preparation and build steps.

Tauri imports the `.p12` into a temporary keychain, signs with hardened runtime, submits the app to Apple's notary
service and staples its notarization ticket. It packages the stapled app in a signed DMG; the DMG has no separate
notarization ticket. The job verifies the code signatures of the app and DMG and validates the app's stapled ticket
before the publish job may run. A missing secret, invalid key, signing failure or notarization failure stops
publication; there is no unsigned release fallback. Pull request builds remain unsigned.

## Updater signatures

`TAURI_SIGNING_PRIVATE_KEY` and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` sign updater artifacts on every platform. These
are separate from Apple's code-signing certificate. Keep the existing updater key: installed apps verify updates
against the public key in `src-tauri/tauri.conf.json`.

References: [Tauri macOS signing](https://v2.tauri.app/distribute/sign/macos/),
[GitHub Actions secrets](https://docs.github.com/en/actions/security-for-github-actions/security-guides/using-secrets-in-github-actions).

Contract: [Release signing](../../spec/repository/releases.spec.md). The credential preparation and cleanup checks
run on macOS pull request jobs; run them locally with `pnpm exec node --test scripts/release-signing.test.mjs`.
