package com.example.service

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
)