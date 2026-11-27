# SPDX-License-Identifier: GPL-3.0-or-later
# Build host for the Android launcher (android/): a JDK, the Android SDK command-line tools with
# one platform and one build-tools version, and Gradle. Nothing Android is installed on the
# developer's machine; `just android` builds the APK in here. Versions were checked on
# developer.android.com and gradle.org (2026-11-25); the downloads are verified by SHA-256.
# Building in this container accepts the Android SDK licence on the user's behalf
# (`sdkmanager --licenses`).
FROM docker.io/library/debian:trixie

RUN apt-get update \
 && apt-get install -y --no-install-recommends ca-certificates curl unzip openjdk-21-jdk-headless \
 && rm -rf /var/lib/apt/lists/*

# https://developer.android.com/studio#command-line-tools-only
ARG CMDLINE_TOOLS=commandlinetools-linux-15859902_latest.zip
ARG CMDLINE_TOOLS_SHA256=4e4c464f145a7512b57d088ac6c278c03c9eea610886b35a5e0804e74eedf583
# https://gradle.org/release-checksums/ (AGP 9.5 needs Gradle 9.6.0 or later)
ARG GRADLE_VERSION=9.9.2
ARG GRADLE_SHA256=971fcd7150ca4ba04852c0b2ca0d2d0ab9468e14b03473eab19528809914c76b
# Must match compileSdk and buildToolsVersion in app/build.gradle.kts.
ARG PLATFORM=android-37.0
ARG BUILD_TOOLS=36.0.0

ENV ANDROID_HOME=/opt/android-sdk
ENV PATH=/opt/gradle/bin:/opt/android-sdk/cmdline-tools/latest/bin:$PATH

RUN curl -fsSLo /tmp/tools.zip "https://dl.google.com/android/repository/$CMDLINE_TOOLS" \
 && echo "$CMDLINE_TOOLS_SHA256  /tmp/tools.zip" | sha256sum -c - \
 && mkdir -p "$ANDROID_HOME/cmdline-tools" \
 && unzip -q /tmp/tools.zip -d "$ANDROID_HOME/cmdline-tools" \
 && mv "$ANDROID_HOME/cmdline-tools/cmdline-tools" "$ANDROID_HOME/cmdline-tools/latest" \
 && rm /tmp/tools.zip \
 && yes | sdkmanager --licenses > /dev/null \
 && sdkmanager --install "platforms;$PLATFORM" "build-tools;$BUILD_TOOLS" platform-tools > /dev/null \
 && chmod -R a+rX "$ANDROID_HOME"

RUN curl -fsSLo /tmp/gradle.zip "https://services.gradle.org/distributions/gradle-$GRADLE_VERSION-bin.zip" \
 && echo "$GRADLE_SHA256  /tmp/gradle.zip" | sha256sum -c - \
 && unzip -q /tmp/gradle.zip -d /opt \
 && mv "/opt/gradle-$GRADLE_VERSION" /opt/gradle \
 && rm /tmp/gradle.zip

WORKDIR /src/android
