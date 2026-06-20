package com.adaasch.mdview

import android.app.Activity
import android.net.Uri
import android.os.Handler
import android.os.Looper
import android.util.Log
import java.util.concurrent.atomic.AtomicReference

/**
 * Bridge object between Rust and Kotlin. The Rust layer calls these static
 * methods via JNI. The Kotlin layer calls back into Rust via
 * `onFilePicked` (called from `MainActivity` after the file picker returns).
 *
 * The methods are all static so the Rust layer can call them without holding
 * a reference to an `Activity` (which would be a JNI global reference leak).
 *
 * The file picker is asynchronous on Android — the user picks a file in a
 * separate Activity and the result comes back via `ActivityResultContracts`.
 * Rust polls `pickFileResult` to get the result; `pickFile` returns null
 * immediately and launches the picker.
 */
object RustBridge {
    private const val TAG = "mdview/RustBridge"
    private const val CLASS_NAME = "com/adaasch/mdview/RustBridge"

    /** The currently active Activity (weak reference to avoid leaks). */
    private var activity: java.lang.ref.WeakReference<MainActivity>? = null

    /**
     * Pending file picker result. Set by the file picker callback
     * (`onFilePicked`), read by Rust via `pickFileResult`.
     *
     * `None` = no result yet (or already consumed).
     * `Some(None)` = picker was cancelled.
     * `Some(Some(uri))` = picker returned a URI.
     */
    private val pendingPickResult = AtomicReference<String?>(null)

    /**
     * Pending intent data URI. Set by `MainActivity.onCreate` and
     * `MainActivity.onNewIntent` when a new intent with data is delivered,
     * read by Rust via `consumeIntentData`.
     */
    private val pendingIntentData = AtomicReference<String?>(null)

    /**
     * Pending folder picker result (a granted tree URI). Set by the folder
     * picker callback in `MainActivity`, read by Rust via `pickFolderResult`.
     */
    private val pendingFolderResult = AtomicReference<String?>(null)

    /** The main-thread Handler used to marshal file picker launches. */
    private val mainHandler = Handler(Looper.getMainLooper())

    /** Set the current Activity. Called from MainActivity.onCreate / onDestroy. */
    fun setActivity(activity: MainActivity?) {
        if (activity == null) {
            this.activity = null
        } else {
            this.activity = java.lang.ref.WeakReference(activity)
        }
    }

    /**
     * Called by Rust to launch the file picker. Returns null immediately;
     * the actual result is delivered via `pickFileResult`.
     */
    @JvmStatic
    fun pickFile(mimeType: String): String? {
        val activity = activity?.get() ?: run {
            Log.w(TAG, "pickFile called but no Activity is set")
            pendingPickResult.set(null)
            return null
        }
        // Reset pending result before launching.
        pendingPickResult.set(null)
        // Launch the file picker on the main thread.
        mainHandler.post {
            try {
                activity.launchFilePicker(mimeType)
            } catch (e: Exception) {
                Log.e(TAG, "Failed to launch file picker: ${e.message}", e)
                pendingPickResult.set(null)
            }
        }
        return null
    }

    /**
     * Called by Rust to poll for the file picker result. Returns the URI
     * string if a file was picked, null otherwise (no result yet or cancelled).
     *
     * After this call the pending result is cleared.
     */
    @JvmStatic
    fun pickFileResult(): String? {
        return pendingPickResult.getAndSet(null)
    }

    /**
     * Called by MainActivity to set the pending file picker result.
     * Used both for the file picker callback and for the initial intent
     * data URI (so the Rust layer can pick it up via `pickFileResult`).
     */
    fun setPendingPickResult(uriString: String?) {
        pendingPickResult.set(uriString)
    }

    /**
     * Called by Rust to launch the folder picker. Returns null immediately;
     * the granted tree URI is delivered via `pickFolderResult`.
     */
    @JvmStatic
    fun pickFolder(): String? {
        val activity = activity?.get() ?: run {
            Log.w(TAG, "pickFolder called but no Activity is set")
            pendingFolderResult.set(null)
            return null
        }
        pendingFolderResult.set(null)
        mainHandler.post {
            try {
                activity.launchFolderPicker()
            } catch (e: Exception) {
                Log.e(TAG, "Failed to launch folder picker: ${e.message}", e)
                pendingFolderResult.set(null)
            }
        }
        return null
    }

    /** Called by Rust to poll for the folder picker result (a tree URI). */
    @JvmStatic
    fun pickFolderResult(): String? {
        return pendingFolderResult.getAndSet(null)
    }

    /** Called by MainActivity to set the pending folder picker result. */
    fun setPendingFolderResult(uriString: String?) {
        pendingFolderResult.set(uriString)
    }

    /**
     * Called by Rust to list markdown/text files within a granted folder tree.
     * Returns the tree-relative paths, one per line, or null on failure.
     */
    @JvmStatic
    fun listTreeMarkdown(treeUri: String): String? {
        return activity?.get()?.listTreeMarkdownImpl(treeUri)
    }

    /**
     * Called by Rust to resolve a tree-relative path (e.g. `sub/other.md`) to a
     * document URI within a granted folder tree. Returns the URI string, or
     * null if the file isn't found.
     */
    @JvmStatic
    fun resolveTreePath(treeUri: String, relPath: String): String? {
        return activity?.get()?.resolveTreePathImpl(treeUri, relPath)
    }

    /**
     * Called by MainActivity to set the pending intent data URI.
     * This is called from both `onCreate` and `onNewIntent` so that
     * "open with" intents are delivered to the Rust layer even when the
     * activity is already running.
     */
    fun setPendingIntentData(uriString: String?) {
        pendingIntentData.set(uriString)
    }

    /**
     * Called by Rust to read the bytes of a URI.
     * Returns the bytes, or null on failure.
     */
    @JvmStatic
    fun readUri(uriString: String): ByteArray? {
        val activity = activity?.get() ?: run {
            Log.w(TAG, "readUri called but no Activity is set")
            return null
        }
        return activity.readUriBytes(uriString)
    }

    /**
     * Called by Rust to get the intent's data URI string.
     */
    @JvmStatic
    fun intentDataString(): String? {
        val activity = activity?.get() ?: return null
        return activity.getIntentDataString()
    }

    /**
     * Called by Rust to consume the pending intent data URI. Returns the
     * URI string and clears it so it's only consumed once.
     */
    @JvmStatic
    fun consumeIntentData(): String? {
        return pendingIntentData.getAndSet(null)
    }

    /**
     * Called by Rust to get the app's files directory.
     */
    @JvmStatic
    fun filesDir(): String? {
        val activity = activity?.get() ?: return null
        return activity.getFilesDirPath()
    }

    /**
     * Called by Rust to open an external URL.
     */
    @JvmStatic
    fun openExternal(url: String): String {
        val activity = activity?.get() ?: return "fail"
        return activity.openExternalUrl(url)
    }

    init {
        Log.i(TAG, "RustBridge loaded")
    }
}