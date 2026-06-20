plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
}

android {
    namespace = "com.cortex.mobile"
    compileSdk = 35

    defaultConfig {
        applicationId = "com.cortex.mobile"
        minSdk = 26
        targetSdk = 35
        versionCode = 1
        versionName = "0.1.0"
    }

    buildTypes {
        debug {
            applicationIdSuffix = ".debug"
            isMinifyEnabled = false
        }
        release {
            isMinifyEnabled = false
            proguardFiles(
                getDefaultProguardFile("proguard-android-optimize.txt"),
                "proguard-rules.pro",
            )
        }
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
    kotlinOptions {
        jvmTarget = "17"
    }
}

// ── Single source of truth for the web client ───────────────────────────────
// The PWA web client (client.html + the vendored xterm bundle + the Cortex icon)
// lives in the Cortex Rust repo and is served live by the desktop bridge. We copy
// the exact same files into this app's assets at build time so the APK and the
// bridge can never drift. `rootProject` dir is `mobile/`; `../app` is `cortex/app`.
val syncWebAssets by tasks.registering(Copy::class) {
    val bridge = rootProject.file("../app/src/mobile_bridge")
    into(layout.buildDirectory.dir("generated/cortexAssets"))
    from(bridge) { include("client.html") }
    from(bridge.resolve("vendor")) { include("xterm.js", "xterm.css") }
    from(rootProject.file("../app/channels/oss/icon/no-padding/512x512.png")) {
        rename { "icon.png" }
    }
    from(rootProject.file("manifest.webmanifest"))
}

android.sourceSets["main"].assets.srcDir(layout.buildDirectory.dir("generated/cortexAssets"))
tasks.named("preBuild") { dependsOn(syncWebAssets) }

dependencies {
    implementation("androidx.core:core-ktx:1.13.1")
    implementation("androidx.appcompat:appcompat:1.7.0")
    implementation("androidx.activity:activity-ktx:1.9.3")
    implementation("androidx.webkit:webkit:1.12.1")
    implementation("com.journeyapps:zxing-android-embedded:4.3.0")
}
