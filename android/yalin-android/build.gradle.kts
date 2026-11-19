import org.gradle.internal.os.OperatingSystem

plugins {
  alias(libs.plugins.android.library)
}

/** Rust workspace root (contains Cargo.toml). */
val rustRoot: Directory = rootProject.layout.projectDirectory.dir("..")
val rustJniLibs: File = layout.buildDirectory.dir("rustJniLibs").get().asFile

android {
  namespace = "dev.yalin"
  compileSdk = 37
  ndkVersion = "28.2.13676358"

  defaultConfig {
    minSdk = 24
    testInstrumentationRunner = "androidx.test.runner.AndroidJUnitRunner"
    consumerProguardFiles("consumer-rules.pro")
  }
  compileOptions {
    sourceCompatibility = JavaVersion.VERSION_17
    targetCompatibility = JavaVersion.VERSION_17
  }
  sourceSets {
    // Same tests run on the host JVM (test) and on a device/emulator (androidTest).
    getByName("test") { kotlin.directories.add("src/sharedTest/kotlin") }
    getByName("androidTest") { kotlin.directories.add("src/sharedTest/kotlin") }
  }
}

kotlin {
  jvmToolchain(17)
}

dependencies {
  testImplementation(libs.junit)
  androidTestImplementation(libs.junit)
  androidTestImplementation(libs.androidx.test.runner)
}

// ---- libyalin.so via cargo -------------------------------------------------
// arm64-v8a is the shipping ABI; x86_64 exists so the library runs on emulators.

data class RustAbi(val abi: String, val triple: String)

val rustAbis = listOf(
  RustAbi("arm64-v8a", "aarch64-linux-android"),
  RustAbi("x86_64", "x86_64-linux-android"),
)
val minSdk = 24
val ndkHostTag = if (OperatingSystem.current().isMacOsX) "darwin-x86_64" else "linux-x86_64"
val ndkBin: Provider<String> = androidComponents.sdkComponents.ndkDirectory.map {
  it.dir("toolchains/llvm/prebuilt/$ndkHostTag/bin").asFile.path
}

val cargoBuildAndroid = rustAbis.map { target ->
  tasks.register<Exec>("cargoBuild-${target.abi}") {
    group = "rust"
    description = "Builds libyalin.so for ${target.abi}"
    workingDir(rustRoot)
    commandLine("cargo", "build", "-p", "yalin-jni", "--release", "--target", target.triple)
    val linkerVar = "CARGO_TARGET_${target.triple.uppercase().replace('-', '_')}_LINKER"
    environment(linkerVar, "${ndkBin.get()}/${target.triple}$minSdk-clang")
  }
}

val syncRustJniLibs = tasks.register<Sync>("syncRustJniLibs") {
  group = "rust"
  dependsOn(cargoBuildAndroid)
  rustAbis.forEach { target ->
    from(rustRoot.file("target/${target.triple}/release/libyalin.so")) { into(target.abi) }
  }
  into(rustJniLibs)
}

androidComponents.onVariants { variant ->
  variant.sources.jniLibs?.addStaticSourceDirectory(rustJniLibs.path)
}
tasks.named("preBuild") { dependsOn(syncRustJniLibs) }

// Host build so local unit tests exercise the real JNI layer on the JVM.
val cargoBuildHost = tasks.register<Exec>("cargoBuildHost") {
  group = "rust"
  workingDir(rustRoot)
  commandLine("cargo", "build", "-p", "yalin-jni")
}

tasks.withType<Test>().configureEach {
  dependsOn(cargoBuildHost)
  systemProperty("java.library.path", rustRoot.dir("target/debug").asFile.path)
}
