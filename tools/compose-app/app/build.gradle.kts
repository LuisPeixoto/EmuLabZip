plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
    id("org.jetbrains.kotlin.plugin.compose")
}

android {
    namespace = "dev.emulabzip.composedemo"
    compileSdk = 36

    defaultConfig {
        applicationId = "dev.emulabzip.composedemo"
        minSdk = 26
        targetSdk = 36
        versionCode = 1
        versionName = "1.0"
    }
    buildTypes {
        // A release build with R8, so the names in a crash are scrambled. Signed with the debug key, only to try it.
        release {
            isMinifyEnabled = true
            proguardFiles(getDefaultProguardFile("proguard-android-optimize.txt"), "proguard-rules.pro")
            signingConfig = signingConfigs.getByName("debug")
        }
    }
    // A demo: the release lint needs more downloads than the app is worth.
    lint { checkReleaseBuilds = false }
    buildFeatures { compose = true }
    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
    kotlinOptions { jvmTarget = "17" }
}

dependencies {
    val bom = platform("androidx.compose:compose-bom:2025.06.00")
    implementation(bom)
    implementation("androidx.activity:activity-compose:1.10.1")
    implementation("androidx.compose.material3:material3")
    implementation("androidx.compose.foundation:foundation")
    implementation("androidx.compose.ui:ui")
    // What the Data demo keeps: a Preferences DataStore and MMKV, beside SQLite and SharedPreferences.
    implementation("androidx.datastore:datastore-preferences:1.1.1")
    implementation("com.tencent:mmkv:1.3.9")
}
