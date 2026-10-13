package com.example.service

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
)