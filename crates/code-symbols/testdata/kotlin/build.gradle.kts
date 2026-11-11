// A build script: its statements stand at the top level, where a line break ends each
import org.gradle.api.tasks.testing.Test

plugins {
  kotlin("jvm") version "2.2.0"
  id("application")
}

val ktorVersion = "3.1.0"
val isCi = System.getenv("CI") != null

group = "com.example"
version = if (isCi) "1.0.0" else "1.0.0-SNAPSHOT"

repositories {
  mavenCentral()
}

dependencies {
  implementation("io.ktor:ktor-server-core:$ktorVersion")
  implementation(
    "io.ktor:ktor-server-netty:${ktorVersion}",
  )
  testImplementation(kotlin("test"))
}

fun Test.configureLogging(showStreams: Boolean) {
  testLogging {
    showStandardStreams = showStreams
    events("passed", "failed")
  }
}

tasks.withType<Test>().configureEach {
  useJUnitPlatform()
  configureLogging(showStreams = !isCi)
  maxParallelForks = (Runtime.getRuntime().availableProcessors() / 2)
    .coerceAtLeast(1)
}

/** Prints the version: a task a script declares by a call. */
tasks.register("printVersion") {
  doLast {
    println(
      """
      version:
        $version
      """.trimIndent()
    )
  }
}

application.mainClass.set("com.example.MainKt")
