# ProGuard rules for Clarity Mobile.
# Keep the FFI bridge classes unobfuscated so that UniFFI-generated Kotlin
# can load the native library by name.
-keep class com.juice094.clarity.mobile.** { *; }
