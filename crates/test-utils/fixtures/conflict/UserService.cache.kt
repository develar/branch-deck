package com.example.service

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
)