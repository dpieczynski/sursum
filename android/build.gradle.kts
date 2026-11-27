// SPDX-License-Identifier: GPL-3.0-or-later
// Versions checked on developer.android.com (2026-11-25): AGP 9.5 (Gradle 9.6+, JDK 17+, API
// levels up to 37); 9.5.3 is its latest patch. The platform and build tools match
// android/build.Containerfile.
plugins {
    id("com.android.application") version "9.5.3"
}

// Build output under the repository's target/, like everything else, not in the source tree.
layout.buildDirectory = file("../target/android/build")

android {
    namespace = "io.github.dpieczynski.sursum"
    compileSdk = 37
    buildToolsVersion = "36.0.0"
    // Plain Java: AGP 9 would otherwise add the Kotlin standard library to the APK.
    enableKotlin = false

    defaultConfig {
        applicationId = "io.github.dpieczynski.sursum"
        // Android 7.0. The remote needs a current WebView (updated through the Play Store),
        // not a current Android; old parish tablets should still install the app.
        minSdk = 24
        // Not 37 yet: apps targeting Android 17 need the ACCESS_LOCAL_NETWORK runtime
        // permission to reach the device (developer.android.com/privacy-and-security/
        // local-network-permission); below 37 the INTERNET permission grants it.
        targetSdk = 36
        versionCode = 1
        versionName = "0.1.0"
    }

    // The release key (`just android-key`) is the key Google Play signs with (uploaded to Play
    // App Signing) and the one registered for developer verification. Parish tablets only ever
    // get builds signed with it: an app signed with another key cannot update it, and
    // uninstalling loses the pairing. Without the variables, release builds stay unsigned.
    val keystore = System.getenv("SURSUM_ANDROID_KEYSTORE")
    if (keystore != null) {
        signingConfigs {
            create("release") {
                storeFile = file(keystore)
                storeType = "PKCS12"
                storePassword = System.getenv("SURSUM_ANDROID_KEYSTORE_PASSWORD")
                keyAlias = "sursum"
                keyPassword = System.getenv("SURSUM_ANDROID_KEYSTORE_PASSWORD")
            }
        }
        buildTypes.getByName("release").signingConfig = signingConfigs.getByName("release")
    }

    lint {
        // Deliberate: see targetSdk above.
        disable += "OldTargetApi"
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
}
