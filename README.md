# Branch Deck

**💡 Consider [GitButler](https://gitbutler.com) first**

Branch Deck is a workaround for [performance](https://github.com/gitbutlerapp/gitbutler/issues/3235) 
[issues](https://github.com/gitbutlerapp/gitbutler/issues/9018) with GitButler on large repositories.

Automatically create Git branches from commits with prefixed messages. Organize your work into separate branches without manual branch management.

![Branch Deck Screenshot](branch-deck-screenshot.png)

## What It Does

Branch Deck creates Git branches from your commits based on message prefixes. Commits with the same prefix get cherry-picked into their own dedicated branch.

## How It Works

Branch Deck analyzes your commit history and creates separate branches based on commit message patterns:

### Explicit Prefixes
Use the `(name)` format in your commit messages:
- Commits prefixed with `(feature-auth)` go to a `username/virtual/feature-auth` branch
- Commits prefixed with `(bugfix-login)` go to a `username/virtual/bugfix-login` branch

### Issue Numbers
If no explicit prefix is found, Branch Deck automatically detects issue numbers (e.g., `JIRA-123`, `ABC-456`) in the first line of the commit message:
- Commits with `IJPL-163558: Fix observability` go to a `username/virtual/IJPL-163558` branch
- Commits with `[threading] IJPL-163558: Fix observability` also go to `username/virtual/IJPL-163558` branch
- Square bracket prefixes like `[subsystem]` are ignored when grouping by issue numbers
- Multiple commits with the same issue number are grouped together
- Issue numbers must be in the first line of the commit message

### Priority
- Explicit prefixes `(name)` take precedence over issue numbers
- Commits without either pattern remain on the main branch

This allows you to maintain a clean, organized Git history while working on multiple features simultaneously.

### Current Limitations

- Sub-branches not yet supported: Nested branch structures (e.g., `(feature-auth/oauth)`) are not currently implemented.

## Why Branch Deck?

- **Simple**: Just add `(name)` prefixes or use issue numbers in your commit messages
- **Automatic**: Creates actual Git branches without manual branch management
- **Organized**: Each feature gets its own clean branch automatically
- **Flexible**: Work on multiple features simultaneously in one working directory

## Installation

[Download the latest release](https://github.com/develar/branch-deck/releases/latest)

## Contributing

See [DEVELOPMENT.md](DEVELOPMENT.md) for development setup and contribution guidelines.
