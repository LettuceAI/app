# Add project specific ProGuard rules here.
# You can control the set of applied configuration files using the
# proguardFiles setting in build.gradle.
#
# For more details, see
#   http://developer.android.com/guide/developing/tools/proguard.html

# If your project uses WebView with JS, uncomment the following
# and specify the fully qualified class name to the JavaScript interface
# class:
#-keepclassmembers class fqcn.of.javascript.interface.for.webview {
#   public *;
#}

# Uncomment this to preserve the line number information for
# debugging stack traces.
#-keepattributes SourceFile,LineNumberTable

# If you keep the line number information, uncomment this to
# hide the original source file name.
#-renamesourcefileattribute SourceFile

# Kokoro phonemizer bridge: invoked from Rust via JNI, so R8 has no static
# reference to it and would otherwise strip/inline the static `resolve`
# method that the Rust side calls.
-keep class com.lettuceai.app.KokoroPhonemizerBridge { *; }
-keepclassmembers class com.lettuceai.app.KokoroPhonemizerBridge {
  public static *;
}