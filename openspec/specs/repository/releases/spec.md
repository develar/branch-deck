---
name: Release Signing
description: Developer ID Application signing and Apple notarization required before a macOS release is published.
style: plain-1
targets:
  - .github/workflows/build-tauri.yml
  - .github/workflows/release-please.yml
  - .github/workflows/test.yml
  - scripts/release-signing.test.mjs
---

# Release signing

Status: Implemented
Date: 2026-10-21

## Purpose

The release workflow builds the tagged version into the draft release of release-please. It publishes the release
only after every platform succeeds. macOS releases require Developer ID Application signing and Apple notarization;
the updater signatures remain separate. Setup and retry instructions: [Releases](../../../../docs/contributing/releases.md).

## Requirements

### Requirement: Credentials: Releases require all six Apple secrets

Automatic releases and manual retries MUST require all six Apple secrets: a missing secret MUST fail macOS
preparation before the build starts. There is no unsigned fallback.

#### Scenario: A secret is missing
- **WHEN** an automatic release or a manual retry runs and one of the six Apple secrets is missing
- **THEN** macOS preparation fails before the build starts
- **AND** no unsigned build follows
  [@test] scripts/release-signing.test.mjs (`macOS release requires signing credentials and cleans up its private API key`)

### Requirement: Credentials: The signing identity is Developer ID Application

A signing identity other than Developer ID Application MUST fail macOS preparation before the build starts.

#### Scenario: Another identity
- **WHEN** the signing identity is not a Developer ID Application certificate
- **THEN** macOS preparation fails before the build starts
  [@test] scripts/release-signing.test.mjs (`macOS release requires signing credentials and cleans up its private API key`)

### Requirement: Credentials: Apple credentials reach only the macOS steps

The workflow MUST pass Apple credentials only to macOS preparation and build steps.

#### Scenario: The steps of a release
- **WHEN** the release workflow runs
- **THEN** only the macOS preparation and build steps get the Apple credentials

### Requirement: Credentials: The API private key is private and removed

The API private key MUST be written under the runner's temporary directory with mode 0600, checked as a private key,
and removed after success or failure. Its contents MUST NOT be printed.

#### Scenario: A valid key
- **WHEN** macOS preparation writes a valid API private key
- **THEN** it lies under the runner's temporary directory with mode 0600
- **AND** its contents are not printed
- **AND** it is removed afterwards
  [@test] scripts/release-signing.test.mjs (`macOS release requires signing credentials and cleans up its private API key`)

#### Scenario: An invalid key
- **WHEN** the API private key is not a private key
- **THEN** the check fails macOS preparation
- **AND** the key is removed after the failure
  [@test] scripts/release-signing.test.mjs (`macOS release requires signing credentials and cleans up its private API key`)

### Requirement: Publication: The macOS job verifies the signatures and the notarization

The macOS job MUST verify the app and DMG signatures and the app's stapled notarization ticket after Tauri builds them.

#### Scenario: Tauri built the app and the DMG
- **WHEN** Tauri has built the app and the DMG in the macOS job
- **THEN** the job verifies their signatures and the app's stapled notarization ticket

### Requirement: Publication: The publish job waits for every platform

The publish job MUST depend on every platform's completed build job, including these checks. A failed check MUST leave
the release as a draft.

#### Scenario: Every platform succeeded
- **WHEN** every platform's build job completed, its checks included
- **THEN** the publish job runs

#### Scenario: A check failed
- **WHEN** a check of a build job fails
- **THEN** the release stays a draft

### Requirement: Publication: Pull request builds stay unsigned

Pull request builds MUST have no Apple credentials and SHALL remain unsigned.

#### Scenario: A build of a pull request
- **WHEN** the workflow builds a pull request
- **THEN** it has no Apple credentials and the build is unsigned
