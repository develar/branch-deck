#!/bin/bash

# Script to create a test git repository with commits that have branch prefixes
# Usage: ./scripts/create-test-repo.sh [path]
# If no path is provided, creates repo in ./test-repo

set -e

# Default path if not provided
REPO_PATH="${1:-./test-repo}"

# Colors for output
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo -e "${BLUE}Creating test git repository at: ${REPO_PATH}${NC}"

# Remove existing directory if it exists
if [ -d "$REPO_PATH" ]; then
    echo -e "${YELLOW}Removing existing directory: ${REPO_PATH}${NC}"
    rm -rf "$REPO_PATH"
fi

# Create and initialize git repository
mkdir -p "$REPO_PATH"
cd "$REPO_PATH"
git init

# Configure git user (required for commits)
git config user.email "test@example.com"
git config user.name "Test User"

echo -e "${GREEN}✓ Initialized git repository${NC}"

# Create initial commit
echo "# Test Repository

This is a test repository for branch-deck development.
It contains commits with branch prefixes in the format (branch-name).
" > README.md
git add README.md
git commit -m "Initial commit"

echo -e "${GREEN}✓ Created initial commit${NC}"

# Create commits with branch prefixes
echo -e "${BLUE}Creating commits with branch prefixes...${NC}"

# Feature auth commits
echo "User authentication system implementation" > auth.js
git add auth.js
git commit -m "(feature-auth) Add user authentication system"

echo "Password validation logic" >> auth.js
git add auth.js
git commit -m "(feature-auth) Improve password validation"

echo "Two-factor authentication support" >> auth.js
git add auth.js
git commit -m "(feature-auth) Add two-factor authentication"

# Bug fix commits
echo "Login timeout fix implementation" > login-fix.js
git add login-fix.js
git commit -m "(bugfix-login) Fix login timeout issue"

echo "Session expiry handling" > session-fix.js
git add session-fix.js
git commit -m "(bugfix-session) Fix session expiry handling"

# UI components commits
echo "Button component implementation" > button.vue
git add button.vue
git commit -m "(ui-components) Add new button component"

echo "Button styles and variants" >> button.vue
git add button.vue
git commit -m "(ui-components) Style button component with CSS"

echo "Modal component implementation" > modal.vue
git add modal.vue
git commit -m "(ui-components) Add modal component"

# API endpoints commits
echo "User profile API endpoint" > profile-api.js
git add profile-api.js
git commit -m "(api-endpoints) Create user profile endpoint"

echo "Settings API endpoint" > settings-api.js
git add settings-api.js
git commit -m "(api-endpoints) Add user settings endpoint"

# Documentation commits
echo "API documentation updates" > api-docs.md
git add api-docs.md
git commit -m "(docs) Update API documentation"

echo "Installation guide" > install.md
git add install.md
git commit -m "(docs) Add installation guide"

# Performance optimization commits
echo "Database query optimizations" > db-optimize.js
git add db-optimize.js
git commit -m "(perf) Optimize database queries"

echo "Caching implementation" > cache.js
git add cache.js
git commit -m "(perf) Add response caching"

# Regular commit without prefix
echo "General maintenance and cleanup" > maintenance.txt
git add maintenance.txt
git commit -m "Regular commit without branch prefix"

# Security commits
echo "Input validation improvements" > security.js
git add security.js
git commit -m "(security) Improve input validation"

echo "CSRF protection" >> security.js
git add security.js
git commit -m "(security) Add CSRF protection"

# Testing commits
echo "Unit tests for auth module" > auth.test.js
git add auth.test.js
git commit -m "(tests) Add authentication tests"

echo "Integration tests for API" > api.test.js
git add api.test.js
git commit -m "(tests) Add API integration tests"

echo -e "${GREEN}✓ Created all test commits${NC}"

# Display the commit history
echo -e "\n${BLUE}Commit history:${NC}"
git log --oneline --graph

echo -e "\n${GREEN}✓ Test repository created successfully!${NC}"
echo -e "${BLUE}Repository location: $(pwd)${NC}"
echo -e "${YELLOW}Expected branches to be created:${NC}"
echo "  - feature-auth (3 commits)"
echo "  - bugfix-login (1 commit)"
echo "  - bugfix-session (1 commit)"
echo "  - ui-components (3 commits)"
echo "  - api-endpoints (2 commits)"
echo "  - docs (2 commits)"
echo "  - perf (2 commits)"
echo "  - security (2 commits)"
echo "  - tests (2 commits)"
echo "  - 1 commit without prefix (should remain on main branch)"
