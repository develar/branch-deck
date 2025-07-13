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

# Create branches that will have conflicts
echo -e "\n${BLUE}Creating branches with conflicts...${NC}"

# Save current branch
MAIN_BRANCH=$(git rev-parse --abbrev-ref HEAD)

# Create a base commit that will be modified differently in branches
echo "// Base configuration file
const config = {
  apiUrl: 'http://localhost:3000',
  timeout: 5000,
  retries: 3,
  features: {
    auth: true,
    analytics: false
  }
};

module.exports = config;" > config.js
git add config.js
git commit -m "Add base configuration file"

# Create another file that will have conflicts
echo "class UserService {
  constructor() {
    this.users = [];
  }

  addUser(user) {
    this.users.push(user);
  }

  getUser(id) {
    return this.users.find(u => u.id === id);
  }
}

module.exports = UserService;" > userService.js
git add userService.js
git commit -m "Add user service base implementation"

# Branch 1: Modify config and userService
git checkout -b conflict-branch-1
echo "// Configuration file - Updated by feature team
const config = {
  apiUrl: 'https://api.production.com',  // Changed to production URL
  timeout: 10000,  // Increased timeout
  retries: 5,      // More retries
  features: {
    auth: true,
    analytics: true,  // Enabled analytics
    logging: true     // Added logging
  },
  database: {         // Added database config
    host: 'db.production.com',
    port: 5432
  }
};

module.exports = config;" > config.js

echo "class UserService {
  constructor() {
    this.users = [];
    this.cache = new Map();  // Added caching
  }

  addUser(user) {
    // Validate user before adding
    if (!user.id || !user.email) {
      throw new Error('Invalid user data');
    }
    this.users.push(user);
    this.cache.set(user.id, user);
  }

  getUser(id) {
    // Check cache first
    if (this.cache.has(id)) {
      return this.cache.get(id);
    }
    return this.users.find(u => u.id === id);
  }

  deleteUser(id) {
    this.users = this.users.filter(u => u.id !== id);
    this.cache.delete(id);
  }
}

module.exports = UserService;" > userService.js

git add config.js userService.js
git commit -m "(feature-config) Update configuration for production and enhance user service"

# Go back to main and create conflicting changes
git checkout $MAIN_BRANCH

# Branch 2: Make different modifications to the same files
echo "// Configuration file - Updated by ops team
const config = {
  apiUrl: process.env.API_URL || 'http://localhost:3000',  // Made configurable
  timeout: 8000,    // Different timeout value
  retries: 3,
  features: {
    auth: true,
    analytics: false,
    monitoring: true  // Added monitoring instead of logging
  },
  redis: {           // Added Redis config instead of database
    host: 'redis.local',
    port: 6379
  }
};

module.exports = config;" > config.js

echo "class UserService {
  constructor(database) {  // Added dependency injection
    this.users = [];
    this.db = database;
  }

  async addUser(user) {  // Made async
    // Different validation logic
    if (!user.name || !user.email) {
      throw new Error('Name and email are required');
    }
    this.users.push(user);
    await this.db.save('users', user);  // Save to database
  }

  async getUser(id) {  // Made async
    const cachedUser = await this.db.get('users', id);
    if (cachedUser) return cachedUser;
    
    return this.users.find(u => u.id === id);
  }

  async updateUser(id, updates) {  // Added update method
    const userIndex = this.users.findIndex(u => u.id === id);
    if (userIndex !== -1) {
      this.users[userIndex] = { ...this.users[userIndex], ...updates };
      await this.db.save('users', this.users[userIndex]);
    }
  }
}

module.exports = UserService;" > userService.js

git add config.js userService.js
git commit -m "(ops-config) Make config environment-aware and add database support to user service"

# Create more conflicting branches
echo "import React from 'react';

function Button({ label, onClick }) {
  return (
    <button onClick={onClick}>
      {label}
    </button>
  );
}

export default Button;" > Button.jsx
git add Button.jsx
git commit -m "Add basic Button component"

# Branch 3: Style the button one way
git checkout -b ui-styling-1
echo "import React from 'react';
import './Button.css';

function Button({ label, onClick, variant = 'primary' }) {
  return (
    <button 
      className={\`btn btn-\${variant}\`}
      onClick={onClick}
    >
      {label}
    </button>
  );
}

Button.propTypes = {
  label: PropTypes.string.isRequired,
  onClick: PropTypes.func.isRequired,
  variant: PropTypes.oneOf(['primary', 'secondary', 'danger'])
};

export default Button;" > Button.jsx
git add Button.jsx
git commit -m "(ui-styling-1) Add CSS classes and prop validation to Button"

# Go back to main and create different styling
git checkout $MAIN_BRANCH

echo "import React from 'react';
import styled from 'styled-components';

const StyledButton = styled.button\`
  padding: 10px 20px;
  border-radius: 4px;
  border: none;
  background-color: \${props => props.primary ? '#007bff' : '#6c757d'};
  color: white;
  cursor: pointer;
  
  &:hover {
    opacity: 0.8;
  }
\`;

function Button({ label, onClick, primary = true }) {
  return (
    <StyledButton primary={primary} onClick={onClick}>
      {label}
    </StyledButton>
  );
}

export default Button;" > Button.jsx
git add Button.jsx
git commit -m "(ui-styling-2) Use styled-components for Button styling"

echo -e "${GREEN}✓ Created branches with conflicts${NC}"

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
echo "  - feature-config (1 commit) - WILL HAVE CONFLICTS with ops-config"
echo "  - ops-config (1 commit) - WILL HAVE CONFLICTS with feature-config"
echo "  - ui-styling-1 (1 commit) - WILL HAVE CONFLICTS with ui-styling-2"
echo "  - ui-styling-2 (1 commit) - WILL HAVE CONFLICTS with ui-styling-1"
echo "  - 1 commit without prefix (should remain on main branch)"
echo -e "\n${YELLOW}Conflicting branches:${NC}"
echo "  - feature-config vs ops-config: Different changes to config.js and userService.js"
echo "  - ui-styling-1 vs ui-styling-2: Different styling approaches for Button.jsx"
