---
name: Release Signing
description: Developer ID Application signing and Apple notarization required before a macOS release is published.
style: plain-1
targets:
  - ../../.github/workflows/build-tauri.yml
  - ../../.github/workflows/release-please.yml
  - ../../.github/workflows/test.yml
  - ../../scripts/release-signing.test.mjs
---

# Release signing

Status: Implemented
Date: 2026-10-21

## Summary

The release workflow builds the tagged version into the draft release of release-please. It publishes the release
only after every platform succeeds. macOS releases require Developer ID Application signing and Apple notarization;
the updater signatures remain separate. Setup and retry instructions: [Releases](../../docs/contributing/releases.md).

## Requirements

### Credentials

- Automatic releases and manual retries require all six Apple secrets. A missing secret or a signing identity other
  than Developer ID Application fails macOS preparation before the build starts. The workflow passes Apple
  credentials only to macOS preparation and build steps. There is no unsigned fallback.
- The API private key is written under the runner's temporary directory with mode 0600, checked as a private key,
  and removed after success or failure. Its contents are not printed.

[@test] ../../scripts/release-signing.test.mjs (`macOS release requires signing credentials and cleans up its private API key`)

### Publication

The macOS job verifies the app and DMG signatures and the app's stapled notarization ticket after Tauri builds them.
The publish job depends on every platform's completed build job, including these checks. A failed check leaves the
release as a draft. Pull request builds have no Apple credentials and remain unsigned.
