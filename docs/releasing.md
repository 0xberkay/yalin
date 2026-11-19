# Releasing Yalın for Android

The `:yalin-android` module publishes the `release` AAR, Kotlin sources, Dokka API documentation and POM as `io.github.0xberkay:yalin-android`. The sample app and benchmark modules are development tools.

## Verify the publication locally

From `android/`, run:

```sh
./gradlew :yalin-android:publishToMavenLocal
```

Gradle installs the artifacts under `~/.m2/repository/io/github/0xberkay/yalin-android/`. To use them in a local consumer, add `mavenLocal()` to its dependency repositories and declare the version in the library’s `coordinates(...)` block.

## Publish to Maven Central

1. Sign in to the [Central Portal](https://central.sonatype.com/) and verify the `io.github.0xberkay` [namespace](https://central.sonatype.org/register/namespace/).
2. Generate a [Portal token](https://central.sonatype.org/publish/generate-portal-token/) and configure `mavenCentralUsername` and `mavenCentralPassword` in `~/.gradle/gradle.properties` or as `ORG_GRADLE_PROJECT_` environment variables.
3. Configure a [Gradle signing key](https://vanniktech.github.io/gradle-maven-publish-plugin/central/#secrets) whose public key is available to Maven Central. From `android/`, stage the release with:

   ```sh
   ./gradlew :yalin-android:publishToMavenCentral
   ```

4. Review the validated deployment in the Central Portal and publish it. After the coordinates resolve from Maven Central, tag the corresponding source commit and create the GitHub release.
