# Proposal

## Why

A holder past its grace counts the most of its live use of the last 5 min only while its window counts. A window that
spans less than 5 min, whose last sample is older than 1 min, or whose most leaves less than a fifth of the host beside
it counted as the grace: the measure of the command, or the last sample. Of 39 holder-minutes past the grace of the
holders of 1.24.0 on 2026-11-25, 13 counted so, and their most of 5 min was 7.8 cores (p50) above the count. At 12:08
a fast lane of idea-1 had a peak of 17.3 and counted its measure of 12.6.

## What Changes

- A holder past its grace counts at least the most of the samples of the last 5 min that its window holds, its last
  sample too, whatever the span of the window and the age of its last sample. A holder without a measure, or with one
  of the whole host, still holds the whole host when its window counts as the grace.

Capabilities: `cli/host`.
