// SPDX-License-Identifier: GPL-3.0-or-later
// The Android launcher for the remote: one module, no libraries. Gradle fetches only the
// Android Gradle plugin (google()) and its own dependencies (mavenCentral()) at build time.
pluginManagement {
    repositories {
        google()
        mavenCentral()
    }
}

dependencyResolutionManagement {
    repositoriesMode.set(RepositoriesMode.FAIL_ON_PROJECT_REPOS)
    repositories {
        google()
        mavenCentral()
    }
}

rootProject.name = "sursum-android"
