plugins {
    kotlin("jvm") version "1.9.20"
    id("org.springframework.boot") version "3.1.5"
}

dependencies {
    implementation("org.springframework.boot:spring-boot-starter-web")
    implementation("org.jetbrains.kotlin:kotlin-reflect")
    implementation("org.mindrot:jbcrypt:0.4")
}
