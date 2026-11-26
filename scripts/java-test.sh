#!/usr/bin/env bash
# Run the JVM tests of the ported Android Java (crates/android/java-tests, Robolectric, no device).
# Usage: scripts/java-test.sh [gradle args...]   (default: test). See docs/android-tests.md.
# Needs Java 17+ to start Gradle and an Android SDK (ANDROID_HOME); Gradle fetches JDK 17 and android-37.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
# The CLI's pinned Gradle 9.8.2 wrapper, reused by path: the wrapper reads gradle-wrapper.properties next to its jar.
wrapper="$root/crates/cli/templates/android/gradle/wrapper/gradle-wrapper.jar"
java="${JAVA_HOME:+$JAVA_HOME/bin/}java"

[[ -n "${ANDROID_HOME:-}" ]] || { echo "error: ANDROID_HOME is not set (the Android SDK, e.g. ~/Android/Sdk)" >&2; exit 1; }

exec "$java" -cp "$wrapper" org.gradle.wrapper.GradleWrapperMain \
    --project-dir "$root/crates/android/java-tests" "${@:-test}"
