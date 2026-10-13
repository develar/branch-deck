package com.example.service

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
}