use anyhow::Result;
use rand::SeedableRng;
use rand::prelude::*;
use std::fs;
use std::path::Path;
use std::process::Command;

/// Generates realistic test repositories with various commit patterns and conflict scenarios
pub struct TestRepoGenerator {
  rng: StdRng,
}

impl Default for TestRepoGenerator {
  fn default() -> Self {
    Self::new()
  }
}

impl TestRepoGenerator {
  pub fn new() -> Self {
    // Use a stable seed by default for reproducible tests
    Self::with_seed(42)
  }

  pub fn with_seed(seed: u64) -> Self {
    Self { rng: StdRng::seed_from_u64(seed) }
  }

  /// Generate a complete test repository at the specified path
  pub fn generate(&mut self, output_path: &Path) -> Result<TestRepoStats> {
    // Remove existing directory if it exists
    if output_path.exists() {
      fs::remove_dir_all(output_path)?;
    }

    // Create and initialize repository
    fs::create_dir_all(output_path)?;
    self.init_repo(output_path)?;

    // Generate initial structure
    self.create_initial_commit(output_path)?;

    // Generate various commit types
    let mut stats = TestRepoStats::default();

    // Generate feature commits with issue prefixes
    stats.issue_commits += self.generate_issue_commits(output_path)?;

    // Generate regular maintenance commits
    stats.maintenance_commits += self.generate_maintenance_commits(output_path)?;

    // Generate complex branch scenarios
    stats.conflict_branches += self.generate_conflict_scenarios(output_path)?;

    // Generate missing commit scenarios
    self.generate_missing_commit_scenarios(output_path)?;

    Ok(stats)
  }

  /// Generate a repository focused on conflict scenarios
  pub fn generate_with_conflicts(&mut self, output_path: &Path) -> Result<TestRepoStats> {
    if output_path.exists() {
      fs::remove_dir_all(output_path)?;
    }

    fs::create_dir_all(output_path)?;
    self.init_repo(output_path)?;

    self.create_initial_commit(output_path)?;

    let mut stats = TestRepoStats::default();

    // Generate more conflict scenarios
    for _ in 0..5 {
      stats.conflict_branches += self.generate_conflict_scenarios(output_path)?;

      // Add some commits between conflicts
      stats.issue_commits += self.generate_issue_commits(output_path)?;
    }

    // Multiple missing commit scenarios
    for _ in 0..3 {
      self.generate_missing_commit_scenarios(output_path)?;
    }

    Ok(stats)
  }

  /// Generate a repository focused on missing commit scenarios
  pub fn generate_with_missing_commits(&mut self, output_path: &Path) -> Result<TestRepoStats> {
    if output_path.exists() {
      fs::remove_dir_all(output_path)?;
    }

    fs::create_dir_all(output_path)?;
    self.init_repo(output_path)?;

    self.create_initial_commit(output_path)?;

    let mut stats = TestRepoStats::default();

    // Generate many missing commit scenarios
    for _ in 0..5 {
      self.generate_missing_commit_scenarios(output_path)?;
      stats.issue_commits += self.generate_issue_commits(output_path)?;
    }

    stats.maintenance_commits += self.generate_maintenance_commits(output_path)?;

    Ok(stats)
  }

  /// Generate a large repository with many commits
  pub fn generate_large(&mut self, output_path: &Path) -> Result<TestRepoStats> {
    if output_path.exists() {
      fs::remove_dir_all(output_path)?;
    }

    fs::create_dir_all(output_path)?;
    self.init_repo(output_path)?;

    self.create_initial_commit(output_path)?;

    let mut stats = TestRepoStats::default();

    // Generate many commits
    for _ in 0..10 {
      stats.issue_commits += self.generate_issue_commits(output_path)?;
      stats.maintenance_commits += self.generate_maintenance_commits(output_path)?;
    }

    // Some conflict scenarios
    stats.conflict_branches += self.generate_conflict_scenarios(output_path)?;

    Ok(stats)
  }

  fn init_repo(&self, repo_path: &Path) -> Result<()> {
    // Initialize git repository with master as default branch
    let output = Command::new("git").args(["--no-pager", "init", "-b", "master"]).current_dir(repo_path).output()?;

    if !output.status.success() {
      anyhow::bail!("Git init failed: {}", String::from_utf8_lossy(&output.stderr));
    }

    // Configure git user
    Command::new("git")
      .args(["--no-pager", "config", "user.name", "Test User"])
      .current_dir(repo_path)
      .output()?;

    Command::new("git")
      .args(["--no-pager", "config", "user.email", "test@example.com"])
      .current_dir(repo_path)
      .output()?;

    // Configure merge conflict style
    Command::new("git")
      .args(["--no-pager", "config", "merge.conflictstyle", "zdiff3"])
      .current_dir(repo_path)
      .output()?;

    Ok(())
  }

  fn create_initial_commit(&mut self, repo_path: &Path) -> Result<()> {
    // Create a proper Kotlin/Java project structure
    let files = vec![
      (
        "README.md",
        "# Test Repository\n\nGenerated test repository for development.\n\n## Build\n\n```bash\n./gradlew build\n```\n",
      ),
      (
        "build.gradle.kts",
        r#"plugins {
    kotlin("jvm") version "1.9.20"
    id("org.springframework.boot") version "3.1.5"
    id("io.spring.dependency-management") version "1.1.3"
}

group = "com.example"
version = "0.0.1-SNAPSHOT"
java.sourceCompatibility = JavaVersion.VERSION_17

repositories {
    mavenCentral()
}

dependencies {
    implementation("org.springframework.boot:spring-boot-starter-web")
    implementation("org.jetbrains.kotlin:kotlin-reflect")
    implementation("com.fasterxml.jackson.module:jackson-module-kotlin")
    testImplementation("org.springframework.boot:spring-boot-starter-test")
}

tasks.withType<org.jetbrains.kotlin.gradle.tasks.KotlinCompile> {
    kotlinOptions {
        freeCompilerArgs = listOf("-Xjsr305=strict")
        jvmTarget = "17"
    }
}
"#,
      ),
      ("settings.gradle.kts", "rootProject.name = \"test-repo\"\n"),
      (
        ".gitignore",
        r#"# Gradle
.gradle/
build/
!gradle/wrapper/gradle-wrapper.jar

# IDE
.idea/
*.iml
*.ipr
*.iws
.vscode/

# OS
.DS_Store
Thumbs.db

# Compiled class files
*.class

# Log files
*.log

# Package files
*.jar
*.war
*.nar
*.ear
*.zip
*.tar.gz
*.rar
"#,
      ),
      (
        "src/main/kotlin/com/example/Application.kt",
        r#"package com.example

import org.springframework.boot.autoconfigure.SpringBootApplication
import org.springframework.boot.runApplication

@SpringBootApplication
class Application

fun main(args: Array<String>) {
    runApplication<Application>(*args)
}
"#,
      ),
      (
        "src/main/kotlin/com/example/controller/ApiController.kt",
        r#"package com.example.controller

import org.springframework.web.bind.annotation.*

@RestController
@RequestMapping("/api")
class ApiController {
    
    @GetMapping("/health")
    fun health(): Map<String, String> {
        return mapOf("status" to "OK")
    }
}
"#,
      ),
      (
        "src/main/resources/application.yml",
        r#"spring:
  application:
    name: test-repo

server:
  port: 8080

logging:
  level:
    root: INFO
    com.example: DEBUG
"#,
      ),
    ];

    self.create_commit(repo_path, "Initial commit", &files)
  }

  fn create_commit(&self, repo_path: &Path, message: &str, files: &[(&str, &str)]) -> Result<()> {
    // Write files
    for (filename, content) in files {
      let file_path = repo_path.join(filename);
      if let Some(parent) = file_path.parent() {
        fs::create_dir_all(parent)?;
      }
      fs::write(&file_path, content)?;

      // Add to git
      Command::new("git").args(["--no-pager", "add", filename]).current_dir(repo_path).output()?;
    }

    // Write commit message to a temporary file to support multi-line messages
    let commit_msg_file = repo_path.join(".git/COMMIT_MSG");
    fs::write(&commit_msg_file, message)?;

    // Commit using -F flag
    let output = Command::new("git")
      .args(["--no-pager", "commit", "-F", ".git/COMMIT_MSG"])
      .current_dir(repo_path)
      .output()?;

    // Clean up the message file
    let _ = fs::remove_file(&commit_msg_file);

    if !output.status.success() {
      anyhow::bail!("Git commit failed: {}", String::from_utf8_lossy(&output.stderr));
    }

    Ok(())
  }

  fn create_branch(&self, repo_path: &Path, branch_name: &str) -> Result<()> {
    let output = Command::new("git").args(["--no-pager", "checkout", "-b", branch_name]).current_dir(repo_path).output()?;

    if !output.status.success() {
      anyhow::bail!("Git checkout -b failed: {}", String::from_utf8_lossy(&output.stderr));
    }

    Ok(())
  }

  fn checkout(&self, repo_path: &Path, ref_name: &str) -> Result<()> {
    let output = Command::new("git").args(["--no-pager", "checkout", ref_name]).current_dir(repo_path).output()?;

    if !output.status.success() {
      anyhow::bail!("Git checkout failed: {}", String::from_utf8_lossy(&output.stderr));
    }

    Ok(())
  }

  fn generate_issue_commits(&mut self, repo_path: &Path) -> Result<usize> {
    let mut count = 0;

    // Generate different types of commits - some with branch prefixes, some with issue numbers
    let issue_types = vec![("IDEA", 300000, 350000), ("KT", 50000, 60000), ("RIDER", 20000, 25000), ("WEB", 40000, 45000)];

    // Branch names for prefix-style commits
    let branch_names = [
      "feature-editor",
      "bugfix-completion",
      "refactor-ui",
      "perf-indexing",
      "feature-debugger",
      "bugfix-vcs",
      "feature-gradle",
      "tests-platform",
      "docs-api",
      "feature-kotlin",
      "bugfix-java",
      "feature-run-configs",
    ];

    for (prefix, min_id, max_id) in issue_types {
      let num_commits = self.rng.random_range(3..8);

      for i in 0..num_commits {
        let issue_id = self.rng.random_range(min_id..max_id);
        let component = self.random_component();
        let action = self.random_action();
        let description = self.random_description();

        // Mix different commit message styles
        let message = match i % 3 {
          0 => {
            // Branch prefix style: (branch-name) message
            let branch = branch_names.choose(&mut self.rng).unwrap();
            format!("({branch}) {component}: {description}")
          }
          1 => {
            // Issue number at start: IDEA-123456 message
            format!("{prefix}-{issue_id} {component}: {description}")
          }
          _ => {
            // Parentheses with issue number (older style)
            format!("({prefix}-{issue_id}) {component}: {description}")
          }
        };

        let file_path = self.generate_file_path(component);
        let content = self.generate_file_content(component, action);

        // Generate multi-line message
        let full_message = self.generate_commit_message(&message);

        self.create_commit(repo_path, &full_message, &[(&file_path, &content)])?;
        count += 1;
      }
    }

    // Add some commits with just branch prefixes (no issue numbers)
    for _ in 0..5 {
      let branch = branch_names.choose(&mut self.rng).unwrap();
      let component = self.random_component();
      let action = self.random_action();
      let description = self.random_description();

      let message = format!("({branch}) {component}: {description}");
      let file_path = self.generate_file_path(component);
      let content = self.generate_file_content(component, action);

      // Generate multi-line message
      let full_message = self.generate_commit_message(&message);

      self.create_commit(repo_path, &full_message, &[(&file_path, &content)])?;
      count += 1;
    }

    Ok(count)
  }

  fn generate_maintenance_commits(&mut self, repo_path: &Path) -> Result<usize> {
    let mut count = 0;

    let maintenance_types = vec![
      ("Dependencies", "Update {} to version {}", vec!["kotlin", "gradle", "junit", "mockito"]),
      ("Cleanup", "Cleanup: {}", vec!["remove deprecated API usages", "fix compiler warnings", "optimize imports"]),
      ("Build", "Build: {}", vec!["migrate to JDK 17", "update gradle wrapper", "fix build on Windows"]),
      ("Tests", "Tests: {}", vec!["fix flaky tests", "add missing test coverage", "improve test performance"]),
      ("Docs", "DOC: {}", vec!["update API documentation", "fix typos in comments", "add usage examples"]),
    ];

    for (category, template, options) in maintenance_types {
      let num_commits = self.rng.random_range(1..3);

      for _ in 0..num_commits {
        let option = options.choose(&mut self.rng).unwrap();
        let message = if category == "Dependencies" {
          let version = format!("{}.{}.{}", self.rng.random_range(1..10), self.rng.random_range(0..20), self.rng.random_range(0..50));
          template.replace("{}", option).replace("{}", &version)
        } else {
          template.replace("{}", option)
        };

        let file_path = self.generate_maintenance_file_path(category);
        let content = self.generate_maintenance_content(category, option, &file_path);

        // Generate multi-line message
        let full_message = self.generate_commit_message(&message);

        self.create_commit(repo_path, &full_message, &[(&file_path, &content)])?;
        count += 1;
      }
    }

    Ok(count)
  }

  fn generate_conflict_scenarios(&mut self, repo_path: &Path) -> Result<usize> {
    let mut count = 0;

    // Scenario 1: Different implementations of user authentication
    let auth_file = "src/main/kotlin/com/example/service/AuthService.kt";
    let base_content = r#"package com.example.service

import org.springframework.stereotype.Service
import java.util.UUID

@Service
class AuthService {
    
    fun authenticate(username: String, password: String): AuthResult {
        // Simple authentication
        if (username == "admin" && password == "admin") {
            return AuthResult.Success(UUID.randomUUID())
        }
        return AuthResult.Failure("Invalid credentials")
    }
}

sealed class AuthResult {
    data class Success(val token: UUID) : AuthResult()
    data class Failure(val message: String) : AuthResult()
}"#;

    self.create_commit(repo_path, "Add base AuthService", &[(auth_file, base_content)])?;

    // Branch 1: Add JWT token support
    self.create_branch(repo_path, "feature-jwt-auth")?;

    let jwt_content = r#"package com.example.service

import org.springframework.stereotype.Service
import io.jsonwebtoken.Jwts
import io.jsonwebtoken.SignatureAlgorithm
import io.jsonwebtoken.security.Keys
import java.util.UUID
import java.util.Date
import javax.crypto.SecretKey

@Service
class AuthService {
    private val secretKey: SecretKey = Keys.secretKeyFor(SignatureAlgorithm.HS256)
    private val tokenExpiry = 3600000L // 1 hour
    
    fun authenticate(username: String, password: String): AuthResult {
        // JWT-based authentication
        if (validateCredentials(username, password)) {
            val token = generateJwtToken(username)
            return AuthResult.Success(token)
        }
        return AuthResult.Failure("Invalid credentials")
    }
    
    private fun validateCredentials(username: String, password: String): Boolean {
        // In real app, check against database
        return username == "admin" && password == "admin"
    }
    
    private fun generateJwtToken(username: String): String {
        val now = Date()
        val expiry = Date(now.time + tokenExpiry)
        
        return Jwts.builder()
            .setSubject(username)
            .setIssuedAt(now)
            .setExpiration(expiry)
            .signWith(secretKey)
            .compact()
    }
    
    fun validateToken(token: String): Boolean {
        return try {
            Jwts.parserBuilder()
                .setSigningKey(secretKey)
                .build()
                .parseClaimsJws(token)
            true
        } catch (e: Exception) {
            false
        }
    }
}

sealed class AuthResult {
    data class Success(val token: String) : AuthResult()
    data class Failure(val message: String) : AuthResult()
}"#;

    let jwt_message = self.generate_commit_message("(feature-jwt-auth) Add JWT token authentication");
    self.create_commit(repo_path, &jwt_message, &[(auth_file, jwt_content)])?;

    // Branch 2: Add OAuth2 support (from master, without JWT changes)
    self.checkout(repo_path, "master")?;
    self.create_branch(repo_path, "feature-oauth2")?;

    let oauth2_content = r#"package com.example.service

import org.springframework.stereotype.Service
import org.springframework.security.oauth2.client.OAuth2AuthorizedClient
import org.springframework.security.oauth2.client.OAuth2AuthorizedClientService
import org.springframework.security.oauth2.core.OAuth2AccessToken
import java.util.UUID

@Service
class AuthService(
    private val authorizedClientService: OAuth2AuthorizedClientService
) {
    
    fun authenticate(username: String, password: String): AuthResult {
        // OAuth2 authentication flow
        return try {
            val client = performOAuth2Flow(username, password)
            val accessToken = client.accessToken
            AuthResult.Success(accessToken.tokenValue, accessToken.expiresAt)
        } catch (e: Exception) {
            AuthResult.Failure("OAuth2 authentication failed: ${e.message}")
        }
    }
    
    private fun performOAuth2Flow(username: String, password: String): OAuth2AuthorizedClient {
        // Simulate OAuth2 flow - in real app, this would involve redirects
        throw UnsupportedOperationException("OAuth2 flow not implemented")
    }
    
    fun refreshToken(refreshToken: String): AuthResult {
        // OAuth2 token refresh logic
        return AuthResult.Failure("Token refresh not implemented")
    }
    
    fun revokeToken(token: String): Boolean {
        // OAuth2 token revocation
        return false
    }
}

sealed class AuthResult {
    data class Success(val token: String, val expiresAt: java.time.Instant?) : AuthResult()
    data class Failure(val message: String) : AuthResult()
}"#;

    let oauth2_message = self.generate_commit_message("(feature-oauth2) Add OAuth2 authentication support");
    self.create_commit(repo_path, &oauth2_message, &[(auth_file, oauth2_content)])?;

    // Conflict scenario created: feature-jwt-auth vs feature-oauth2
    count += 1;

    // Return to master
    self.checkout(repo_path, "master")?;

    // Scenario 2: Different caching implementations
    let cache_file = "src/main/kotlin/com/example/cache/CacheManager.kt";
    let base_cache = r#"package com.example.cache

import org.springframework.stereotype.Component

@Component
class CacheManager {
    private val cache = mutableMapOf<String, Any>()
    
    fun get(key: String): Any? = cache[key]
    
    fun put(key: String, value: Any) {
        cache[key] = value
    }
    
    fun remove(key: String) {
        cache.remove(key)
    }
}"#;

    self.create_commit(repo_path, "Add basic CacheManager", &[(cache_file, base_cache)])?;

    // Branch 1: Add Redis support
    self.create_branch(repo_path, "feature-redis-cache")?;

    let redis_cache = r#"package com.example.cache

import org.springframework.stereotype.Component
import org.springframework.data.redis.core.RedisTemplate
import java.time.Duration

@Component
class CacheManager(
    private val redisTemplate: RedisTemplate<String, Any>
) {
    private val defaultTtl = Duration.ofMinutes(30)
    
    fun get(key: String): Any? {
        return redisTemplate.opsForValue().get(key)
    }
    
    fun put(key: String, value: Any, ttl: Duration = defaultTtl) {
        redisTemplate.opsForValue().set(key, value, ttl)
    }
    
    fun remove(key: String) {
        redisTemplate.delete(key)
    }
    
    fun clear() {
        redisTemplate.connectionFactory?.connection?.flushAll()
    }
    
    fun exists(key: String): Boolean {
        return redisTemplate.hasKey(key) ?: false
    }
}"#;

    self.create_commit(repo_path, "(feature-redis-cache) Implement Redis-based caching", &[(cache_file, redis_cache)])?;

    // Branch 2: Add Caffeine cache (from master)
    self.checkout(repo_path, "master")?;
    self.create_branch(repo_path, "feature-caffeine-cache")?;

    let caffeine_cache = r#"package com.example.cache

import org.springframework.stereotype.Component
import com.github.benmanes.caffeine.cache.Caffeine
import com.github.benmanes.caffeine.cache.Cache
import java.time.Duration

@Component
class CacheManager {
    private val cache: Cache<String, Any> = Caffeine.newBuilder()
        .maximumSize(10_000)
        .expireAfterWrite(Duration.ofMinutes(30))
        .recordStats()
        .build()
    
    fun get(key: String): Any? = cache.getIfPresent(key)
    
    fun put(key: String, value: Any) {
        cache.put(key, value)
    }
    
    fun remove(key: String) {
        cache.invalidate(key)
    }
    
    fun clear() {
        cache.invalidateAll()
    }
    
    fun getStats() = cache.stats()
}"#;

    self.create_commit(repo_path, "(feature-caffeine-cache) Implement Caffeine in-memory cache", &[(cache_file, caffeine_cache)])?;

    count += 1;

    // Return to master
    self.checkout(repo_path, "master")?;

    Ok(count)
  }

  fn generate_missing_commit_scenarios(&mut self, repo_path: &Path) -> Result<()> {
    // Create a feature branch with multiple commits for user management
    self.create_branch(repo_path, "feature-user-management")?;

    // Commit 1: Add User entity
    let user_entity = r#"package com.example.model

import jakarta.persistence.*
import java.util.UUID
import java.time.LocalDateTime

@Entity
@Table(name = "users")
data class User(
    @Id
    val id: UUID = UUID.randomUUID(),
    
    @Column(unique = true, nullable = false)
    val email: String,
    
    @Column(nullable = false)
    val username: String,
    
    @Column(nullable = false)
    val passwordHash: String,
    
    @Enumerated(EnumType.STRING)
    val role: UserRole = UserRole.USER,
    
    val createdAt: LocalDateTime = LocalDateTime.now(),
    
    var updatedAt: LocalDateTime = LocalDateTime.now()
)

enum class UserRole {
    USER, ADMIN, MODERATOR
}"#;

    self.create_commit(
      repo_path,
      "(feature-user-management) Add User entity",
      &[("src/main/kotlin/com/example/model/User.kt", user_entity)],
    )?;

    // Commit 2: Add UserRepository
    let user_repo = r#"package com.example.repository

import com.example.model.User
import org.springframework.data.jpa.repository.JpaRepository
import org.springframework.stereotype.Repository
import java.util.UUID
import java.util.Optional

@Repository
interface UserRepository : JpaRepository<User, UUID> {
    fun findByEmail(email: String): Optional<User>
    fun findByUsername(username: String): Optional<User>
    fun existsByEmail(email: String): Boolean
    fun existsByUsername(username: String): Boolean
}"#;

    self.create_commit(
      repo_path,
      "(feature-user-management) Add UserRepository",
      &[("src/main/kotlin/com/example/repository/UserRepository.kt", user_repo)],
    )?;

    // Commit 3: Add UserService
    let user_service = r#"package com.example.service

import com.example.model.User
import com.example.model.UserRole
import com.example.repository.UserRepository
import org.springframework.security.crypto.password.PasswordEncoder
import org.springframework.stereotype.Service
import org.springframework.transaction.annotation.Transactional
import java.util.UUID
import java.time.LocalDateTime

@Service
@Transactional
class UserService(
    private val userRepository: UserRepository,
    private val passwordEncoder: PasswordEncoder
) {
    
    fun createUser(email: String, username: String, password: String): User {
        if (userRepository.existsByEmail(email)) {
            throw IllegalArgumentException("Email already exists")
        }
        if (userRepository.existsByUsername(username)) {
            throw IllegalArgumentException("Username already exists")
        }
        
        val user = User(
            email = email,
            username = username,
            passwordHash = passwordEncoder.encode(password)
        )
        
        return userRepository.save(user)
    }
    
    fun findByEmail(email: String): User? {
        return userRepository.findByEmail(email).orElse(null)
    }
    
    fun updateUserRole(userId: UUID, newRole: UserRole): User {
        val user = userRepository.findById(userId)
            .orElseThrow { NoSuchElementException("User not found") }
        
        user.role = newRole
        user.updatedAt = LocalDateTime.now()
        
        return userRepository.save(user)
    }
}"#;

    let user_service_message = self.generate_commit_message("(feature-user-management) Add UserService");
    self.create_commit(repo_path, &user_service_message, &[("src/main/kotlin/com/example/service/UserService.kt", user_service)])?;

    // Commit 4: Add UserController
    let user_controller = r#"package com.example.controller

import com.example.model.User
import com.example.service.UserService
import org.springframework.web.bind.annotation.*
import org.springframework.http.HttpStatus
import java.util.UUID

@RestController
@RequestMapping("/api/users")
class UserController(
    private val userService: UserService
) {
    
    @PostMapping
    @ResponseStatus(HttpStatus.CREATED)
    fun createUser(@RequestBody request: CreateUserRequest): UserResponse {
        val user = userService.createUser(
            email = request.email,
            username = request.username,
            password = request.password
        )
        return UserResponse.from(user)
    }
    
    @GetMapping("/email/{email}")
    fun getUserByEmail(@PathVariable email: String): UserResponse? {
        return userService.findByEmail(email)?.let { UserResponse.from(it) }
    }
}

data class CreateUserRequest(
    val email: String,
    val username: String,
    val password: String
)

data class UserResponse(
    val id: UUID,
    val email: String,
    val username: String,
    val role: String
) {
    companion object {
        fun from(user: User) = UserResponse(
            id = user.id,
            email = user.email,
            username = user.username,
            role = user.role.name
        )
    }
}"#;

    let user_controller_message = self.generate_commit_message("(feature-user-management) Add UserController");
    self.create_commit(
      repo_path,
      &user_controller_message,
      &[("src/main/kotlin/com/example/controller/UserController.kt", user_controller)],
    )?;

    // Create another branch from master (missing all 4 commits)
    self.checkout(repo_path, "master")?;
    self.create_branch(repo_path, "feature-auth-only")?;

    // Make conflicting changes - different User model structure
    let conflicting_user = r#"package com.example.model

import java.time.Instant

// Simplified user model for auth only
data class User(
    val id: String,  // Using String ID instead of UUID
    val email: String,
    val passwordHash: String,
    val isActive: Boolean = true,
    val lastLogin: Instant? = null
)"#;

    self.create_commit(
      repo_path,
      "(feature-auth-only) Add simplified User model",
      &[("src/main/kotlin/com/example/model/User.kt", conflicting_user)],
    )?;

    self.checkout(repo_path, "master")?;

    Ok(())
  }

  // Helper method to generate multi-line commit messages
  fn generate_commit_message(&mut self, base_message: &str) -> String {
    // Randomly decide message type
    let message_type = self.rng.random_range(0..10);

    match message_type {
      0..=3 => {
        // 40% - Single line message (keep existing behavior)
        base_message.to_string()
      }
      4..=6 => {
        // 30% - Small multi-line message (2-3 paragraphs)
        self.generate_small_multiline_message(base_message)
      }
      7..=8 => {
        // 20% - Medium multi-line message (4-6 paragraphs)
        self.generate_medium_multiline_message(base_message)
      }
      _ => {
        // 10% - Large multi-line message (7-10 paragraphs)
        self.generate_large_multiline_message(base_message)
      }
    }
  }

  fn generate_small_multiline_message(&mut self, subject: &str) -> String {
    let explanations = [
      "This change was necessary to improve the overall system performance.",
      "The previous implementation had several issues that needed to be addressed.",
      "This commit introduces a more robust solution to the problem.",
      "The refactoring helps maintain better code organization.",
      "This update ensures compatibility with the latest framework version.",
    ];

    let details = [
      "- Fixed memory leak in the background processing\n- Improved error handling\n- Added proper resource cleanup",
      "- Optimized database queries\n- Reduced unnecessary object allocations\n- Improved caching strategy",
      "- Updated deprecated API calls\n- Improved code readability\n- Added missing documentation",
    ];

    let explanation = explanations.choose(&mut self.rng).unwrap();
    let detail = details.choose(&mut self.rng).unwrap();

    format!("{subject}\n\n{explanation}\n\n{detail}")
  }

  fn generate_medium_multiline_message(&mut self, subject: &str) -> String {
    let problem_descriptions = [
      "The application was experiencing intermittent crashes when processing large datasets. Investigation revealed that the root cause was improper memory management in the data processing pipeline.",
      "Users reported slow response times during peak hours. Performance profiling showed that the database connection pool was exhausted due to connection leaks.",
      "The authentication system was vulnerable to timing attacks. Security audit revealed that the password comparison was not using constant-time comparison.",
    ];

    let solution_descriptions = [
      "Implemented a streaming approach to process data in chunks rather than loading everything into memory. Added proper resource management with try-with-resources blocks.",
      "Refactored the database access layer to ensure connections are properly released. Implemented connection pooling with proper timeout configurations.",
      "Replaced the string comparison with a constant-time comparison algorithm. Added additional security measures including rate limiting.",
    ];

    let testing_notes = [
      "Tested with datasets up to 10GB without any memory issues. Performance benchmarks show 3x improvement in processing speed.",
      "Load tested with 1000 concurrent users. Connection pool metrics show stable behavior with no leaks detected over 24-hour test period.",
      "Security tests confirm the timing attack vector has been eliminated. Penetration testing validates the improved security posture.",
    ];

    let problem = problem_descriptions.choose(&mut self.rng).unwrap();
    let solution = solution_descriptions.choose(&mut self.rng).unwrap();
    let testing = testing_notes.choose(&mut self.rng).unwrap();

    format!(
      "{subject}\n\n## Problem\n\n{problem}\n\n## Solution\n\n{solution}\n\n## Testing\n\n{testing}\n\n## Additional Notes\n\nThis change is backward compatible and requires no migration."
    )
  }

  fn generate_large_multiline_message(&mut self, subject: &str) -> String {
    let overview = "This commit represents a significant refactoring of the core system architecture. The changes were necessary to address several long-standing technical debt issues and to prepare the codebase for future scalability requirements.";

    let background = "The original implementation was designed for a much smaller scale of operations. As the system grew, several architectural decisions became bottlenecks:\n\n1. Synchronous processing model limited throughput\n2. Tight coupling between components made testing difficult\n3. Lack of proper abstraction layers hindered feature development\n4. Monolithic structure prevented independent scaling of components";

    let changes = "## Key Changes\n\n### 1. Asynchronous Processing\n- Migrated from blocking I/O to non-blocking async operations\n- Implemented event-driven architecture using message queues\n- Added circuit breakers for external service calls\n\n### 2. Dependency Injection\n- Introduced DI container for better testability\n- Decoupled components using interfaces\n- Simplified configuration management\n\n### 3. Modular Architecture\n- Split monolith into logical modules\n- Defined clear API boundaries between modules\n- Implemented module-level testing strategies";

    let performance = "## Performance Improvements\n\n- Request throughput: 500 req/s → 5000 req/s (10x improvement)\n- Average latency: 200ms → 50ms (75% reduction)\n- Memory usage: 4GB → 2GB (50% reduction)\n- Startup time: 30s → 5s (83% reduction)";

    let migration = "## Migration Guide\n\n1. Update configuration files to use new format\n2. Run database migration script: `./migrate.sh`\n3. Update client libraries to v2.0\n4. Review and update any custom integrations";

    let future_work = "## Future Work\n\n- Implement distributed tracing for better observability\n- Add support for horizontal scaling\n- Introduce feature flags for gradual rollout\n- Implement automated performance regression testing";

    let acknowledgments = "## Acknowledgments\n\nThanks to the entire team for their contributions to this major refactoring effort. Special thanks to the QA team for extensive testing and the DevOps team for helping with the deployment strategy.";

    format!(
      "{subject}\n\n{overview}\n\n## Background\n\n{background}\n\n{changes}\n\n{performance}\n\n{migration}\n\n{future_work}\n\n{acknowledgments}\n\nReviewed-by: Team Lead\nTested-by: QA Team\nApproved-by: Architecture Board"
    )
  }

  // Helper methods for generating random data
  fn random_component(&mut self) -> &'static str {
    let components = vec![
      "Controller",
      "Service",
      "Repository",
      "Model",
      "Editor",
      "Debugger",
      "VCS",
      "Indexing",
      "UI",
      "Auth",
      "Cache",
      "Validation",
      "Configuration",
      "Security",
      "Integration",
    ];
    components.choose(&mut self.rng).unwrap()
  }

  fn random_action(&mut self) -> &'static str {
    let actions = [
      "fix",
      "improve",
      "optimize",
      "implement",
      "add support for",
      "enhance",
      "refactor",
      "update",
      "resolve",
      "handle",
    ];
    actions.choose(&mut self.rng).unwrap()
  }

  fn random_description(&mut self) -> String {
    let descriptions = [
      "memory leak in background tasks",
      "performance regression in large projects",
      "incorrect handling of edge cases",
      "UI freeze on certain operations",
      "compatibility with new framework version",
      "async operation cancellation",
      "proper error reporting",
      "resource cleanup on shutdown",
      "concurrent modification detection",
      "cache invalidation logic",
    ];

    let base = descriptions.choose(&mut self.rng).unwrap();
    format!("{} {}", self.random_action(), base)
  }

  fn generate_file_path(&mut self, component: &str) -> String {
    let is_kotlin = self.rng.random::<bool>();
    let extension = if is_kotlin { "kt" } else { "java" };

    let base_path = match component {
      "Editor" => "src/main/kotlin/com/example/editor",
      "Debugger" => "src/main/kotlin/com/example/debugger",
      "VCS" => "src/main/kotlin/com/example/vcs",
      "Indexing" => "src/main/kotlin/com/example/indexing",
      "UI" => "src/main/kotlin/com/example/ui",
      "Controller" => "src/main/kotlin/com/example/controller",
      "Service" => "src/main/kotlin/com/example/service",
      "Repository" => "src/main/kotlin/com/example/repository",
      "Model" => "src/main/kotlin/com/example/model",
      _ => "src/main/kotlin/com/example/core",
    };

    let file_name = format!("{}.{}", self.random_class_name(), extension);
    format!("{base_path}/{file_name}")
  }

  fn random_class_name(&mut self) -> String {
    let prefixes = ["Default", "Abstract", "Base", "Simple", "Advanced"];
    let middles = ["Data", "Service", "Manager", "Handler", "Processor"];
    let suffixes = ["Impl", "Provider", "Factory", "Builder", "Helper"];

    let prefix = prefixes.choose(&mut self.rng).unwrap();
    let middle = middles.choose(&mut self.rng).unwrap();
    let suffix = suffixes.choose(&mut self.rng).unwrap();

    format!("{prefix}{middle}{suffix}")
  }

  fn generate_file_content(&mut self, component: &str, action: &str) -> String {
    let class_name = self.random_class_name();
    let is_kotlin = self.rng.random::<bool>();

    if is_kotlin {
      self.generate_kotlin_content(component, action, &class_name)
    } else {
      self.generate_java_content(component, action, &class_name)
    }
  }

  fn generate_kotlin_content(&mut self, component: &str, action: &str, class_name: &str) -> String {
    let package_name = match component {
      "Controller" => "com.example.controller",
      "Service" => "com.example.service",
      "Repository" => "com.example.repository",
      "Model" => "com.example.model",
      _ => "com.example.core",
    };

    match component {
      "Controller" => format!(
        r#"package {}

import org.springframework.web.bind.annotation.*
import org.springframework.http.ResponseEntity
import java.util.UUID

@RestController
@RequestMapping("/api/{}")
class {} @Autowired constructor(
    private val service: {}Service
) {{
    
    @GetMapping("/{{id}}")
    fun getById(@PathVariable id: UUID): ResponseEntity<{}> {{
        return service.findById(id)?.let {{
            ResponseEntity.ok(it)
        }} ?: ResponseEntity.notFound().build()
    }}
    
    @PostMapping
    fun create(@RequestBody request: {}Request): ResponseEntity<{}> {{
        val result = service.create(request)
        return ResponseEntity.ok(result)
    }}
    
    @PutMapping("/{{id}}")
    fun update(
        @PathVariable id: UUID,
        @RequestBody request: {}Request
    ): ResponseEntity<{}> {{
        return service.update(id, request)?.let {{
            ResponseEntity.ok(it)
        }} ?: ResponseEntity.notFound().build()
    }}
    
    @DeleteMapping("/{{id}}")
    fun delete(@PathVariable id: UUID): ResponseEntity<Void> {{
        service.delete(id)
        return ResponseEntity.noContent().build()
    }}
}}

data class {}Request(
    val name: String,
    val description: String? = null,
    val enabled: Boolean = true
)"#,
        package_name,
        component.to_lowercase(),
        class_name,
        class_name.trim_end_matches("Controller"),
        class_name.trim_end_matches("Controller"),
        class_name.trim_end_matches("Controller"),
        class_name.trim_end_matches("Controller"),
        class_name.trim_end_matches("Controller"),
        class_name.trim_end_matches("Controller"),
        class_name.trim_end_matches("Controller")
      ),

      "Service" => format!(
        r#"package {}

import org.springframework.stereotype.Service
import org.springframework.transaction.annotation.Transactional
import java.util.UUID
import java.time.LocalDateTime
import org.slf4j.LoggerFactory

@Service
@Transactional
class {} @Autowired constructor(
    private val repository: {}Repository
) {{
    private val logger = LoggerFactory.getLogger(javaClass)
    
    fun findById(id: UUID): {}? {{
        logger.debug("Finding entity by id: $id")
        return repository.findById(id).orElse(null)
    }}
    
    fun create(request: {}Request): {} {{
        logger.info("Creating new entity: ${{request.name}}")
        val entity = {}(
            id = UUID.randomUUID(),
            name = request.name,
            description = request.description,
            enabled = request.enabled,
            createdAt = LocalDateTime.now(),
            updatedAt = LocalDateTime.now()
        )
        return repository.save(entity)
    }}
    
    fun update(id: UUID, request: {}Request): {}? {{
        return findById(id)?.let {{ existing ->
            logger.info("Updating entity: $id")
            val updated = existing.copy(
                name = request.name,
                description = request.description,
                enabled = request.enabled,
                updatedAt = LocalDateTime.now()
            )
            repository.save(updated)
        }}
    }}
    
    fun delete(id: UUID) {{
        logger.warn("Deleting entity: $id")
        repository.deleteById(id)
    }}
    
    fun findAll(): List<{}> {{
        return repository.findAll()
    }}
}}"#,
        package_name,
        class_name,
        class_name.trim_end_matches("Service"),
        class_name.trim_end_matches("Service"),
        class_name.trim_end_matches("Service"),
        class_name.trim_end_matches("Service"),
        class_name.trim_end_matches("Service"),
        class_name.trim_end_matches("Service"),
        class_name.trim_end_matches("Service"),
        class_name.trim_end_matches("Service")
      ),

      _ => format!(
        r#"package {}

import org.springframework.stereotype.Component
import java.util.UUID
import java.time.LocalDateTime

/**
 * {} implementation for {}
 * Action: {}
 */
@Component
class {} {{
    
    fun execute(input: String): Result<String> {{
        return try {{
            // Process the input
            val processed = processInput(input)
            
            // Validate the result
            validateResult(processed)
            
            Result.success(processed)
        }} catch (e: Exception) {{
            Result.failure(e)
        }}
    }}
    
    private fun processInput(input: String): String {{
        // {} logic here
        return input.trim()
            .lowercase()
            .replace(" ", "-")
            .take(50)
    }}
    
    private fun validateResult(result: String) {{
        require(result.isNotBlank()) {{ "Result cannot be blank" }}
        require(result.length <= 50) {{ "Result exceeds maximum length" }}
        require(result.matches(Regex("[a-z0-9-]+"))) {{ 
            "Result contains invalid characters" 
        }}
    }}
}}"#,
        package_name,
        self.random_action(),
        component,
        action,
        class_name,
        action
      ),
    }
  }

  fn generate_java_content(&mut self, component: &str, action: &str, class_name: &str) -> String {
    let package_name = match component {
      "Controller" => "com.example.controller",
      "Service" => "com.example.service",
      "Repository" => "com.example.repository",
      _ => "com.example.core",
    };

    format!(
      r#"package {};

import org.springframework.stereotype.Component;
import java.util.*;
import java.time.LocalDateTime;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

/**
 * {} implementation for {}
 * Generated for: {}
 */
@Component
public class {} {{
    
    private static final Logger logger = LoggerFactory.getLogger({}.class);
    
    private final Map<String, Object> cache = new HashMap<>();
    
    public String execute(String input) {{
        logger.debug("Executing with input: {{}}", input);
        
        try {{
            // Check cache
            if (cache.containsKey(input)) {{
                logger.debug("Cache hit for: {{}}", input);
                return (String) cache.get(input);
            }}
            
            // Process the input
            String result = processInput(input);
            
            // Store in cache
            cache.put(input, result);
            
            return result;
        }} catch (Exception e) {{
            logger.error("Error processing input", e);
            throw new RuntimeException("Processing failed", e);
        }}
    }}
    
    private String processInput(String input) {{
        // {} logic implementation
        StringBuilder result = new StringBuilder();
        
        for (char c : input.toCharArray()) {{
            if (Character.isLetterOrDigit(c) || c == '-') {{
                result.append(Character.toLowerCase(c));
            }} else if (c == ' ') {{
                result.append('-');
            }}
        }}
        
        return result.toString();
    }}
    
    public void clearCache() {{
        logger.info("Clearing cache with {{}} entries", cache.size());
        cache.clear();
    }}
}}"#,
      package_name,
      self.random_action(),
      component,
      action,
      class_name,
      class_name,
      action
    )
  }

  fn generate_maintenance_file_path(&mut self, category: &str) -> String {
    match category {
      "Dependencies" => "build.gradle.kts".to_string(),
      "Build" => "gradle.properties".to_string(),
      "Tests" => {
        let is_kotlin = self.rng.random::<bool>();
        let extension = if is_kotlin { "kt" } else { "java" };
        format!("src/test/kotlin/com/example/{}.{}", self.random_class_name(), extension)
      }
      "Docs" => "README.md".to_string(),
      _ => {
        let is_kotlin = self.rng.random::<bool>();
        let extension = if is_kotlin { "kt" } else { "java" };
        format!("src/main/kotlin/com/example/{}.{}", self.random_class_name(), extension)
      }
    }
  }

  fn generate_maintenance_content(&mut self, category: &str, option: &str, file_path: &str) -> String {
    match category {
      "Dependencies" => {
        // Read existing build.gradle.kts if it exists, or create a new one
        format!(
          r#"plugins {{
    kotlin("jvm") version "1.9.20"
    id("org.springframework.boot") version "3.1.5"
    id("io.spring.dependency-management") version "1.1.3"
}}

group = "com.example"
version = "0.0.1-SNAPSHOT"
java.sourceCompatibility = JavaVersion.VERSION_17

repositories {{
    mavenCentral()
}}

dependencies {{
    implementation("org.springframework.boot:spring-boot-starter-web")
    implementation("org.springframework.boot:spring-boot-starter-data-jpa")
    implementation("org.jetbrains.kotlin:kotlin-reflect")
    implementation("com.fasterxml.jackson.module:jackson-module-kotlin")
    
    // Updated: {option}
    implementation("org.jetbrains.kotlin:kotlin-stdlib:1.9.20")
    
    // Security
    implementation("org.springframework.boot:spring-boot-starter-security")
    implementation("io.jsonwebtoken:jjwt-api:0.12.3")
    runtimeOnly("io.jsonwebtoken:jjwt-impl:0.12.3")
    runtimeOnly("io.jsonwebtoken:jjwt-jackson:0.12.3")
    
    // Caching
    implementation("com.github.ben-manes.caffeine:caffeine:3.1.8")
    implementation("org.springframework.boot:spring-boot-starter-data-redis")
    
    // Testing
    testImplementation("org.springframework.boot:spring-boot-starter-test")
    testImplementation("io.mockk:mockk:1.13.8")
    testImplementation("com.ninja-squad:springmockk:4.0.2")
}}

tasks.withType<org.jetbrains.kotlin.gradle.tasks.KotlinCompile> {{
    kotlinOptions {{
        freeCompilerArgs = listOf("-Xjsr305=strict")
        jvmTarget = "17"
    }}
}}"#
        )
      }

      "Build" => {
        format!(
          r#"# Gradle properties
org.gradle.jvmargs=-Xmx2048m -XX:MaxPermSize=512m -XX:+HeapDumpOnOutOfMemoryError -Dfile.encoding=UTF-8
org.gradle.parallel=true
org.gradle.caching=true

# Kotlin
kotlin.code.style=official
kotlin.incremental=true

# Version properties
springBootVersion=3.1.5
kotlinVersion=1.9.20

# {option} configuration
systemProp.file.encoding=UTF-8
systemProp.user.timezone=UTC"#
        )
      }

      "Tests" => {
        if file_path.ends_with(".kt") {
          format!(
            r#"package com.example

import io.mockk.*
import org.junit.jupiter.api.Test
import org.junit.jupiter.api.BeforeEach
import org.junit.jupiter.api.Assertions.*
import org.springframework.boot.test.context.SpringBootTest

@SpringBootTest
class {} {{
    
    private val mockService = mockk<Service>()
    
    @BeforeEach
    fun setup() {{
        clearAllMocks()
    }}
    
    @Test
    fun `test {} scenario`() {{
        // Given
        every {{ mockService.process(any()) }} returns "expected"
        
        // When
        val result = mockService.process("input")
        
        // Then
        assertEquals("expected", result)
        verify(exactly = 1) {{ mockService.process("input") }}
    }}
    
    @Test
    fun `test error handling`() {{
        // Given
        every {{ mockService.process(any()) }} throws RuntimeException("Test error")
        
        // When/Then
        assertThrows(RuntimeException::class.java) {{
            mockService.process("input")
        }}
    }}
}}"#,
            self.random_class_name(),
            option
          )
        } else {
          format!(
            r#"package com.example;

import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.BeforeEach;
import org.mockito.Mock;
import org.mockito.MockitoAnnotations;
import static org.junit.jupiter.api.Assertions.*;
import static org.mockito.Mockito.*;

public class {} {{
    
    @Mock
    private Service mockService;
    
    @BeforeEach
    public void setup() {{
        MockitoAnnotations.openMocks(this);
    }}
    
    @Test
    public void test{}Scenario() {{
        // Given
        when(mockService.process(anyString())).thenReturn("expected");
        
        // When
        String result = mockService.process("input");
        
        // Then
        assertEquals("expected", result);
        verify(mockService, times(1)).process("input");
    }}
}}"#,
            self.random_class_name(),
            option.replace(" ", "_")
          )
        }
      }

      "Docs" => {
        format!(
          r#"# Test Repository

Generated test repository for development.

## Build

```bash
./gradlew build
```

## Run

```bash
./gradlew bootRun
```

## Test

```bash
./gradlew test
```

## Recent Changes

- {option}

## Architecture

This is a Spring Boot application written in Kotlin/Java with:
- Spring Web for REST APIs
- Spring Data JPA for persistence
- Spring Security for authentication
- JWT tokens for stateless auth
- Redis/Caffeine for caching

## API Endpoints

- `GET /api/health` - Health check
- `POST /api/users` - Create user
- `GET /api/users/email/{{email}}` - Get user by email
"#
        )
      }

      _ => {
        // Cleanup category - generate actual code cleanup
        if file_path.ends_with(".kt") {
          format!(
            r#"package com.example.util

import org.springframework.stereotype.Component
import org.slf4j.LoggerFactory

/**
 * Utility class for {option}
 */
@Component
class CleanupUtil {{
    private val logger = LoggerFactory.getLogger(javaClass)
    
    fun performCleanup() {{
        logger.info("Performing cleanup: {option}")
        
        // Remove deprecated code
        cleanupDeprecatedApis()
        
        // Optimize imports
        optimizeImports()
        
        // Fix warnings
        fixCompilerWarnings()
    }}
    
    private fun cleanupDeprecatedApis() {{
        // Implementation for removing deprecated API usage
    }}
    
    private fun optimizeImports() {{
        // Implementation for import optimization
    }}
    
    private fun fixCompilerWarnings() {{
        // Implementation for fixing compiler warnings
    }}
}}"#
          )
        } else {
          format!(
            r#"package com.example.util;

import org.springframework.stereotype.Component;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

/**
 * Utility class for {option}
 */
@Component
public class CleanupUtil {{
    private static final Logger logger = LoggerFactory.getLogger(CleanupUtil.class);
    
    public void performCleanup() {{
        logger.info("Performing cleanup: {{}}", "{option}");
        
        // Implementation
    }}
}}"#
          )
        }
      }
    }
  }
}

#[derive(Debug, Default)]
pub struct TestRepoStats {
  pub issue_commits: usize,
  pub maintenance_commits: usize,
  pub conflict_branches: usize,
}

impl TestRepoStats {
  pub fn total_commits(&self) -> usize {
    self.issue_commits + self.maintenance_commits + 1 // +1 for initial commit
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use tempfile::TempDir;

  #[test]
  fn test_generate_repo() {
    let temp_dir = TempDir::new().unwrap();
    let mut generator = TestRepoGenerator::with_seed(42);

    let stats = generator.generate(temp_dir.path()).unwrap();

    assert!(stats.issue_commits > 0);
    assert!(stats.maintenance_commits > 0);
    assert!(stats.conflict_branches > 0);
  }
}
