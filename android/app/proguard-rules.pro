# ProGuard / R8 rules for the MarkDownViewer Android app.
#
# This file is referenced from `app/build.gradle` via:
#     proguardFiles getDefaultProguardFile('proguard-android-optimize.txt'), 'proguard-rules.pro'
#
# Without these rules, R8 (the release-build code shrinker) renames and
# removes the `RustBridge` class and its `@JvmStatic` methods because they
# are only referenced from Rust via JNI. R8 cannot see JNI references, so it
# thinks the class is unused and strips it. That breaks the file picker
# (and every other JNI bridge call) at runtime with a `ClassNotFoundException`
# when Rust calls `find_class("eu/io_com/mdview/RustBridge")`.

# ---------------------------------------------------------------------------
# JNI bridge: RustBridge
# ---------------------------------------------------------------------------
# The Rust layer (`src/android_shim.rs`) looks up this class by its fully
# qualified name and calls the following `@JvmStatic` methods via JNI:
#
#   pickFile(String) -> String
#   pickFileResult() -> String
#   readUri(String) -> byte[]
#   intentDataString() -> String
#   consumeIntentData() -> String
#   filesDir() -> String
#   openExternal(String) -> String
#
# We must:
#   1. Keep the class name itself (R8 would otherwise rename it to e.g.
#      `T.a`).
#   2. Keep every `@JvmStatic` method (R8 would otherwise remove them all).
#   3. Keep the `INSTANCE` field (Kotlin `object` requires it for static
#      initialisation) and the `Companion` field if present.
#   4. Keep the static fields used by MainActivity (`pendingPickResult`,
#      `pendingIntentData`, `mainHandler`, `activity`).
#   5. Keep the instance methods called from MainActivity (`setActivity`,
#      `setPendingPickResult`, `setPendingIntentData`) — although these
#      are inlined into MainActivity by R8, keeping them is harmless and
#      protects against future R8 inlining decisions.
-keep class eu.io_com.mdview.RustBridge {
    public static <fields>;
    private static <fields>;
    public static <methods>;
    private static <methods>;
}
-keepclassmembers class eu.io_com.mdview.RustBridge {
    *** Companion;
}
-keepclasseswithmembernames class eu.io_com.mdview.RustBridge {
    native <methods>;
}

# ---------------------------------------------------------------------------
# JNI bridge: MainActivity (instance methods called by RustBridge)
# ---------------------------------------------------------------------------
# MainActivity is already kept by the framework (it's the launcher
# activity in AndroidManifest.xml), but its non-overridden methods
# (`launchFilePicker`, `readUriBytes`, `getIntentDataString`,
# `consumeIntentData`, `getFilesDirPath`, `openExternalUrl`) are only
# called from `RustBridge` and would be stripped if we did not keep them.
# Keeping the class and all members is safe because MainActivity is
# already kept by the manifest.
-keep class eu.io_com.mdview.MainActivity {
    public <fields>;
    private <fields>;
    public <methods>;
    private <methods>;
}

# ---------------------------------------------------------------------------
# Keep all `@JvmStatic` methods on any class in our package.
# ---------------------------------------------------------------------------
# This is a defence-in-depth rule in case a future refactor adds another
# `@JvmStatic` JNI-callable helper class without updating this file.
-keepclassmembers class eu.io_com.mdview.** {
    @android.annotation.Keep public static <methods>;
    @android.annotation.Keep private static <methods>;
    @kotlin.jvm.JvmStatic public static <methods>;
    @kotlin.jvm.JvmStatic private static <methods>;
}