import java.net.HttpURLConnection
import java.net.URI
import java.nio.file.Files
import java.nio.file.StandardCopyOption
import java.security.MessageDigest
import java.util.Properties
import javax.inject.Inject
import org.gradle.api.file.ArchiveOperations
import org.gradle.api.file.FileSystemOperations
import org.gradle.api.file.RelativePath
import org.jetbrains.kotlin.gradle.dsl.JvmTarget

plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
    id("rust")
}

val tauriProperties = Properties().apply {
    val propFile = file("tauri.properties")
    if (propFile.exists()) {
        propFile.inputStream().use { load(it) }
    }
}

val keystoreProperties = Properties().apply {
    val propFile = rootProject.file("keystore.properties")
    if (propFile.exists()) {
        propFile.inputStream().use { load(it) }
    }
}

val espeakBundle = Properties().apply {
    rootProject.file("espeak-android-bundle.properties").inputStream().use { load(it) }
}

abstract class PrepareEspeakAndroidLibraries : DefaultTask() {
    @get:Input
    abstract val url: Property<String>

    @get:Input
    abstract val sha256: Property<String>

    @get:Internal
    abstract val archive: RegularFileProperty

    @get:Internal
    abstract val jniLibs: DirectoryProperty

    @get:Inject
    abstract val files: FileSystemOperations

    @get:Inject
    abstract val archives: ArchiveOperations

    @TaskAction
    fun prepare() {
        val expected = sha256.get()
        val bundle = archive.get().asFile
        if (!bundle.isFile || digest(bundle) != expected) {
            bundle.parentFile.mkdirs()
            val partial = File(bundle.parentFile, bundle.name + ".part")
            val connection = URI(url.get()).toURL().openConnection() as HttpURLConnection
            connection.connectTimeout = 30_000
            connection.readTimeout = 60_000
            connection.instanceFollowRedirects = true
            try {
                if (connection.responseCode != 200) {
                    throw GradleException("eSpeak NG bundle download failed with status ${connection.responseCode}")
                }
                connection.inputStream.use { input -> partial.outputStream().use { input.copyTo(it) } }
            } finally {
                connection.disconnect()
            }
            val actual = digest(partial)
            if (actual != expected) {
                partial.delete()
                throw GradleException("eSpeak NG bundle sha256 is $actual, expected $expected")
            }
            Files.move(partial.toPath(), bundle.toPath(), StandardCopyOption.REPLACE_EXISTING, StandardCopyOption.ATOMIC_MOVE)
        }
        files.copy {
            from(archives.tarTree(archives.gzip(bundle))) {
                include("jniLibs/*/libttsespeak.so")
                eachFile {
                    relativePath = RelativePath(true, *relativePath.segments.drop(1).toTypedArray())
                }
                includeEmptyDirs = false
            }
            into(jniLibs)
        }
    }

    private fun digest(file: File): String {
        val hash = MessageDigest.getInstance("SHA-256")
        file.inputStream().use { input ->
            val buffer = ByteArray(64 * 1024)
            while (true) {
                val read = input.read(buffer)
                if (read < 0) break
                hash.update(buffer, 0, read)
            }
        }
        return hash.digest().joinToString("") { "%02x".format(it) }
    }
}

val prepareEspeakAndroidLibraries by tasks.registering(PrepareEspeakAndroidLibraries::class) {
    val repository = espeakBundle.getProperty("repository")
    val tag = espeakBundle.getProperty("tag")
    val asset = espeakBundle.getProperty("asset")
    url.set("https://github.com/$repository/releases/download/$tag/$asset")
    sha256.set(espeakBundle.getProperty("sha256"))
    archive.set(layout.buildDirectory.file("espeak-android-bundle/$tag/$asset"))
    jniLibs.set(layout.projectDirectory.dir("src/main/jniLibs"))
}

tasks.named("preBuild") {
    dependsOn(prepareEspeakAndroidLibraries)
}

val applicationIdOverride = providers
    .gradleProperty("lettuceApplicationId")
    .orElse(providers.environmentVariable("LETTUCE_ANDROID_APPLICATION_ID"))
    .orElse("com.lettuceai.app")
    .get()

android {
    compileSdk = 37
    namespace = "com.lettuceai.app"
    defaultConfig {
        manifestPlaceholders["usesCleartextTraffic"] = "false"
        applicationId = applicationIdOverride
        minSdk = 30
        targetSdk = 37
        versionCode = tauriProperties.getProperty("tauri.android.versionCode", "1").toInt()
        versionName = tauriProperties.getProperty("tauri.android.versionName", "1.0")
    }
    signingConfigs {
        if (!keystoreProperties.isEmpty) {
            create("release") {
                keyAlias = keystoreProperties.getProperty("keyAlias")
                keyPassword = keystoreProperties.getProperty("password")
                storeFile = file(keystoreProperties.getProperty("storeFile"))
                storePassword = keystoreProperties.getProperty("password")
            }
        }
    }
    buildTypes {
        getByName("debug") {
            signingConfigs.findByName("release")?.let { signingConfig = it }
            manifestPlaceholders["usesCleartextTraffic"] = "true"
            isDebuggable = true
            isJniDebuggable = true
            isMinifyEnabled = false
            packaging {
                jniLibs.keepDebugSymbols.add("*/arm64-v8a/*.so")
                jniLibs.keepDebugSymbols.add("*/armeabi-v7a/*.so")
                jniLibs.keepDebugSymbols.add("*/x86/*.so")
                jniLibs.keepDebugSymbols.add("*/x86_64/*.so")
            }
        }
        getByName("release") {
            signingConfig = signingConfigs.findByName("release")
            optimization {
               enable = true
            }
            proguardFiles(
                *fileTree(".") {
                  include("**/*.pro")
                  exclude("build/**")
                }.files.toTypedArray()
            )
        }
    }
    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_1_8
        targetCompatibility = JavaVersion.VERSION_1_8
    }
    buildFeatures {
        buildConfig = true
    }
}

kotlin {
    compilerOptions {
        jvmTarget = JvmTarget.JVM_1_8
    }
}

rust {
    rootDirRel = "../../../"
}

dependencies {
    implementation("androidx.webkit:webkit:1.14.0")
    implementation("androidx.appcompat:appcompat:1.7.1")
    implementation("androidx.activity:activity-ktx:1.10.1")
    implementation("com.google.android.material:material:1.12.0")
    implementation("androidx.lifecycle:lifecycle-process:2.10.0")
    implementation("com.microsoft.onnxruntime:onnxruntime-android:1.22.0")
    testImplementation("junit:junit:4.13.2")
    androidTestImplementation("androidx.test.ext:junit:1.1.4")
    androidTestImplementation("androidx.test.espresso:espresso-core:3.5.0")
}

apply(from = file("tauri.build.gradle.kts"))
