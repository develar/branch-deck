package com.example.model

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
}