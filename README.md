# Branch Deck

**💡 Consider [GitButler](https://gitbutler.com) first**

Branch Deck is a workaround for [performance](https://github.com/gitbutlerapp/gitbutler/issues/3235) 
[issues](https://github.com/gitbutlerapp/gitbutler/issues/9018) with GitButler on large repositories.

Automatically create Git branches from commits with prefixed messages. Organize your work into separate branches without manual branch management.

![Branch Deck Screenshot](branch-deck-screenshot.png)

## What It Does

Branch Deck creates Git branches from your commits based on message prefixes. Commits with the same prefix get cherry-picked into their own dedicated branch.

## How It Works

Branch Deck analyzes your commit history and creates separate branches based on commit message prefixes using the `[name]` format. For example:
- Commits prefixed with `[feature-auth]` go to a `feature-auth` branch
- Commits prefixed with `[bugfix-login]` go to a `bugfix-login` branch
- Commits without prefixes remain on the main branch

This allows you to maintain a clean, organized Git history while working on multiple features simultaneously.

### Current Limitations

- **Sub-branches not yet supported**: Nested branch structures (e.g., `[feature-auth/oauth]`) are not currently implemented.

## Why Branch Deck?

- 🎯 **Simple**: Just add `[name]` prefixes to your commit messages
- ⚡ **Automatic**: Creates actual Git branches without manual branch management
- 📱 **Organized**: Each feature gets its own clean branch automatically
- 🔄 **Flexible**: Work on multiple features simultaneously in one working directory

## Installation

[Download the latest release](https://github.com/develar/branch-deck/releases/latest)

Note: Auto-update is not yet supported but is planned for future releases.

## Contributing

See [DEVELOPMENT.md](DEVELOPMENT.md) for development setup and contribution guidelines.