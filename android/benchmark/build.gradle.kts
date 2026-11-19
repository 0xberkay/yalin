// Microbenchmarks: yalin vs Gson, Moshi, kotlinx.serialization, Jackson, fastjson2,
// org.json on the same payloads.
// Run: ./gradlew :benchmark:connectedReleaseAndroidTest
plugins {
  alias(libs.plugins.android.library)
  alias(libs.plugins.androidx.benchmark)
  alias(libs.plugins.ksp)
  alias(libs.plugins.kotlin.serialization)
}

android {
  namespace = "dev.yalin.benchmark"
  compileSdk = 37

  defaultConfig {
    // Jackson uses MethodHandle (API 26+).
    minSdk = 26
    testInstrumentationRunner = "androidx.benchmark.junit4.AndroidBenchmarkRunner"
  }
  // Benchmarks must run non-debuggable; the test APK is signed with the debug key.
  testBuildType = "release"
  buildTypes {
    release {
      isDefault = true
      isMinifyEnabled = false
    }
  }
  compileOptions {
    sourceCompatibility = JavaVersion.VERSION_17
    targetCompatibility = JavaVersion.VERSION_17
  }
}

kotlin {
  jvmToolchain(17)
}

dependencies {
  androidTestImplementation(project(":yalin-android"))
  androidTestImplementation(libs.androidx.benchmark.junit4)
  androidTestImplementation(libs.androidx.test.runner)
  androidTestImplementation(libs.junit)
  androidTestImplementation(libs.gson)
  androidTestImplementation(libs.moshi)
  kspAndroidTest(libs.moshi.kotlin.codegen)
  androidTestImplementation(libs.kotlinx.serialization.json)
  androidTestImplementation(libs.jackson.module.kotlin)
  androidTestImplementation(libs.kotlin.reflect)
  androidTestImplementation(libs.fastjson2)
}
