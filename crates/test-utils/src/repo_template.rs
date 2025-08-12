use anyhow::Result;
use std::fs;
use std::path::Path;
use std::process::Command;

/// A builder for creating test repository templates
pub struct RepoTemplate {
  #[allow(dead_code)]
  name: String,
  branch_prefix: Option<String>,
  commits: Vec<CommitSpec>,
}

struct CommitSpec {
  message: String,
  files: Vec<(String, String)>,
  timestamp: Option<i64>,
}

impl RepoTemplate {
  pub fn new(name: impl Into<String>) -> Self {
    Self {
      name: name.into(),
      branch_prefix: None,
      commits: Vec::new(),
    }
  }

  pub fn branch_prefix(mut self, prefix: impl Into<String>) -> Self {
    self.branch_prefix = Some(prefix.into());
    self
  }

  pub fn commit(self, message: impl Into<String>, files: &[(&str, &str)]) -> Self {
    self.commit_with_timestamp(message, files, None)
  }

  pub fn commit_with_timestamp(mut self, message: impl Into<String>, files: &[(&str, &str)], timestamp: Option<i64>) -> Self {
    let files = files.iter().map(|(path, content)| (path.to_string(), content.to_string())).collect();

    self.commits.push(CommitSpec {
      message: message.into(),
      files,
      timestamp,
    });
    self
  }

  /// Build the repository at the specified path
  pub fn build(self, output_path: &Path) -> Result<()> {
    // Create directory
    fs::create_dir_all(output_path)?;

    // Initialize git repository
    Command::new("git").args(["init", "--initial-branch=master"]).current_dir(output_path).output()?;

    // Configure git
    Command::new("git").args(["config", "user.name", "Test User"]).current_dir(output_path).output()?;

    Command::new("git").args(["config", "user.email", "test@example.com"]).current_dir(output_path).output()?;

    // Set branch prefix if specified
    if let Some(prefix) = &self.branch_prefix {
      Command::new("git").args(["config", "branchdeck.branchPrefix", prefix]).current_dir(output_path).output()?;
    }

    // Track if we have any commits
    let has_commits = !self.commits.is_empty();

    // Create commits
    for commit in self.commits {
      // Write files
      for (file_path, content) in &commit.files {
        let full_path = output_path.join(file_path);
        if let Some(parent) = full_path.parent() {
          fs::create_dir_all(parent)?;
        }
        fs::write(&full_path, content)?;

        // Stage file
        Command::new("git").args(["add", file_path]).current_dir(output_path).output()?;
      }

      // Commit with optional timestamp
      let mut cmd = Command::new("git");

      // If timestamp is provided, set GIT_AUTHOR_DATE and GIT_COMMITTER_DATE
      if let Some(ts) = commit.timestamp {
        let date_str = format!("{ts} +0000");
        cmd.env("GIT_AUTHOR_DATE", &date_str);
        cmd.env("GIT_COMMITTER_DATE", &date_str);
      }

      cmd.args(["commit", "-m", &commit.message]).current_dir(output_path).output()?;
    }

    // Add a fake origin remote pointing to self for testing
    Command::new("git").args(["remote", "add", "origin", "."]).current_dir(output_path).output()?;

    // For templates that simulate existing repositories with history,
    // create origin/master at the initial commit. This is needed for
    // sync_branches to work properly.
    if has_commits {
      // Get the initial commit
      let output = Command::new("git").args(["rev-list", "--max-parents=0", "HEAD"]).current_dir(output_path).output()?;

      let initial_commit = String::from_utf8(output.stdout)?.trim().to_string();

      // Create origin/master at the initial commit
      Command::new("git")
        .args(["update-ref", "refs/remotes/origin/master", &initial_commit])
        .current_dir(output_path)
        .output()?;
    }

    Ok(())
  }
}

/// Pre-defined test repository templates
pub mod templates {
  use super::RepoTemplate;
  use anyhow::Result;
  use std::fs;
  use std::path::Path;
  use std::process::Command;

  /// Simple repository with 2 commits using branch prefix
  pub fn simple() -> RepoTemplate {
    // Use fixed timestamps: Jan 1, 2024 14:00:00 UTC and 14:30:00 UTC
    RepoTemplate::new("simple")
      .branch_prefix("user-name")
      .commit_with_timestamp("(test-branch) foo 1", &[("file1.txt", "Content 1")], Some(1704117600))
      .commit_with_timestamp("(test-branch) foo 2", &[("file2.txt", "Content 2")], Some(1704119400))
  }

  /// Repository with unassigned commits (no branch prefix in commit messages)
  pub fn unassigned() -> RepoTemplate {
    // Use fixed timestamps: Jan 1, 2024 starting at 14:00:00 UTC, incrementing by 30 minutes
    RepoTemplate::new("unassigned")
      .branch_prefix("user-name")
      .commit_with_timestamp("Initial commit", &[("README.md", "# Test Project\n\nThis is a test project.")], Some(1704117600))
      .commit_with_timestamp(
        "Add user authentication",
        &[("auth.js", "// Authentication logic\nexport function login() {}")],
        Some(1704119400),
      )
      .commit_with_timestamp(
        "Fix login validation bug",
        &[("auth.js", "// Authentication logic\nexport function login() {\n  // Fixed validation\n}")],
        Some(1704121200),
      )
  }

  /// Create a proper conflict scenario with interleaved commits
  fn create_conflict_scenario(name: &str, scenario_type: &str) -> RepoTemplate {
    // Use fixed timestamps starting at Jan 1, 2024 14:00:00 UTC, incrementing by 30 minutes
    let mut template = RepoTemplate::new(name)
      .branch_prefix("user-name")
      // Initial commit - this becomes origin/master
      .commit_with_timestamp(
        "Initial project setup",
        &[
          ("README.md", "# Test Repository\n\nGenerated test repository for Branch Deck conflict demonstration.\n"),
          (".gitignore", "*.class\nbuild/\n.gradle/\n.idea/\n*.iml\n"),
          (
            "build.gradle.kts",
            r#"plugins {
    kotlin("jvm") version "1.9.20"
    id("org.springframework.boot") version "3.1.5"
}

dependencies {
    implementation("org.springframework.boot:spring-boot-starter-web")
    implementation("org.jetbrains.kotlin:kotlin-reflect")
}
"#,
          ),
        ],
        Some(1704117600),
      );

    match scenario_type {
      "unassigned" => {
        // Create unassigned commits scenario
        template = template
          // Unassigned commit that will be pushed
          .commit_with_timestamp(
            "Add UserService",
            &[(
              "src/main/kotlin/com/example/service/UserService.kt",
              r#"package com.example.service

import org.springframework.stereotype.Service

@Service
class UserService {
    private val users = mutableMapOf<String, User>()
    
    fun getUser(id: String): User? = users[id]
    
    fun createUser(name: String, email: String): User {
        val user = User(
            id = generateId(),
            name = name,
            email = email
        )
        users[user.id] = user
        return user
    }
    
    private fun generateId(): String = "user-${System.currentTimeMillis()}"
}

data class User(
    val id: String,
    val name: String,
    val email: String
)"#,
            )],
            Some(1704119400),
          )
          // Auth feature commits
          .commit_with_timestamp(
            "(feature-auth) Add authentication to UserService",
            &[(
              "src/main/kotlin/com/example/service/UserService.kt",
              r#"package com.example.service

import org.springframework.stereotype.Service

@Service
class UserService {
    private val users = mutableMapOf<String, User>()
    private val tokens = mutableMapOf<String, String>() // token -> userId
    
    fun getUser(id: String): User? = users[id]
    
    fun createUser(name: String, email: String, password: String): User {
        val user = User(
            id = generateId(),
            name = name,
            email = email,
            passwordHash = hashPassword(password)
        )
        users[user.id] = user
        return user
    }
    
    fun authenticate(email: String, password: String): String? {
        val user = users.values.find { it.email == email }
        return if (user != null && verifyPassword(password, user.passwordHash)) {
            val token = generateToken()
            tokens[token] = user.id
            token
        } else null
    }
    
    fun getUserByToken(token: String): User? {
        val userId = tokens[token] ?: return null
        return users[userId]
    }
    
    private fun generateId(): String = "user-${System.currentTimeMillis()}"
    private fun generateToken(): String = "token-${System.currentTimeMillis()}"
    private fun hashPassword(password: String): String = password.reversed() // Simple fake hash
    private fun verifyPassword(password: String, hash: String): Boolean = password.reversed() == hash
}

data class User(
    val id: String,
    val name: String,
    val email: String,
    val passwordHash: String = ""
)"#,
            )],
            Some(1704121200),
          )
          // Cache feature commits
          .commit_with_timestamp(
            "(feature-cache) Add caching to UserService",
            &[(
              "src/main/kotlin/com/example/service/UserService.kt",
              r#"package com.example.service

import org.springframework.stereotype.Service

@Service
class UserService {
    private val users = mutableMapOf<String, User>()
    private val tokens = mutableMapOf<String, String>() // token -> userId
    private val cache = mutableMapOf<String, User>() // Simple cache
    
    fun getUser(id: String): User? {
        // Check cache first
        cache[id]?.let { return it }
        
        // Load from storage and cache
        val user = users[id]
        if (user != null) {
            cache[id] = user
        }
        return user
    }
    
    fun createUser(name: String, email: String, password: String): User {
        val user = User(
            id = generateId(),
            name = name,
            email = email,
            passwordHash = hashPassword(password),
            cached = false
        )
        users[user.id] = user
        cache[user.id] = user.copy(cached = true)
        return user
    }
    
    fun authenticate(email: String, password: String): String? {
        val user = users.values.find { it.email == email }
        return if (user != null && verifyPassword(password, user.passwordHash)) {
            val token = generateToken()
            tokens[token] = user.id
            token
        } else null
    }
    
    fun getUserByToken(token: String): User? {
        val userId = tokens[token] ?: return null
        return getUser(userId) // Uses cached version
    }
    
    fun clearCache() {
        cache.clear()
    }
    
    private fun generateId(): String = "user-${System.currentTimeMillis()}"
    private fun generateToken(): String = "token-${System.currentTimeMillis()}"
    private fun hashPassword(password: String): String = password.reversed()
    private fun verifyPassword(password: String, hash: String): Boolean = password.reversed() == hash
}

data class User(
    val id: String,
    val name: String,
    val email: String,
    val passwordHash: String = "",
    val cached: Boolean = false
)"#,
            )],
            Some(1704123000),
          )
          // Auth depends on cache - will conflict when grouped
          .commit_with_timestamp(
            "(feature-auth) Add JWT tokens using cache",
            &[(
              "src/main/kotlin/com/example/service/UserService.kt",
              r#"package com.example.service

import org.springframework.stereotype.Service
import java.util.Base64

@Service
class UserService {
    private val users = mutableMapOf<String, User>()
    private val tokens = mutableMapOf<String, String>()
    private val cache = mutableMapOf<String, User>()
    private val jwtCache = mutableMapOf<String, JwtToken>() // Cache JWT tokens
    
    fun getUser(id: String): User? {
        cache[id]?.let { return it }
        
        val user = users[id]
        if (user != null) {
            cache[id] = user
        }
        return user
    }
    
    fun createUser(name: String, email: String, password: String): User {
        val user = User(
            id = generateId(),
            name = name,
            email = email,
            passwordHash = hashPassword(password),
            cached = false
        )
        users[user.id] = user
        cache[user.id] = user.copy(cached = true)
        return user
    }
    
    fun authenticate(email: String, password: String): String? {
        val user = users.values.find { it.email == email }
        return if (user != null && verifyPassword(password, user.passwordHash)) {
            // Check JWT cache first
            val cachedJwt = jwtCache[user.id]
            if (cachedJwt != null && !cachedJwt.isExpired()) {
                return cachedJwt.token
            }
            
            // Generate new JWT
            val jwt = generateJWT(user.id)
            jwtCache[user.id] = jwt
            jwt.token
        } else null
    }
    
    fun getUserByToken(token: String): User? {
        // Check if it's a JWT token
        if (token.startsWith("jwt.")) {
            val userId = decodeJWT(token)
            return userId?.let { getUser(it) }
        }
        
        // Legacy token support
        val userId = tokens[token] ?: return null
        return getUser(userId)
    }
    
    fun clearCache() {
        cache.clear()
        jwtCache.clear()
    }
    
    private fun generateId(): String = "user-${System.currentTimeMillis()}"
    private fun generateToken(): String = "token-${System.currentTimeMillis()}"
    private fun hashPassword(password: String): String = password.reversed()
    private fun verifyPassword(password: String, hash: String): Boolean = password.reversed() == hash
    
    private fun generateJWT(userId: String): JwtToken {
        val token = "jwt.${Base64.getEncoder().encodeToString(userId.toByteArray())}.${System.currentTimeMillis()}"
        return JwtToken(token, System.currentTimeMillis() + 3600000)
    }
    
    private fun decodeJWT(token: String): String? {
        return try {
            val parts = token.split(".")
            if (parts.size >= 2) {
                String(Base64.getDecoder().decode(parts[1]))
            } else null
        } catch (e: Exception) {
            null
        }
    }
}

data class User(
    val id: String,
    val name: String,
    val email: String,
    val passwordHash: String = "",
    val cached: Boolean = false
)

data class JwtToken(
    val token: String,
    val expiresAt: Long
) {
    fun isExpired(): Boolean = System.currentTimeMillis() > expiresAt
}"#,
            )],
            Some(1704124800),
          )
          // Add unassigned commits that depend on each other (for conflict testing)
          .commit_with_timestamp(
            "Add bcrypt dependency",
            &[(
              "build.gradle.kts",
              r#"plugins {
    kotlin("jvm") version "1.9.20"
    id("org.springframework.boot") version "3.1.5"
}

dependencies {
    implementation("org.springframework.boot:spring-boot-starter-web")
    implementation("org.jetbrains.kotlin:kotlin-reflect")
    implementation("org.mindrot:jbcrypt:0.4")
}
"#,
            )],
            Some(1704124800),
          )
          .commit_with_timestamp(
            "Implement secure password hashing",
            &[(
              "src/main/kotlin/com/example/service/UserService.kt",
              r#"package com.example.service

import org.springframework.stereotype.Service
import org.mindrot.jbcrypt.BCrypt

@Service
class UserService {
    private val users = mutableMapOf<String, User>()
    private val tokens = mutableMapOf<String, String>() // token -> userId

    fun getUser(id: String): User? = users[id]

    fun createUser(name: String, email: String, password: String): User {
        val user = User(
            id = generateId(),
            name = name,
            email = email,
            passwordHash = BCrypt.hashpw(password, BCrypt.gensalt()) // Using bcrypt
        )
        users[user.id] = user
        return user
    }

    fun authenticate(email: String, password: String): String? {
        val user = users.values.find { it.email == email }
        return if (user != null && BCrypt.checkpw(password, user.passwordHash)) {
            val token = generateToken()
            tokens[token] = user.id
            token
        } else null
    }

    fun getUserByToken(token: String): User? {
        val userId = tokens[token] ?: return null
        return users[userId]
    }

    private fun generateId(): String = "user-${System.currentTimeMillis()}"
    private fun generateToken(): String = "token-${System.currentTimeMillis()}"
}

data class User(
    val id: String,
    val name: String,
    val email: String,
    val passwordHash: String = ""
)"#,
            )],
            Some(1704126600),
          )
          // Unassigned refactoring
          .commit_with_timestamp(
            "Refactor: Extract configuration constants",
            &[(
              "src/main/kotlin/com/example/config/AppConfig.kt",
              r#"package com.example.config

object AppConfig {
    const val JWT_EXPIRY_MS = 3600000L // 1 hour
    const val CACHE_SIZE_LIMIT = 1000
    const val TOKEN_PREFIX = "jwt."
}"#,
            )],
            Some(1704128400),
          );
      }
      "branches" => {
        // Create branches with conflicts scenario - matching E2E test expectations
        template = template
          // Unassigned base commit
          .commit_with_timestamp(
            "Add UserService",
            &[(
              "src/main/kotlin/com/example/service/UserService.kt",
              r#"package com.example.service

import org.springframework.stereotype.Service

@Service
class UserService {
    private val users = mutableMapOf<String, User>()
    
    fun getUser(id: String): User? = users[id]
    
    fun createUser(name: String, email: String): User {
        val user = User(
            id = generateId(),
            name = name,
            email = email
        )
        users[user.id] = user
        return user
    }
    
    private fun generateId(): String = "user-${System.currentTimeMillis()}"
}

data class User(
    val id: String,
    val name: String,
    val email: String
)"#,
            )],
            Some(1704119400),
          )
          // feature-auth commits that sync cleanly
          .commit_with_timestamp(
            "(feature-auth) Add authentication to UserService",
            &[(
              "src/main/kotlin/com/example/service/UserService.kt",
              r#"package com.example.service

import org.springframework.stereotype.Service

@Service
class UserService {
    private val users = mutableMapOf<String, User>()
    private val tokens = mutableMapOf<String, String>() // token -> userId
    
    fun getUser(id: String): User? = users[id]
    
    fun createUser(name: String, email: String, password: String): User {
        val user = User(
            id = generateId(),
            name = name,
            email = email,
            passwordHash = hashPassword(password)
        )
        users[user.id] = user
        return user
    }
    
    fun authenticate(email: String, password: String): String? {
        val user = users.values.find { it.email == email }
        return if (user != null && verifyPassword(password, user.passwordHash)) {
            val token = generateToken()
            tokens[token] = user.id
            token
        } else null
    }
    
    fun getUserByToken(token: String): User? {
        val userId = tokens[token] ?: return null
        return users[userId]
    }
    
    private fun generateId(): String = "user-${System.currentTimeMillis()}"
    private fun generateToken(): String = "token-${System.currentTimeMillis()}"
    private fun hashPassword(password: String): String = password.reversed() // Simple fake hash
    private fun verifyPassword(password: String, hash: String): Boolean = password.reversed() == hash
}

data class User(
    val id: String,
    val name: String,
    val email: String,
    val passwordHash: String = ""
)"#,
            )],
            Some(1704121200),
          )
          .commit_with_timestamp(
            "(feature-auth) Add user roles and permissions",
            &[(
              "src/main/kotlin/com/example/model/Roles.kt",
              r#"package com.example.model

enum class Role {
    ADMIN, USER, GUEST
}

data class Permission(
    val resource: String,
    val actions: Set<String>
)

object RolePermissions {
    val permissions = mapOf(
        Role.ADMIN to setOf(
            Permission("users", setOf("read", "write", "delete")),
            Permission("settings", setOf("read", "write"))
        ),
        Role.USER to setOf(
            Permission("users", setOf("read")),
            Permission("settings", setOf("read"))
        ),
        Role.GUEST to setOf(
            Permission("users", setOf("read"))
        )
    )
    
    fun hasPermission(role: Role, resource: String, action: String): Boolean {
        return permissions[role]?.any { 
            it.resource == resource && it.actions.contains(action) 
        } ?: false
    }
}"#,
            )],
            Some(1704123000),
          )
          // Unassigned commit (bcrypt dependency) - expected by E2E test
          .commit_with_timestamp(
            "Add bcrypt dependency",
            &[(
              "build.gradle.kts",
              r#"plugins {
    kotlin("jvm") version "1.9.20"
    id("org.springframework.boot") version "3.1.5"
}

dependencies {
    implementation("org.springframework.boot:spring-boot-starter-web")
    implementation("org.jetbrains.kotlin:kotlin-reflect")
    implementation("org.mindrot:jbcrypt:0.4")
}
"#,
            )],
            Some(1704124800),
          )
          // bug-fix branch that depends on bcrypt (will conflict)
          .commit_with_timestamp(
            "(bug-fix) Implement secure password hashing",
            &[(
              "src/main/kotlin/com/example/service/UserService.kt",
              r#"package com.example.service

import org.springframework.stereotype.Service
import org.mindrot.jbcrypt.BCrypt

@Service
class UserService {
    private val users = mutableMapOf<String, User>()
    private val tokens = mutableMapOf<String, String>() // token -> userId
    
    fun getUser(id: String): User? = users[id]
    
    fun createUser(name: String, email: String, password: String): User {
        val user = User(
            id = generateId(),
            name = name,
            email = email,
            passwordHash = BCrypt.hashpw(password, BCrypt.gensalt()) // Using bcrypt
        )
        users[user.id] = user
        return user
    }
    
    fun authenticate(email: String, password: String): String? {
        val user = users.values.find { it.email == email }
        return if (user != null && BCrypt.checkpw(password, user.passwordHash)) {
            val token = generateToken()
            tokens[token] = user.id
            token
        } else null
    }
    
    fun getUserByToken(token: String): User? {
        val userId = tokens[token] ?: return null
        return users[userId]
    }
    
    private fun generateId(): String = "user-${System.currentTimeMillis()}"
    private fun generateToken(): String = "token-${System.currentTimeMillis()}"
}

data class User(
    val id: String,
    val name: String,
    val email: String,
    val passwordHash: String = ""
)"#,
            )],
            Some(1704126600),
          );
      }
      _ => unreachable!(),
    }

    template
  }

  /// Repository with unassigned commits that will have missing commits when assigned
  pub fn conflict_unassigned() -> RepoTemplate {
    create_conflict_scenario("conflict_unassigned", "unassigned")
  }

  /// Repository with branches where some commits are missing prefixes
  pub fn conflict_branches() -> RepoTemplate {
    create_conflict_scenario("conflict_branches", "branches")
  }

  /// Repository with exactly one unassigned commit for testing singular form
  pub fn single_unassigned() -> RepoTemplate {
    // Use fixed timestamps: Jan 1, 2024 starting at 14:00:00 UTC, incrementing by 30 minutes
    RepoTemplate::new("single_unassigned")
      .branch_prefix("user-name")
      .commit_with_timestamp("Initial setup", &[("README.md", "# Project\n\nInitial project setup.")], Some(1704117600))
      .commit_with_timestamp(
        "(feature) Add authentication",
        &[("auth.js", "// Authentication module\nexport function authenticate() {}")],
        Some(1704119400),
      )
      .commit_with_timestamp("Fix critical bug", &[("bugfix.txt", "Critical bug fix for production issue")], Some(1704121200)) // This is the single unassigned commit
  }

  /// Repository with issue navigation configuration for testing issue links
  pub fn issue_links() -> RepoTemplate {
    // Use fixed timestamps: Jan 1, 2024 starting at 14:00:00 UTC, incrementing by 30 minutes
    RepoTemplate::new("issue_links")
      .branch_prefix("user-name")
      // Initial commit with IntelliJ IDEA issue navigation config
      .commit_with_timestamp(
        "Initial project setup",
        &[
          ("README.md", "# Test Repository\n\nRepository for testing issue link navigation.\n"),
          (
            ".idea/vcs.xml",
            r##"<?xml version="1.0" encoding="UTF-8"?>
<project version="4">
  <component name="IssueNavigationConfiguration">
    <option name="links">
      <list>
        <IssueNavigationLink>
          <option name="issueRegexp" value="\b[A-Z]+-\d+\b" />
          <option name="linkRegexp" value="https://jira.example.com/browse/$0" />
        </IssueNavigationLink>
        <IssueNavigationLink>
          <option name="issueRegexp" value="GH-(\d+)" />
          <option name="linkRegexp" value="https://github.com/example/repo/issues/$1" />
        </IssueNavigationLink>
        <IssueNavigationLink>
          <option name="issueRegexp" value="#(\d+)" />
          <option name="linkRegexp" value="https://github.com/example/repo/issues/$1" />
        </IssueNavigationLink>
      </list>
    </option>
  </component>
</project>"##,
          ),
        ],
        Some(1704117600),
      )
      // Commits with various issue references
      .commit_with_timestamp(
        "(feature-auth) JIRA-123: Add authentication service",
        &[("auth.js", "// Auth service for JIRA-123\nexport function authenticate() {}")],
        Some(1704119400),
      )
      .commit_with_timestamp(
        "(feature-api) Fix API endpoint for GH-456",
        &[("api.js", "// Fix for issue GH-456\nexport function fixedApi() {}")],
        Some(1704121200),
      )
      .commit_with_timestamp(
        "(feature-ui) Update UI components (#789)",
        &[("ui.js", "// UI update for issue #789\nexport function updateUI() {}")],
        Some(1704123000),
      )
      .commit_with_timestamp(
        "(feature-db) TEST-001 and PROD-999: Database optimization",
        &[("db.js", "// Optimizations for TEST-001 and PROD-999\nexport function optimizeDB() {}")],
        Some(1704124800),
      )
      // Commit without any issue references
      .commit_with_timestamp(
        "(feature-docs) Update documentation",
        &[("docs.md", "# Documentation\n\nUpdated project documentation.")],
        Some(1704126600),
      )
      // Multi-commit issue branch ABC-123 (4 commits)
      .commit_with_timestamp(
        "(ABC-123) Add user authentication module",
        &[("auth/module.js", "// User authentication module\nexport class AuthModule {}")],
        Some(1704128400),
      )
      .commit_with_timestamp(
        "(ABC-123) Add password hashing utility",
        &[("auth/hash.js", "// Password hashing utility\nexport function hashPassword() {}")],
        Some(1704130200),
      )
      .commit_with_timestamp(
        "(ABC-123) Add session management",
        &[("auth/session.js", "// Session management\nexport class SessionManager {}")],
        Some(1704132000),
      )
      .commit_with_timestamp(
        "(ABC-123) Add authentication tests",
        &[("test/auth.test.js", "// Authentication tests\ntest('should authenticate user', () => {})")],
        Some(1704133800),
      )
      // Multi-commit issue branch JIRA-456 (3 commits)
      .commit_with_timestamp(
        "(JIRA-456) Fix database connection pool timeout",
        &[("db/pool.js", "// Database connection pool\nexport const pool = { timeout: 30000 }")],
        Some(1704135600),
      )
      .commit_with_timestamp(
        "(JIRA-456) Increase pool size to 50",
        &[("db/pool.js", "// Database connection pool\nexport const pool = { timeout: 30000, size: 50 }")],
        Some(1704137400),
      )
      .commit_with_timestamp(
        "(JIRA-456) Add connection retry logic",
        &[("db/retry.js", "// Connection retry logic\nexport function retryConnection() {}")],
        Some(1704139200),
      )
      // Unassigned commit with issue reference
      .commit_with_timestamp(
        "Emergency fix for CRITICAL-111",
        &[("fix.js", "// Emergency fix for CRITICAL-111\nexport function emergencyFix() {}")],
        Some(1704141000),
      )
  }

  /// Directory without git initialization - for testing invalid repository paths
  pub fn empty_non_git() -> EmptyNonGitTemplate {
    EmptyNonGitTemplate
  }

  /// Template that creates a directory without git initialization
  pub struct EmptyNonGitTemplate;

  impl EmptyNonGitTemplate {
    pub fn build(self, output_path: &Path) -> Result<()> {
      // Create directory
      fs::create_dir_all(output_path)?;

      // Add a simple file so the directory is not empty
      fs::write(output_path.join("README.txt"), "This is not a git repository")?;

      Ok(())
    }
  }

  /// Repository with archived branches (integrated, partially integrated, and not integrated)
  /// Implements its own build to create archived refs under user-name/archived/<date>/...
  pub struct ArchivedBranchesTemplate;

  pub fn archived_branches() -> ArchivedBranchesTemplate {
    ArchivedBranchesTemplate
  }

  impl ArchivedBranchesTemplate {
    pub fn build(self, output_path: &Path) -> Result<()> {
      // Initialize repo
      fs::create_dir_all(output_path)?;
      Command::new("git").args(["init", "--initial-branch=main"]).current_dir(output_path).output()?;
      Command::new("git").args(["config", "user.name", "Test User"]).current_dir(output_path).output()?;
      Command::new("git").args(["config", "user.email", "test@example.com"]).current_dir(output_path).output()?;
      Command::new("git")
        .args(["config", "branchdeck.branchPrefix", "user-name"])
        .current_dir(output_path)
        .output()?;

      // Helper closures
      let write_file = |rel: &str, content: &str| -> Result<()> {
        let full = output_path.join(rel);
        if let Some(parent) = full.parent() {
          fs::create_dir_all(parent)?;
        }
        fs::write(full, content)?;
        Ok(())
      };
      let git_add_all = || -> Result<()> {
        Command::new("git").args(["add", "."]).current_dir(output_path).output()?;
        Ok(())
      };
      let git_commit = |message: &str, ts: i64| -> Result<()> {
        let date_str = format!("{ts} +0000");
        let mut cmd = Command::new("git");
        cmd.env("GIT_AUTHOR_DATE", &date_str);
        cmd.env("GIT_COMMITTER_DATE", &date_str);
        cmd.args(["commit", "-m", message]).current_dir(output_path).output()?;
        Ok(())
      };

      // Timestamps (fixed)
      let t0 = 1704117600; // B0
      let t1 = 1704117900; // F1
      let t2 = 1704118200; // F2
      let t3 = 1704118500; // P1

      // B0: initial
      write_file("README.md", "# Test Repo for Archived Branches\n")?;
      write_file("src/app.txt", "init\n")?;
      git_add_all()?;
      git_commit("Initial commit", t0)?;

      // F1: full step 1 (integrated)
      write_file("src/full.txt", "full step 1\n")?;
      git_add_all()?;
      git_commit("full step 1", t1)?;

      // F2: full step 2 (integrated)
      write_file("src/full.txt", "full step 2\n")?;
      git_add_all()?;
      git_commit("full step 2", t2)?;

      // P1: partial step 1 (integrated only first)
      write_file("src/partial.txt", "partial step 1\n")?;
      git_add_all()?;
      git_commit("partial step 1", t3)?;

      // Add origin remote and set origin/main to HEAD (current state with all commits)
      Command::new("git").args(["remote", "add", "origin", "."]).current_dir(output_path).output()?;
      let current_head = String::from_utf8(Command::new("git").args(["rev-parse", "HEAD"]).current_dir(output_path).output()?.stdout)?;
      let current_head = current_head.trim();
      Command::new("git")
        .args(["update-ref", "refs/remotes/origin/main", current_head])
        .current_dir(output_path)
        .output()?;

      // Create archived branches with their own histories, starting from the initial commit
      let date = "2025-01-11";

      // helper: checkout a detached state at initial commit
      let co_initial = || -> Result<()> {
        // get initial
        let init = String::from_utf8(Command::new("git").args(["rev-list", "--max-parents=0", "HEAD"]).current_dir(output_path).output()?.stdout)?;
        let init = init.trim().to_string();
        Command::new("git").args(["checkout", "-f", &init]).current_dir(output_path).output()?;
        Ok(())
      };
      let create_archived_from = |branch_name: &str, steps: &[(&str, &str, &str, i64)]| -> Result<()> {
        co_initial()?;
        // create a temp branch
        let tmp = format!("tmp-{}", branch_name);
        Command::new("git").args(["checkout", "-b", &tmp]).current_dir(output_path).output()?;
        for (path, content, message, ts) in steps {
          write_file(path, content)?;
          git_add_all()?;
          git_commit(message, *ts)?;
        }
        // Use the expected archived prefix as per E2E docs and snapshots
        let full_ref = format!("user-name/archived/{date}/{branch}", branch = branch_name);
        Command::new("git").args(["branch", &full_ref, "HEAD"]).current_dir(output_path).output()?;
        // return to master
        Command::new("git").args(["checkout", "-f", "main"]).current_dir(output_path).output()?;
        // delete temp branch
        Command::new("git").args(["branch", "-D", &tmp]).current_dir(output_path).output()?;
        Ok(())
      };

      // feature-full archived (fully integrated): point archived ref to current main HEAD
      // This results in zero right-side commits relative to main
      let archived_full_ref = format!("user-name/archived/{date}/feature-full");
      Command::new("git").args(["branch", &archived_full_ref, "HEAD"]).current_dir(output_path).output()?;

      // feature-partial archived: first commit matches baseline P1, second is new
      let p1a = 1704118500; // align with P1
      let p2a = 1704118800; // new
      create_archived_from(
        "feature-partial",
        &[
          ("src/partial.txt", "partial step 1\n", "partial step 1", p1a),
          ("src/partial.txt", "partial step 2\n", "partial step 2", p2a),
        ],
      )?;

      // feature-pending archived: new content only (not on baseline)
      let n1 = 1704119100;
      create_archived_from("feature-pending", &[("src/pending.txt", "pending work\n", "pending work", n1)])?;

      Ok(())
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use tempfile::TempDir;
  use test_log::test;

  #[test]
  fn test_simple_template() {
    let temp_dir = TempDir::new().unwrap();
    let template = templates::simple();

    template.build(temp_dir.path()).unwrap();

    // Verify git repo exists
    assert!(temp_dir.path().join(".git").exists());

    // Verify files exist
    assert!(temp_dir.path().join("file1.txt").exists());
    assert!(temp_dir.path().join("file2.txt").exists());
  }
}
