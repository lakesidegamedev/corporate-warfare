#!/bin/sh
# Minimal Gradle wrapper launcher for MOVGame KMP (Phase 0 scaffold).
# Regenerate the canonical script any time with: gradle wrapper --gradle-version 8.14.5
set -e
APP_HOME=$(cd "$(dirname "$0")" && pwd -P)
WRAPPER_JAR="$APP_HOME/gradle/wrapper/gradle-wrapper.jar"
if [ ! -f "$WRAPPER_JAR" ]; then
    echo "Missing $WRAPPER_JAR" >&2
    exit 1
fi
if [ -n "$JAVA_HOME" ]; then
    JAVA_EXE="$JAVA_HOME/bin/java"
else
    JAVA_EXE="java"
fi
command -v "$JAVA_EXE" >/dev/null 2>&1 || { echo "No JVM found (JAVA_HOME=$JAVA_HOME)" >&2; exit 1; }
exec "$JAVA_EXE" -classpath "$WRAPPER_JAR" org.gradle.wrapper.GradleWrapperMain "$@"
