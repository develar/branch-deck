use anyhow::Result;
use std::fs;
use std::path::Path;
use std::process::Command;

/// Generates test repositories that demonstrate Branch Deck conflict scenarios
pub struct TestRepoGenerator;

impl Default for TestRepoGenerator {
  fn default() -> Self {
    Self::new()
  }
}

impl TestRepoGenerator {
  pub fn new() -> Self {
    Self
  }

  /// Generate a test repository that demonstrates Branch Deck conflicts
  pub fn generate(&self, output_path: &Path) -> Result<()> {
    // Remove existing directory if it exists
    if output_path.exists() {
      fs::remove_dir_all(output_path)?;
    }

    // Create directory structure
    fs::create_dir_all(output_path)?;

    let origin_path = output_path.join("origin.git");
    let working_path = output_path.join("working");

    // Create bare repository for origin
    fs::create_dir_all(&origin_path)?;
    self.init_bare_repo(&origin_path)?;

    // Create working repository
    fs::create_dir_all(&working_path)?;
    self.init_repo(&working_path)?;

    // Add origin remote
    self.add_origin(&working_path, &origin_path)?;

    // Create initial project structure
    self.create_initial_commit(&working_path)?;

    // Create unassigned UserService commit
    self.create_unprefixed_commits(&working_path)?;

    // Push only the unprefixed commits (2 commits: initial + UserService)
    self.push_to_origin(&working_path, 2)?;

    // Create all prefixed commits (these stay unpushed as Branch Deck strips prefixes on push)
    self.create_interleaved_commits(&working_path)?;

    // Create commits that modify pushed files
    self.create_pushed_base_conflicts(&working_path)?;

    Ok(())
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

  fn init_bare_repo(&self, repo_path: &Path) -> Result<()> {
    let output = Command::new("git").args(["--no-pager", "init", "--bare"]).current_dir(repo_path).output()?;

    if !output.status.success() {
      anyhow::bail!("Git init --bare failed: {}", String::from_utf8_lossy(&output.stderr));
    }

    Ok(())
  }

  fn add_origin(&self, repo_path: &Path, _origin_path: &Path) -> Result<()> {
    let output = Command::new("git")
      .args(["--no-pager", "remote", "add", "origin", "../origin.git"])
      .current_dir(repo_path)
      .output()?;

    if !output.status.success() {
      anyhow::bail!("Git remote add failed: {}", String::from_utf8_lossy(&output.stderr));
    }

    Ok(())
  }

  fn push_to_origin(&self, repo_path: &Path, num_commits: usize) -> Result<()> {
    // First, get the commit hash at the position we want to push up to
    let output = Command::new("git")
      .args(["--no-pager", "log", "--oneline", "--reverse", "-n", &format!("{}", num_commits + 1)])
      .current_dir(repo_path)
      .output()?;

    if !output.status.success() {
      anyhow::bail!("Git log failed: {}", String::from_utf8_lossy(&output.stderr));
    }

    // Parse the output to get the commit hash at position num_commits
    let log_output = String::from_utf8_lossy(&output.stdout);
    let lines: Vec<&str> = log_output.lines().collect();
    if lines.len() < num_commits {
      anyhow::bail!("Not enough commits to push. Have {}, need {}", lines.len(), num_commits);
    }

    let target_commit = lines[num_commits - 1]
      .split_whitespace()
      .next()
      .ok_or_else(|| anyhow::anyhow!("Failed to parse commit hash"))?;

    // Reset master on origin to only include the specified number of commits
    let output = Command::new("git")
      .args(["--no-pager", "push", "origin", &format!("{target_commit}:refs/heads/master"), "-f"])
      .current_dir(repo_path)
      .output()?;

    if !output.status.success() {
      anyhow::bail!("Git push failed: {}", String::from_utf8_lossy(&output.stderr));
    }

    // Update remote tracking branch
    let output = Command::new("git").args(["--no-pager", "fetch", "origin"]).current_dir(repo_path).output()?;

    if !output.status.success() {
      anyhow::bail!("Git fetch failed: {}", String::from_utf8_lossy(&output.stderr));
    }

    tracing::info!("✅ Pushed {num_commits} commits to origin");

    Ok(())
  }

  fn create_initial_commit(&self, repo_path: &Path) -> Result<()> {
    // Create a simple Spring Boot project structure
    let files = vec![
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
    ];

    self.create_commit(repo_path, "Initial project setup", &files)
  }

  fn create_unprefixed_commits(&self, repo_path: &Path) -> Result<()> {
    // 1. Create UserService without any features (unassigned)
    let user_service_basic = r#"package com.example.service

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
)"#;

    self.create_commit(repo_path, "Add UserService", &[("src/main/kotlin/com/example/service/UserService.kt", user_service_basic)])?;

    Ok(())
  }

  fn create_interleaved_commits(&self, repo_path: &Path) -> Result<()> {
    // 2. Add authentication to UserService
    let user_service_with_auth = r#"package com.example.service

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
)"#;

    self.create_commit(
      repo_path,
      "(auth) Add authentication to UserService",
      &[("src/main/kotlin/com/example/service/UserService.kt", user_service_with_auth)],
    )?;

    // 3. Add caching to UserService (different feature, modifies same file)
    let user_service_with_cache = r#"package com.example.service

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
)"#;

    self.create_commit(
      repo_path,
      "(cache) Add caching to UserService",
      &[("src/main/kotlin/com/example/service/UserService.kt", user_service_with_cache)],
    )?;

    // 4. Auth feature now depends on cache (this will conflict when grouped separately)
    let user_service_jwt_with_cache = r#"package com.example.service

import org.springframework.stereotype.Service
import java.util.Base64

@Service
class UserService {
    private val users = mutableMapOf<String, User>()
    private val tokens = mutableMapOf<String, String>() // token -> userId
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
}"#;

    self.create_commit(
      repo_path,
      "(auth) Add JWT tokens using cache",
      &[("src/main/kotlin/com/example/service/UserService.kt", user_service_jwt_with_cache)],
    )?;

    // 5. Unassigned refactoring
    let config_file = r#"package com.example.config

object AppConfig {
    const val JWT_EXPIRY_MS = 3600000L // 1 hour
    const val CACHE_SIZE_LIMIT = 1000
    const val TOKEN_PREFIX = "jwt."
}"#;

    self.create_commit(
      repo_path,
      "Refactor: Extract configuration constants",
      &[("src/main/kotlin/com/example/config/AppConfig.kt", config_file)],
    )?;

    // 6. Cache feature depends on auth (creates circular dependency when grouped)
    let user_service_cache_auth = r#"package com.example.service

import org.springframework.stereotype.Service
import com.example.config.AppConfig
import java.util.Base64

@Service
class UserService {
    private val users = mutableMapOf<String, User>()
    private val tokens = mutableMapOf<String, String>()
    private val cache = mutableMapOf<String, User>()
    private val jwtCache = mutableMapOf<String, JwtToken>()
    private val authCache = mutableMapOf<String, Long>() // Cache auth attempts
    
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
        // Check auth cache to prevent brute force
        val cacheKey = "$email:${hashPassword(password)}"
        val lastAttempt = authCache[cacheKey]
        if (lastAttempt != null && System.currentTimeMillis() - lastAttempt < 1000) {
            return null // Rate limited
        }
        authCache[cacheKey] = System.currentTimeMillis()
        
        val user = users.values.find { it.email == email }
        return if (user != null && verifyPassword(password, user.passwordHash)) {
            val cachedJwt = jwtCache[user.id]
            if (cachedJwt != null && !cachedJwt.isExpired()) {
                return cachedJwt.token
            }
            
            val jwt = generateJWT(user.id)
            jwtCache[user.id] = jwt
            jwt.token
        } else null
    }
    
    fun getUserByToken(token: String): User? {
        if (token.startsWith(AppConfig.TOKEN_PREFIX)) {
            val userId = decodeJWT(token)
            return userId?.let { getUser(it) }
        }
        
        val userId = tokens[token] ?: return null
        return getUser(userId)
    }
    
    fun clearCache() {
        cache.clear()
        jwtCache.clear()
        authCache.clear()
    }
    
    fun getCacheStats(): CacheStats {
        return CacheStats(
            userCacheSize = cache.size,
            jwtCacheSize = jwtCache.size,
            authCacheSize = authCache.size,
            totalSize = cache.size + jwtCache.size + authCache.size
        )
    }
    
    private fun generateId(): String = "user-${System.currentTimeMillis()}"
    private fun generateToken(): String = "token-${System.currentTimeMillis()}"
    private fun hashPassword(password: String): String = password.reversed()
    private fun verifyPassword(password: String, hash: String): Boolean = password.reversed() == hash
    
    private fun generateJWT(userId: String): JwtToken {
        val token = "${AppConfig.TOKEN_PREFIX}${Base64.getEncoder().encodeToString(userId.toByteArray())}.${System.currentTimeMillis()}"
        return JwtToken(token, System.currentTimeMillis() + AppConfig.JWT_EXPIRY_MS)
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
}

data class CacheStats(
    val userCacheSize: Int,
    val jwtCacheSize: Int,
    val authCacheSize: Int,
    val totalSize: Int
)"#;

    self.create_commit(
      repo_path,
      "(cache) Cache auth tokens and add stats",
      &[("src/main/kotlin/com/example/service/UserService.kt", user_service_cache_auth)],
    )?;

    // 7. Add metrics service (new feature)
    let metrics_service = r#"package com.example.service

import org.springframework.stereotype.Service

@Service
class MetricsService {
    private val counters = mutableMapOf<String, Long>()
    private val timers = mutableMapOf<String, MutableList<Long>>()
    
    fun incrementCounter(name: String) {
        counters[name] = (counters[name] ?: 0) + 1
    }
    
    fun recordTime(name: String, durationMs: Long) {
        timers.getOrPut(name) { mutableListOf() }.add(durationMs)
    }
    
    fun getCounter(name: String): Long = counters[name] ?: 0
    
    fun getAverageTime(name: String): Double {
        val times = timers[name] ?: return 0.0
        return if (times.isEmpty()) 0.0 else times.average()
    }
    
    fun reset() {
        counters.clear()
        timers.clear()
    }
}"#;

    self.create_commit(
      repo_path,
      "(metrics) Add metrics service",
      &[("src/main/kotlin/com/example/service/MetricsService.kt", metrics_service)],
    )?;

    // 8. Auth adds metrics (depends on metrics service)
    let user_service_auth_metrics = r#"package com.example.service

import org.springframework.stereotype.Service
import com.example.config.AppConfig
import java.util.Base64

@Service
class UserService(
    private val metricsService: MetricsService
) {
    private val users = mutableMapOf<String, User>()
    private val tokens = mutableMapOf<String, String>()
    private val cache = mutableMapOf<String, User>()
    private val jwtCache = mutableMapOf<String, JwtToken>()
    private val authCache = mutableMapOf<String, Long>()
    
    fun getUser(id: String): User? {
        val startTime = System.currentTimeMillis()
        
        cache[id]?.let { 
            metricsService.incrementCounter("user.cache.hit")
            metricsService.recordTime("user.get", System.currentTimeMillis() - startTime)
            return it 
        }
        
        metricsService.incrementCounter("user.cache.miss")
        val user = users[id]
        if (user != null) {
            cache[id] = user
        }
        metricsService.recordTime("user.get", System.currentTimeMillis() - startTime)
        return user
    }
    
    fun createUser(name: String, email: String, password: String): User {
        val startTime = System.currentTimeMillis()
        metricsService.incrementCounter("user.create")
        
        val user = User(
            id = generateId(),
            name = name,
            email = email,
            passwordHash = hashPassword(password),
            cached = false
        )
        users[user.id] = user
        cache[user.id] = user.copy(cached = true)
        
        metricsService.recordTime("user.create", System.currentTimeMillis() - startTime)
        return user
    }
    
    fun authenticate(email: String, password: String): String? {
        val startTime = System.currentTimeMillis()
        metricsService.incrementCounter("auth.attempt")
        
        val cacheKey = "$email:${hashPassword(password)}"
        val lastAttempt = authCache[cacheKey]
        if (lastAttempt != null && System.currentTimeMillis() - lastAttempt < 1000) {
            metricsService.incrementCounter("auth.rate_limited")
            return null
        }
        authCache[cacheKey] = System.currentTimeMillis()
        
        val user = users.values.find { it.email == email }
        return if (user != null && verifyPassword(password, user.passwordHash)) {
            metricsService.incrementCounter("auth.success")
            
            val cachedJwt = jwtCache[user.id]
            if (cachedJwt != null && !cachedJwt.isExpired()) {
                metricsService.incrementCounter("jwt.cache.hit")
                metricsService.recordTime("auth.total", System.currentTimeMillis() - startTime)
                return cachedJwt.token
            }
            
            metricsService.incrementCounter("jwt.cache.miss")
            val jwt = generateJWT(user.id)
            jwtCache[user.id] = jwt
            metricsService.recordTime("auth.total", System.currentTimeMillis() - startTime)
            jwt.token
        } else {
            metricsService.incrementCounter("auth.failure")
            metricsService.recordTime("auth.total", System.currentTimeMillis() - startTime)
            null
        }
    }
    
    fun getUserByToken(token: String): User? {
        metricsService.incrementCounter("token.validate")
        
        if (token.startsWith(AppConfig.TOKEN_PREFIX)) {
            val userId = decodeJWT(token)
            return userId?.let { getUser(it) }
        }
        
        val userId = tokens[token] ?: return null
        return getUser(userId)
    }
    
    fun clearCache() {
        cache.clear()
        jwtCache.clear()
        authCache.clear()
        metricsService.incrementCounter("cache.clear")
    }
    
    fun getCacheStats(): CacheStats {
        return CacheStats(
            userCacheSize = cache.size,
            jwtCacheSize = jwtCache.size,
            authCacheSize = authCache.size,
            totalSize = cache.size + jwtCache.size + authCache.size
        )
    }
    
    private fun generateId(): String = "user-${System.currentTimeMillis()}"
    private fun generateToken(): String = "token-${System.currentTimeMillis()}"
    private fun hashPassword(password: String): String = password.reversed()
    private fun verifyPassword(password: String, hash: String): Boolean = password.reversed() == hash
    
    private fun generateJWT(userId: String): JwtToken {
        val token = "${AppConfig.TOKEN_PREFIX}${Base64.getEncoder().encodeToString(userId.toByteArray())}.${System.currentTimeMillis()}"
        return JwtToken(token, System.currentTimeMillis() + AppConfig.JWT_EXPIRY_MS)
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
}

data class CacheStats(
    val userCacheSize: Int,
    val jwtCacheSize: Int,
    val authCacheSize: Int,
    val totalSize: Int
)"#;

    self.create_commit(
      repo_path,
      "(auth) Add metrics to authentication",
      &[("src/main/kotlin/com/example/service/UserService.kt", user_service_auth_metrics)],
    )?;

    // 9. Cache adds metrics (also depends on metrics)
    let cache_service = r#"package com.example.service

import org.springframework.stereotype.Service

@Service
class CacheService(
    private val metricsService: MetricsService
) {
    private val caches = mutableMapOf<String, MutableMap<String, Any>>()
    
    fun <T> get(cacheName: String, key: String): T? {
        metricsService.incrementCounter("cache.$cacheName.get")
        
        val cache = caches[cacheName] ?: return null
        val value = cache[key]
        
        if (value != null) {
            metricsService.incrementCounter("cache.$cacheName.hit")
        } else {
            metricsService.incrementCounter("cache.$cacheName.miss")
        }
        
        @Suppress("UNCHECKED_CAST")
        return value as? T
    }
    
    fun put(cacheName: String, key: String, value: Any) {
        metricsService.incrementCounter("cache.$cacheName.put")
        caches.getOrPut(cacheName) { mutableMapOf() }[key] = value
    }
    
    fun evict(cacheName: String, key: String) {
        metricsService.incrementCounter("cache.$cacheName.evict")
        caches[cacheName]?.remove(key)
    }
    
    fun clear(cacheName: String) {
        metricsService.incrementCounter("cache.$cacheName.clear")
        caches[cacheName]?.clear()
    }
    
    fun getCacheSize(cacheName: String): Int = caches[cacheName]?.size ?: 0
    
    fun getHitRate(cacheName: String): Double {
        val hits = metricsService.getCounter("cache.$cacheName.hit")
        val gets = metricsService.getCounter("cache.$cacheName.get")
        return if (gets > 0) hits.toDouble() / gets else 0.0
    }
}"#;

    self.create_commit(
      repo_path,
      "(cache) Add dedicated cache service with metrics",
      &[("src/main/kotlin/com/example/service/CacheService.kt", cache_service)],
    )?;

    // 10. Metrics tracks cache hit rate (depends on both cache commits)
    let metrics_dashboard = r#"package com.example.service

import org.springframework.stereotype.Service

@Service
class MetricsDashboard(
    private val metricsService: MetricsService,
    private val cacheService: CacheService
) {
    fun getDashboard(): Dashboard {
        return Dashboard(
            authMetrics = AuthMetrics(
                totalAttempts = metricsService.getCounter("auth.attempt"),
                successfulAuths = metricsService.getCounter("auth.success"),
                failedAuths = metricsService.getCounter("auth.failure"),
                rateLimited = metricsService.getCounter("auth.rate_limited"),
                averageAuthTime = metricsService.getAverageTime("auth.total")
            ),
            cacheMetrics = CacheMetrics(
                userCacheHitRate = cacheService.getHitRate("user"),
                jwtCacheHitRate = calculateJwtHitRate(),
                totalCacheOperations = calculateTotalCacheOps()
            ),
            userMetrics = UserMetrics(
                totalUsers = metricsService.getCounter("user.create"),
                averageGetTime = metricsService.getAverageTime("user.get"),
                cacheHitRate = calculateUserCacheHitRate()
            )
        )
    }
    
    private fun calculateJwtHitRate(): Double {
        val hits = metricsService.getCounter("jwt.cache.hit")
        val total = hits + metricsService.getCounter("jwt.cache.miss")
        return if (total > 0) hits.toDouble() / total else 0.0
    }
    
    private fun calculateUserCacheHitRate(): Double {
        val hits = metricsService.getCounter("user.cache.hit")
        val total = hits + metricsService.getCounter("user.cache.miss")
        return if (total > 0) hits.toDouble() / total else 0.0
    }
    
    private fun calculateTotalCacheOps(): Long {
        return metricsService.getCounter("cache.user.get") +
               metricsService.getCounter("cache.user.put") +
               metricsService.getCounter("cache.user.evict")
    }
}

data class Dashboard(
    val authMetrics: AuthMetrics,
    val cacheMetrics: CacheMetrics,
    val userMetrics: UserMetrics
)

data class AuthMetrics(
    val totalAttempts: Long,
    val successfulAuths: Long,
    val failedAuths: Long,
    val rateLimited: Long,
    val averageAuthTime: Double
)

data class CacheMetrics(
    val userCacheHitRate: Double,
    val jwtCacheHitRate: Double,
    val totalCacheOperations: Long
)

data class UserMetrics(
    val totalUsers: Long,
    val averageGetTime: Double,
    val cacheHitRate: Double
)"#;

    self.create_commit(
      repo_path,
      "(metrics) Add metrics dashboard tracking cache performance",
      &[("src/main/kotlin/com/example/service/MetricsDashboard.kt", metrics_dashboard)],
    )?;

    tracing::info!(
      "✅ Created test repository with interleaved commits. \
       📊 Commit structure demonstrates Branch Deck conflicts: \
       Commits 4 depends on 3 (different prefix), \
       Commit 6 depends on 2 (different prefix), \
       Commits 8,9 depend on 7 (same file modifications), \
       Commit 10 depends on both 8 and 9"
    );

    Ok(())
  }

  fn create_pushed_base_conflicts(&self, repo_path: &Path) -> Result<()> {
    // At this point, commits 1-4 have been pushed to origin
    // Now create new commits that modify files from pushed commits

    // 11. Security feature adds validation to already-pushed UserService
    let user_service_with_validation = r#"package com.example.service

import org.springframework.stereotype.Service
import com.example.config.AppConfig
import java.util.Base64

@Service
class UserService(
    private val metricsService: MetricsService
) {
    private val users = mutableMapOf<String, User>()
    private val tokens = mutableMapOf<String, String>()
    private val cache = mutableMapOf<String, User>()
    private val jwtCache = mutableMapOf<String, JwtToken>()
    private val authCache = mutableMapOf<String, Long>()
    
    fun getUser(id: String): User? {
        // Validate user ID format
        require(id.matches(Regex("user-\\d+"))) { "Invalid user ID format" }
        
        val startTime = System.currentTimeMillis()
        
        cache[id]?.let { 
            metricsService.incrementCounter("user.cache.hit")
            metricsService.recordTime("user.get", System.currentTimeMillis() - startTime)
            return it 
        }
        
        metricsService.incrementCounter("user.cache.miss")
        val user = users[id]
        if (user != null) {
            cache[id] = user
        }
        metricsService.recordTime("user.get", System.currentTimeMillis() - startTime)
        return user
    }
    
    fun createUser(name: String, email: String, password: String): User {
        // Input validation
        require(name.isNotBlank()) { "Name cannot be blank" }
        require(email.matches(Regex("^[A-Za-z0-9+_.-]+@(.+)$"))) { "Invalid email format" }
        require(password.length >= 8) { "Password must be at least 8 characters" }
        
        val startTime = System.currentTimeMillis()
        metricsService.incrementCounter("user.create")
        
        val user = User(
            id = generateId(),
            name = name.trim(),
            email = email.toLowerCase().trim(),
            passwordHash = hashPassword(password),
            cached = false
        )
        users[user.id] = user
        cache[user.id] = user.copy(cached = true)
        
        metricsService.recordTime("user.create", System.currentTimeMillis() - startTime)
        return user
    }
    
    fun authenticate(email: String, password: String): String? {
        // Validate inputs
        if (email.isBlank() || password.isBlank()) {
            metricsService.incrementCounter("auth.invalid_input")
            return null
        }
        
        val startTime = System.currentTimeMillis()
        metricsService.incrementCounter("auth.attempt")
        
        val cacheKey = "$email:${hashPassword(password)}"
        val lastAttempt = authCache[cacheKey]
        if (lastAttempt != null && System.currentTimeMillis() - lastAttempt < 1000) {
            metricsService.incrementCounter("auth.rate_limited")
            return null
        }
        authCache[cacheKey] = System.currentTimeMillis()
        
        val user = users.values.find { it.email == email }
        return if (user != null && verifyPassword(password, user.passwordHash)) {
            metricsService.incrementCounter("auth.success")
            
            val cachedJwt = jwtCache[user.id]
            if (cachedJwt != null && !cachedJwt.isExpired()) {
                metricsService.incrementCounter("jwt.cache.hit")
                metricsService.recordTime("auth.total", System.currentTimeMillis() - startTime)
                return cachedJwt.token
            }
            
            metricsService.incrementCounter("jwt.cache.miss")
            val jwt = generateJWT(user.id)
            jwtCache[user.id] = jwt
            metricsService.recordTime("auth.total", System.currentTimeMillis() - startTime)
            jwt.token
        } else {
            metricsService.incrementCounter("auth.failure")
            metricsService.recordTime("auth.total", System.currentTimeMillis() - startTime)
            null
        }
    }
    
    fun getUserByToken(token: String): User? {
        metricsService.incrementCounter("token.validate")
        
        if (token.startsWith(AppConfig.TOKEN_PREFIX)) {
            val userId = decodeJWT(token)
            return userId?.let { getUser(it) }
        }
        
        val userId = tokens[token] ?: return null
        return getUser(userId)
    }
    
    fun clearCache() {
        cache.clear()
        jwtCache.clear()
        authCache.clear()
        metricsService.incrementCounter("cache.clear")
    }
    
    fun getCacheStats(): CacheStats {
        return CacheStats(
            userCacheSize = cache.size,
            jwtCacheSize = jwtCache.size,
            authCacheSize = authCache.size,
            totalSize = cache.size + jwtCache.size + authCache.size
        )
    }
    
    private fun generateId(): String = "user-${System.currentTimeMillis()}"
    private fun generateToken(): String = "token-${System.currentTimeMillis()}"
    private fun hashPassword(password: String): String = password.reversed()
    private fun verifyPassword(password: String, hash: String): Boolean = password.reversed() == hash
    
    private fun generateJWT(userId: String): JwtToken {
        val token = "${AppConfig.TOKEN_PREFIX}${Base64.getEncoder().encodeToString(userId.toByteArray())}.${System.currentTimeMillis()}"
        return JwtToken(token, System.currentTimeMillis() + AppConfig.JWT_EXPIRY_MS)
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
}

data class CacheStats(
    val userCacheSize: Int,
    val jwtCacheSize: Int,
    val authCacheSize: Int,
    val totalSize: Int
)"#;

    self.create_commit(
      repo_path,
      "(sec) Add input validation to UserService",
      &[("src/main/kotlin/com/example/service/UserService.kt", user_service_with_validation)],
    )?;

    // 12. Performance optimization for UserService
    let user_service_with_perf = r#"package com.example.service

import org.springframework.stereotype.Service
import com.example.config.AppConfig
import java.util.Base64
import java.util.concurrent.ConcurrentHashMap

@Service
class UserService(
    private val metricsService: MetricsService
) {
    // Use concurrent collections for thread safety and performance
    private val users = ConcurrentHashMap<String, User>()
    private val tokens = ConcurrentHashMap<String, String>()
    private val cache = ConcurrentHashMap<String, User>()
    private val jwtCache = ConcurrentHashMap<String, JwtToken>()
    private val authCache = ConcurrentHashMap<String, Long>()
    
    // Lazy load user data
    private val lazyUserLoader = lazy { loadAllUsers() }
    
    fun getUser(id: String): User? {
        require(id.matches(Regex("user-\\d+"))) { "Invalid user ID format" }
        
        val startTime = System.currentTimeMillis()
        
        // Check cache with optimized lookup
        cache[id]?.let { 
            metricsService.incrementCounter("user.cache.hit")
            metricsService.recordTime("user.get", System.currentTimeMillis() - startTime)
            return it 
        }
        
        metricsService.incrementCounter("user.cache.miss")
        
        // Trigger lazy loading if needed
        if (users.isEmpty()) {
            lazyUserLoader.value
        }
        
        val user = users[id]
        if (user != null) {
            // Use putIfAbsent for thread safety
            cache.putIfAbsent(id, user)
        }
        metricsService.recordTime("user.get", System.currentTimeMillis() - startTime)
        return user
    }
    
    fun createUser(name: String, email: String, password: String): User {
        require(name.isNotBlank()) { "Name cannot be blank" }
        require(email.matches(Regex("^[A-Za-z0-9+_.-]+@(.+)$"))) { "Invalid email format" }
        require(password.length >= 8) { "Password must be at least 8 characters" }
        
        val startTime = System.currentTimeMillis()
        metricsService.incrementCounter("user.create")
        
        // Pre-compute values to reduce redundant operations
        val trimmedName = name.trim()
        val normalizedEmail = email.toLowerCase().trim()
        val hashedPassword = hashPassword(password)
        
        val user = User(
            id = generateId(),
            name = trimmedName,
            email = normalizedEmail,
            passwordHash = hashedPassword,
            cached = false
        )
        
        // Atomic operations
        users.putIfAbsent(user.id, user)
        cache.putIfAbsent(user.id, user.copy(cached = true))
        
        metricsService.recordTime("user.create", System.currentTimeMillis() - startTime)
        return user
    }
    
    fun authenticate(email: String, password: String): String? {
        if (email.isBlank() || password.isBlank()) {
            metricsService.incrementCounter("auth.invalid_input")
            return null
        }
        
        val startTime = System.currentTimeMillis()
        metricsService.incrementCounter("auth.attempt")
        
        // Pre-compute cache key
        val normalizedEmail = email.toLowerCase().trim()
        val cacheKey = "$normalizedEmail:${hashPassword(password)}"
        
        // Optimized rate limiting check
        val now = System.currentTimeMillis()
        val lastAttempt = authCache[cacheKey]
        if (lastAttempt != null && now - lastAttempt < 1000) {
            metricsService.incrementCounter("auth.rate_limited")
            return null
        }
        authCache[cacheKey] = now
        
        // Optimized user lookup by email index
        val user = findUserByEmail(normalizedEmail)
        return if (user != null && verifyPassword(password, user.passwordHash)) {
            metricsService.incrementCounter("auth.success")
            
            // Check JWT cache with compute if absent
            val jwt = jwtCache.compute(user.id) { _, existing ->
                if (existing != null && !existing.isExpired()) {
                    metricsService.incrementCounter("jwt.cache.hit")
                    existing
                } else {
                    metricsService.incrementCounter("jwt.cache.miss")
                    generateJWT(user.id)
                }
            }
            
            metricsService.recordTime("auth.total", System.currentTimeMillis() - startTime)
            jwt?.token
        } else {
            metricsService.incrementCounter("auth.failure")
            metricsService.recordTime("auth.total", System.currentTimeMillis() - startTime)
            null
        }
    }
    
    fun getUserByToken(token: String): User? {
        metricsService.incrementCounter("token.validate")
        
        if (token.startsWith(AppConfig.TOKEN_PREFIX)) {
            val userId = decodeJWT(token)
            return userId?.let { getUser(it) }
        }
        
        val userId = tokens[token] ?: return null
        return getUser(userId)
    }
    
    fun clearCache() {
        cache.clear()
        jwtCache.clear()
        authCache.clear()
        metricsService.incrementCounter("cache.clear")
    }
    
    fun getCacheStats(): CacheStats {
        return CacheStats(
            userCacheSize = cache.size,
            jwtCacheSize = jwtCache.size,
            authCacheSize = authCache.size,
            totalSize = cache.size + jwtCache.size + authCache.size
        )
    }
    
    // Optimized user lookup with email index
    private fun findUserByEmail(email: String): User? {
        // In production, this would use an index
        return users.values.find { it.email == email }
    }
    
    private fun loadAllUsers(): Map<String, User> {
        // Simulate loading users from database
        metricsService.incrementCounter("user.lazy_load")
        return emptyMap()
    }
    
    private fun generateId(): String = "user-${System.nanoTime()}" // More unique IDs
    private fun generateToken(): String = "token-${System.nanoTime()}"
    private fun hashPassword(password: String): String = password.reversed()
    private fun verifyPassword(password: String, hash: String): Boolean = password.reversed() == hash
    
    private fun generateJWT(userId: String): JwtToken {
        val token = "${AppConfig.TOKEN_PREFIX}${Base64.getEncoder().encodeToString(userId.toByteArray())}.${System.currentTimeMillis()}"
        return JwtToken(token, System.currentTimeMillis() + AppConfig.JWT_EXPIRY_MS)
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
}

data class CacheStats(
    val userCacheSize: Int,
    val jwtCacheSize: Int,
    val authCacheSize: Int,
    val totalSize: Int
)"#;

    self.create_commit(
      repo_path,
      "(perf) Optimize UserService with concurrent collections",
      &[("src/main/kotlin/com/example/service/UserService.kt", user_service_with_perf)],
    )?;

    // 13. Security adds audit logging (depends on metrics from pushed commits)
    let audit_service = r#"package com.example.service

import org.springframework.stereotype.Service
import java.time.Instant

@Service
class AuditService(
    private val metricsService: MetricsService
) {
    private val auditLog = mutableListOf<AuditEntry>()
    
    fun logSecurityEvent(event: SecurityEvent, userId: String?, details: String) {
        val entry = AuditEntry(
            timestamp = Instant.now(),
            event = event,
            userId = userId,
            details = details
        )
        auditLog.add(entry)
        metricsService.incrementCounter("audit.${event.name.toLowerCase()}")
    }
    
    fun logFailedAuth(email: String, reason: String) {
        logSecurityEvent(
            SecurityEvent.AUTH_FAILED,
            null,
            "Failed auth for email: $email, reason: $reason"
        )
    }
    
    fun logSuccessfulAuth(userId: String) {
        logSecurityEvent(
            SecurityEvent.AUTH_SUCCESS,
            userId,
            "Successful authentication"
        )
    }
    
    fun logRateLimited(email: String) {
        logSecurityEvent(
            SecurityEvent.RATE_LIMITED,
            null,
            "Rate limited: $email"
        )
    }
    
    fun getRecentEvents(limit: Int = 100): List<AuditEntry> {
        return auditLog.takeLast(limit)
    }
    
    fun getEventsByUser(userId: String): List<AuditEntry> {
        return auditLog.filter { it.userId == userId }
    }
}

enum class SecurityEvent {
    AUTH_SUCCESS,
    AUTH_FAILED,
    RATE_LIMITED,
    INVALID_TOKEN,
    USER_CREATED,
    USER_DELETED,
    PERMISSION_DENIED
}

data class AuditEntry(
    val timestamp: Instant,
    val event: SecurityEvent,
    val userId: String?,
    val details: String
)"#;

    self.create_commit(
      repo_path,
      "(sec) Add security audit logging",
      &[("src/main/kotlin/com/example/service/AuditService.kt", audit_service)],
    )?;

    // 14. Performance adds batch operations (modifies CacheService from pushed commits)
    let cache_service_batch = r#"package com.example.service

import org.springframework.stereotype.Service
import java.util.concurrent.CompletableFuture
import java.util.concurrent.ConcurrentHashMap

@Service
class CacheService(
    private val metricsService: MetricsService
) {
    private val caches = ConcurrentHashMap<String, ConcurrentHashMap<String, Any>>()
    
    fun <T> get(cacheName: String, key: String): T? {
        metricsService.incrementCounter("cache.$cacheName.get")
        
        val cache = caches[cacheName] ?: return null
        val value = cache[key]
        
        if (value != null) {
            metricsService.incrementCounter("cache.$cacheName.hit")
        } else {
            metricsService.incrementCounter("cache.$cacheName.miss")
        }
        
        @Suppress("UNCHECKED_CAST")
        return value as? T
    }
    
    // Batch get operation for performance
    fun <T> getBatch(cacheName: String, keys: List<String>): Map<String, T> {
        val startTime = System.currentTimeMillis()
        metricsService.incrementCounter("cache.$cacheName.batch_get")
        
        val cache = caches[cacheName] ?: return emptyMap()
        val results = mutableMapOf<String, T>()
        var hits = 0
        
        keys.forEach { key ->
            cache[key]?.let { value ->
                @Suppress("UNCHECKED_CAST")
                results[key] = value as T
                hits++
            }
        }
        
        metricsService.incrementCounter("cache.$cacheName.batch_hits", hits.toLong())
        metricsService.incrementCounter("cache.$cacheName.batch_misses", (keys.size - hits).toLong())
        metricsService.recordTime("cache.$cacheName.batch_get_time", System.currentTimeMillis() - startTime)
        
        return results
    }
    
    fun put(cacheName: String, key: String, value: Any) {
        metricsService.incrementCounter("cache.$cacheName.put")
        caches.getOrPut(cacheName) { ConcurrentHashMap() }[key] = value
    }
    
    // Batch put operation for performance
    fun putBatch(cacheName: String, entries: Map<String, Any>) {
        val startTime = System.currentTimeMillis()
        metricsService.incrementCounter("cache.$cacheName.batch_put")
        
        val cache = caches.getOrPut(cacheName) { ConcurrentHashMap() }
        cache.putAll(entries)
        
        metricsService.incrementCounter("cache.$cacheName.batch_put_count", entries.size.toLong())
        metricsService.recordTime("cache.$cacheName.batch_put_time", System.currentTimeMillis() - startTime)
    }
    
    // Async batch operations
    fun <T> getBatchAsync(cacheName: String, keys: List<String>): CompletableFuture<Map<String, T>> {
        return CompletableFuture.supplyAsync {
            getBatch(cacheName, keys)
        }
    }
    
    fun putBatchAsync(cacheName: String, entries: Map<String, Any>): CompletableFuture<Void> {
        return CompletableFuture.runAsync {
            putBatch(cacheName, entries)
        }
    }
    
    fun evict(cacheName: String, key: String) {
        metricsService.incrementCounter("cache.$cacheName.evict")
        caches[cacheName]?.remove(key)
    }
    
    fun clear(cacheName: String) {
        metricsService.incrementCounter("cache.$cacheName.clear")
        caches[cacheName]?.clear()
    }
    
    fun getCacheSize(cacheName: String): Int = caches[cacheName]?.size ?: 0
    
    fun getHitRate(cacheName: String): Double {
        val hits = metricsService.getCounter("cache.$cacheName.hit")
        val gets = metricsService.getCounter("cache.$cacheName.get")
        return if (gets > 0) hits.toDouble() / gets else 0.0
    }
    
    // Performance monitoring
    fun getCacheMetrics(cacheName: String): CacheMetrics {
        return CacheMetrics(
            size = getCacheSize(cacheName),
            hitRate = getHitRate(cacheName),
            totalGets = metricsService.getCounter("cache.$cacheName.get"),
            totalPuts = metricsService.getCounter("cache.$cacheName.put"),
            batchOperations = metricsService.getCounter("cache.$cacheName.batch_get") +
                            metricsService.getCounter("cache.$cacheName.batch_put")
        )
    }
}

data class CacheMetrics(
    val size: Int,
    val hitRate: Double,
    val totalGets: Long,
    val totalPuts: Long,
    val batchOperations: Long
)"#;

    self.create_commit(
      repo_path,
      "(perf) Add batch operations to CacheService",
      &[("src/main/kotlin/com/example/service/CacheService.kt", cache_service_batch)],
    )?;

    // 15. Documentation updates (good branch with no conflicts)
    let api_docs = r#"# API Documentation

This document describes the REST API endpoints for the User Service.

## Authentication

All endpoints require authentication using JWT tokens. Include the token in the Authorization header:

```
Authorization: Bearer <token>
```

## Endpoints

### GET /api/users/{id}
Retrieves a user by their ID.

**Parameters:**
- `id` (path parameter): The user ID in format `user-<timestamp>`

**Response:**
```json
{
  "id": "user-1234567890",
  "name": "John Doe",
  "email": "john@example.com"
}
```

### POST /api/users
Creates a new user.

**Request Body:**
```json
{
  "name": "John Doe",
  "email": "john@example.com",
  "password": "securepassword"
}
```

**Response:**
```json
{
  "id": "user-1234567890",
  "name": "John Doe",
  "email": "john@example.com"
}
```

### POST /api/auth
Authenticates a user and returns a JWT token.

**Request Body:**
```json
{
  "email": "john@example.com",
  "password": "securepassword"
}
```

**Response:**
```json
{
  "token": "jwt.eyJ1c2VySWQiOiJ1c2VyLTEyMzQ1Njc4OTAifQ==.1234567890"
}
```

## Error Responses

All endpoints return appropriate HTTP status codes:
- 200: Success
- 400: Bad Request (invalid input)
- 401: Unauthorized
- 404: Not Found
- 429: Too Many Requests (rate limited)
- 500: Internal Server Error
"#;

    self.create_commit(
      repo_path,
      "(docs) Add comprehensive API documentation

This commit adds detailed API documentation for all REST endpoints
in the User Service. The documentation includes:

- Authentication requirements and token format
- Detailed endpoint descriptions with parameters
- Request/response examples with JSON schemas
- Common error codes and their meanings
- Usage examples with curl commands

The documentation follows OpenAPI 3.0 conventions and can be
used to generate interactive API documentation using tools
like Swagger UI or ReDoc.

This is part of our effort to improve developer experience
and make the API more accessible to frontend developers and
third-party integrations.",
      &[("docs/api.md", api_docs)],
    )?;

    // 16. Update README with architecture
    let readme_update = r#"# Test Repository

Generated test repository for Branch Deck conflict demonstration.

## Architecture Overview

This application follows a layered architecture pattern with clear separation of concerns:

### Service Layer
The service layer contains the business logic:
- **UserService**: Manages user CRUD operations and authentication
- **CacheService**: Provides caching functionality with metrics
- **MetricsService**: Collects and aggregates application metrics
- **AuditService**: Logs security events for compliance

### Configuration
- **AppConfig**: Centralized configuration constants
- Uses Spring Boot's configuration management
- Environment-specific settings via application.yml

### Security Features
- JWT-based authentication
- Password hashing (simplified for demo)
- Rate limiting on authentication attempts
- Security audit logging

### Performance Optimizations
- Multi-level caching strategy
- Concurrent collections for thread safety
- Batch operations for bulk processing
- Lazy loading of user data

## Project Structure

```
src/
├── main/
│   └── kotlin/
│       └── com/example/
│           ├── config/
│           │   └── AppConfig.kt
│           └── service/
│               ├── UserService.kt
│               ├── CacheService.kt
│               ├── MetricsService.kt
│               ├── MetricsDashboard.kt
│               └── AuditService.kt
└── test/
    └── kotlin/
```

## Getting Started

1. Clone the repository
2. Run `./gradlew bootRun`
3. Access the API at http://localhost:8080

## Development Workflow

This repository demonstrates Branch Deck's conflict scenarios:
- Interleaved commits with different prefixes
- Dependencies between features
- Pushed vs unpushed base conflicts
"#;

    self.create_commit(
      repo_path,
      "(docs) Update README with detailed architecture overview

Added comprehensive architecture documentation to help new developers
understand the system design and component interactions.

Key additions:
- System architecture overview with component descriptions
- Project structure visualization
- Security features documentation
- Performance optimization notes
- Development workflow guidelines

This documentation will serve as the primary reference for developers
joining the project and help maintain consistency in design decisions.",
      &[("README.md", readme_update)],
    )?;

    // 17. Developer setup guide
    let setup_guide = r#"# Developer Setup Guide

This guide will help you set up your development environment for the project.

## Prerequisites

- JDK 17 or higher
- Gradle 7.x
- Git
- Your favorite IDE (IntelliJ IDEA recommended)

## Initial Setup

### 1. Clone the Repository

```bash
git clone <repository-url>
cd test-repo
```

### 2. Install Dependencies

```bash
./gradlew build
```

This will download all required dependencies and compile the project.

### 3. IDE Configuration

#### IntelliJ IDEA
1. Open IntelliJ IDEA
2. Select "Open" and choose the project directory
3. Wait for indexing to complete
4. Enable annotation processing: 
   - Settings → Build, Execution, Deployment → Compiler → Annotation Processors
   - Check "Enable annotation processing"

#### VS Code
1. Install the Kotlin extension
2. Install the Gradle extension
3. Open the project folder

### 4. Database Setup (if applicable)

For local development, we use an in-memory H2 database by default.
No additional setup required.

For production-like environment:
```bash
docker-compose up -d postgres
```

### 5. Environment Variables

Create a `.env` file in the project root:
```env
JWT_SECRET=your-secret-key
CACHE_SIZE=1000
RATE_LIMIT_WINDOW=60000
```

## Running the Application

### Development Mode
```bash
./gradlew bootRun
```

### Debug Mode
```bash
./gradlew bootRun --debug-jvm
```

### Running Tests
```bash
./gradlew test
```

### Building for Production
```bash
./gradlew bootJar
```

## Common Issues

### Port Already in Use
If port 8080 is already in use:
```bash
./gradlew bootRun --args='--server.port=8081'
```

### Gradle Build Failures
Clear the cache and rebuild:
```bash
./gradlew clean build --refresh-dependencies
```

### IDE Not Recognizing Kotlin Files
- Ensure Kotlin plugin is installed and enabled
- Invalidate caches and restart the IDE

## Code Style

We use the official Kotlin coding conventions. Configure your IDE:
- IntelliJ: Settings → Editor → Code Style → Kotlin → Set from → Kotlin style guide
- Run `./gradlew ktlintFormat` before committing

## Commit Guidelines

Use conventional commits:
- `feat:` for new features
- `fix:` for bug fixes
- `docs:` for documentation
- `refactor:` for code refactoring
- `test:` for tests
- `chore:` for maintenance tasks
"#;

    self.create_commit(repo_path, "(docs) Add comprehensive developer setup guide", &[("docs/setup.md", setup_guide)])?;

    // 18. Deployment documentation
    let deployment_docs = r#"# Deployment Guide

This guide covers deployment strategies and procedures for the application.

## Deployment Overview

The application can be deployed using various strategies depending on your infrastructure and requirements.

### Supported Deployment Targets

1. **Kubernetes** (recommended for production)
2. **Docker Containers**
3. **Traditional VM/Bare Metal**
4. **Cloud Platforms** (AWS, GCP, Azure)

## Kubernetes Deployment

### Prerequisites
- Kubernetes cluster (1.19+)
- kubectl configured
- Helm 3.x (optional but recommended)

### Step 1: Build Docker Image

```bash
# Build the application
./gradlew bootJar

# Build Docker image
docker build -t myapp:latest .

# Push to registry
docker tag myapp:latest registry.example.com/myapp:latest
docker push registry.example.com/myapp:latest
```

### Step 2: Deploy to Kubernetes

Using kubectl:
```bash
kubectl apply -f k8s/deployment.yaml
kubectl apply -f k8s/service.yaml
kubectl apply -f k8s/ingress.yaml
```

Using Helm:
```bash
helm install myapp ./helm-chart \
  --set image.tag=latest \
  --set ingress.enabled=true \
  --set ingress.host=myapp.example.com
```

### Step 3: Configure Secrets

```bash
kubectl create secret generic app-secrets \
  --from-literal=jwt-secret=your-secret-key \
  --from-literal=db-password=your-db-password
```

## Docker Deployment

### Single Container

```bash
docker run -d \
  --name myapp \
  -p 8080:8080 \
  -e JWT_SECRET=your-secret-key \
  -e SPRING_PROFILES_ACTIVE=production \
  myapp:latest
```

### Docker Compose

```yaml
version: '3.8'
services:
  app:
    image: myapp:latest
    ports:
      - "8080:8080"
    environment:
      - SPRING_PROFILES_ACTIVE=production
      - JWT_SECRET=${JWT_SECRET}
    depends_on:
      - postgres
      - redis
    
  postgres:
    image: postgres:13
    environment:
      - POSTGRES_DB=myapp
      - POSTGRES_USER=myapp
      - POSTGRES_PASSWORD=${DB_PASSWORD}
    volumes:
      - postgres_data:/var/lib/postgresql/data
  
  redis:
    image: redis:6-alpine
    command: redis-server --maxmemory 256mb --maxmemory-policy allkeys-lru

volumes:
  postgres_data:
```

## Cloud Platform Deployment

### AWS Elastic Beanstalk

1. Install EB CLI:
   ```bash
   pip install awsebcli
   ```

2. Initialize application:
   ```bash
   eb init -p docker myapp
   ```

3. Create environment:
   ```bash
   eb create production --instance-type t3.medium
   ```

4. Deploy:
   ```bash
   eb deploy
   ```

### Google Cloud Run

```bash
# Build and push to GCR
gcloud builds submit --tag gcr.io/PROJECT_ID/myapp

# Deploy to Cloud Run
gcloud run deploy myapp \
  --image gcr.io/PROJECT_ID/myapp \
  --platform managed \
  --region us-central1 \
  --allow-unauthenticated
```

## Production Checklist

### Before Deployment

- [ ] All tests passing
- [ ] Security scan completed
- [ ] Performance testing done
- [ ] Database migrations prepared
- [ ] Backup procedures in place
- [ ] Monitoring configured
- [ ] Logging aggregation setup
- [ ] SSL certificates ready

### Configuration

Production environment variables:
```env
SPRING_PROFILES_ACTIVE=production
JWT_SECRET=<strong-secret>
JWT_EXPIRY_MS=3600000
CACHE_SIZE=10000
RATE_LIMIT_WINDOW=60000
DB_HOST=postgres.internal
DB_PORT=5432
DB_NAME=myapp_prod
DB_USER=myapp
DB_PASSWORD=<secure-password>
REDIS_HOST=redis.internal
REDIS_PORT=6379
LOG_LEVEL=INFO
METRICS_ENABLED=true
```

### Monitoring

Configure monitoring endpoints:
- Health check: `/actuator/health`
- Metrics: `/actuator/metrics`
- Info: `/actuator/info`

Recommended monitoring stack:
- Prometheus for metrics collection
- Grafana for visualization
- ELK stack for log aggregation
- PagerDuty for alerting

## Rollback Procedures

### Kubernetes Rollback

```bash
# View rollout history
kubectl rollout history deployment/myapp

# Rollback to previous version
kubectl rollout undo deployment/myapp

# Rollback to specific revision
kubectl rollout undo deployment/myapp --to-revision=2
```

### Database Rollback

Always test rollback procedures in staging first!

```sql
-- Rollback migration example
BEGIN;
-- Your rollback SQL here
COMMIT;
```

## Troubleshooting Deployment Issues

### Application Won't Start

1. Check logs:
   ```bash
   kubectl logs -f deployment/myapp
   docker logs myapp
   ```

2. Verify environment variables
3. Check database connectivity
4. Ensure sufficient resources

### Performance Issues

1. Check resource utilization
2. Review database query performance
3. Analyze cache hit rates
4. Check for memory leaks

### Connectivity Issues

1. Verify service discovery
2. Check ingress configuration
3. Test internal DNS resolution
4. Validate firewall rules
"#;

    self.create_commit(
      repo_path,
      "(docs) Add detailed deployment documentation

This comprehensive deployment guide covers multiple deployment strategies
and platforms to support various infrastructure requirements.

Included sections:
- Kubernetes deployment with Helm charts
- Docker and Docker Compose configurations  
- Cloud platform deployments (AWS, GCP)
- Production readiness checklist
- Monitoring and observability setup
- Rollback procedures
- Troubleshooting common issues

The guide follows best practices for cloud-native applications and
includes security considerations, performance optimization tips,
and operational procedures.

This documentation ensures smooth deployments and provides a reference
for DevOps teams managing the application in production environments.",
      &[("docs/deployment.md", deployment_docs)],
    )?;

    // 19. Troubleshooting guide
    let troubleshooting = r#"# Troubleshooting Guide

This guide helps diagnose and resolve common issues with the application.

## Common Issues and Solutions

### Authentication Issues

#### Problem: "Invalid token" error
**Symptoms:**
- API returns 401 Unauthorized
- Token validation fails

**Solutions:**
1. Check token expiration:
   ```bash
   # Decode JWT to check expiry
   echo $TOKEN | cut -d. -f2 | base64 -d
   ```

2. Verify JWT secret matches:
   - Check environment variable: `echo $JWT_SECRET`
   - Ensure same secret in all instances

3. Token format issues:
   - Ensure "Bearer " prefix in Authorization header
   - Check for extra whitespace

#### Problem: Rate limiting on login
**Symptoms:**
- Multiple failed login attempts
- 429 Too Many Requests response

**Solutions:**
1. Wait for rate limit window (default: 1 minute)
2. Check rate limit configuration
3. Clear auth cache if necessary

### Performance Issues

#### Problem: Slow API responses
**Diagnosis steps:**
1. Check response times in logs
2. Monitor cache hit rates
3. Analyze database query performance

**Solutions:**
1. Increase cache size:
   ```env
   CACHE_SIZE=5000
   ```

2. Enable query optimization:
   ```kotlin
   @Query("SELECT u FROM User u WHERE u.email = :email", 
          hints = @QueryHint(name = "org.hibernate.cacheable", value = "true"))
   ```

3. Add database indexes:
   ```sql
   CREATE INDEX idx_user_email ON users(email);
   ```

### Memory Issues

#### Problem: OutOfMemoryError
**Symptoms:**
- Application crashes with OOM
- Increasing memory usage over time

**Solutions:**
1. Increase heap size:
   ```bash
   java -Xmx2g -Xms1g -jar app.jar
   ```

2. Enable heap dump on OOM:
   ```bash
   -XX:+HeapDumpOnOutOfMemoryError -XX:HeapDumpPath=/tmp/heapdump.hprof
   ```

3. Analyze heap dump:
   ```bash
   jhat /tmp/heapdump.hprof
   ```

### Database Connection Issues

#### Problem: Connection pool exhausted
**Symptoms:**
- "Unable to acquire JDBC Connection" errors
- Timeouts on database operations

**Solutions:**
1. Increase pool size:
   ```properties
   spring.datasource.hikari.maximum-pool-size=20
   spring.datasource.hikari.minimum-idle=5
   ```

2. Check for connection leaks:
   - Review code for unclosed connections
   - Enable leak detection:
   ```properties
   spring.datasource.hikari.leak-detection-threshold=30000
   ```

## Debugging Tools

### Application Logs

Enable debug logging:
```properties
logging.level.com.example=DEBUG
logging.level.org.springframework.web=DEBUG
logging.level.org.hibernate.SQL=DEBUG
```

### JVM Monitoring

1. JConsole:
   ```bash
   jconsole <pid>
   ```

2. JVisualVM for profiling:
   ```bash
   jvisualvm
   ```

3. Thread dumps:
   ```bash
   jstack <pid> > thread-dump.txt
   ```

### Database Queries

Enable query logging:
```properties
spring.jpa.show-sql=true
spring.jpa.properties.hibernate.format_sql=true
logging.level.org.hibernate.type=TRACE
```

## Health Checks

### Application Health
```bash
curl http://localhost:8080/actuator/health
```

Response:
```json
{
  "status": "UP",
  "components": {
    "db": {"status": "UP"},
    "diskSpace": {"status": "UP"},
    "ping": {"status": "UP"}
  }
}
```

### Custom Health Indicators

```kotlin
@Component
class CacheHealthIndicator(
    private val cacheService: CacheService
) : HealthIndicator {
    override fun health(): Health {
        val hitRate = cacheService.getHitRate("user")
        return if (hitRate > 0.5) {
            Health.up()
                .withDetail("hitRate", hitRate)
                .build()
        } else {
            Health.down()
                .withDetail("hitRate", hitRate)
                .withDetail("message", "Cache hit rate below threshold")
                .build()
        }
    }
}
```

## Emergency Procedures

### High CPU Usage
1. Take thread dump
2. Check for infinite loops
3. Review recent deployments
4. Scale horizontally if needed

### Database Lockup
1. Identify blocking queries:
   ```sql
   SELECT * FROM pg_stat_activity WHERE wait_event_type = 'Lock';
   ```
2. Kill blocking session if necessary
3. Review transaction isolation levels

### Cache Corruption
1. Clear all caches:
   ```bash
   curl -X POST http://localhost:8080/admin/cache/clear
   ```
2. Monitor cache rebuild
3. Check for data inconsistencies

## Support Contacts

- On-call engineer: Use PagerDuty
- Database team: #database-support
- Infrastructure: #infrastructure
- Security incidents: security@example.com
"#;

    self.create_commit(
      repo_path,
      "(docs) Add comprehensive troubleshooting guide

Created a detailed troubleshooting guide to help developers and operations
teams quickly diagnose and resolve common issues.

The guide includes:
- Common problems with step-by-step solutions
- Debugging tools and techniques
- Performance troubleshooting procedures
- Emergency response procedures
- Health check implementations
- Support contact information

Each issue includes:
- Clear symptoms for identification
- Diagnostic steps to confirm the problem
- Multiple solution approaches
- Prevention strategies

This guide is based on real production issues and includes battle-tested
solutions that have proven effective in production environments.

The documentation follows a problem-solution format for quick reference
during incidents and includes code examples for implementation.",
      &[("docs/troubleshooting.md", troubleshooting)],
    )?;

    tracing::info!(
      "✅ Created pushed base conflict scenarios and documentation branch. \
       📊 Additional commits demonstrate conflicts with pushed base: \
       (sec) commits modify files from pushed commits, \
       (perf) commits also modify the same pushed files. \
       These will conflict differently than unpushed base. \
       (docs) commits create new files without conflicts"
    );

    Ok(())
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

    // Commit
    let output = Command::new("git").args(["--no-pager", "commit", "-m", message]).current_dir(repo_path).output()?;

    if !output.status.success() {
      anyhow::bail!("Git commit failed: {}", String::from_utf8_lossy(&output.stderr));
    }

    Ok(())
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use tempfile::TempDir;
  use test_log::test;

  #[test]
  fn test_generate_repo() {
    let temp_dir = TempDir::new().unwrap();
    let generator = TestRepoGenerator::new();

    generator.generate(temp_dir.path()).unwrap();

    // Verify repository was created in the working directory
    assert!(temp_dir.path().join("working").join(".git").exists());
    // Verify bare repository was created for origin
    assert!(temp_dir.path().join("origin.git").exists());
  }
}
