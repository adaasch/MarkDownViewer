package eu.io_com.mdview

import android.app.Activity
import android.content.Intent
import android.net.Uri
import android.os.Build
import android.os.Bundle
import android.provider.DocumentsContract
import android.provider.OpenableColumns
import android.util.Log
import androidx.activity.result.contract.ActivityResultContracts
import com.google.androidgamesdk.GameActivity


/**
 * The main (and only) Activity. It hosts the native eframe/winit view via
 * `androidx.gamesdk.activity.GameActivity` which loads `libmdview.so` via
 * `System.loadLibrary("mdview")` and calls `android_main` on a dedicated thread.
 *
 * On startup we:
 *   1. Forward the launching intent to the Rust layer via `RustBridge`.
 *   2. Register a `pickFile` launcher so the Rust layer can request a file
 *      picker via `RustBridge.pickFile()`.
 *   3. Forward the picked URI back to Rust via `RustBridge.pickFileResult()`.
 *
 * The Rust layer calls the static methods of `RustBridge` to:
 *   - Read a URI's bytes (`readUri`).
 *   - Pick a file (`pickFile`).
 *   - Get the file picker result (`pickFileResult`).
 *   - Get the intent data string (`intentDataString`).
 *   - Get the app's files dir (`filesDir`).
 *   - Open an external URL (`openExternal`).
 */
class MainActivity : GameActivity() {
    companion object {
        private const val TAG = "mdview/MainActivity"
    }

    /** File picker launcher. */
    private val pickFileLauncher =
        registerForActivityResult(ActivityResultContracts.OpenDocument()) { uri: Uri? ->
            if (uri == null) {
                Log.i(TAG, "User cancelled file picker")
                RustBridge.setPendingPickResult(RustBridge.PICKER_CANCELLED)
                return@registerForActivityResult
            }
            // Persist permission so we can read it later.
            try {
                contentResolver.takePersistableUriPermission(
                    uri,
                    Intent.FLAG_GRANT_READ_URI_PERMISSION,
                )
            } catch (e: SecurityException) {
                Log.w(TAG, "Failed to take persistable URI permission: ${e.message}")
            }
            RustBridge.setPendingPickResult(uri.toString())
        }

    /** Folder picker launcher (grants access to a whole directory tree). */
    private val pickFolderLauncher =
        registerForActivityResult(ActivityResultContracts.OpenDocumentTree()) { uri: Uri? ->
            if (uri == null) {
                Log.i(TAG, "User cancelled folder picker")
                RustBridge.setPendingFolderResult(RustBridge.PICKER_CANCELLED)
                return@registerForActivityResult
            }
            // Persist read permission for the whole tree so links between files
            // (and reopening) keep working across sessions.
            try {
                contentResolver.takePersistableUriPermission(
                    uri,
                    Intent.FLAG_GRANT_READ_URI_PERMISSION,
                )
            } catch (e: SecurityException) {
                Log.w(TAG, "Failed to take persistable tree permission: ${e.message}")
            }
            RustBridge.setPendingFolderResult(uri.toString())
        }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        // Set up the JNI bridge so the Rust layer can call into Kotlin.
        RustBridge.setActivity(this)
        // Deliberately *not* queueing the launching intent for
        // `consumeIntentData` here. `android_main` reads it directly via
        // `intentDataString()` when it builds the app. Queueing it as well
        // meant the launching document was read, cached and navigated to
        // twice — once at startup and again on the first frame — which left a
        // spurious second entry in the back history. Only `onNewIntent`, which
        // arrives after the app is already running, needs the queue.
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        Log.i(TAG, "onNewIntent: ${intent.data}")
        // Update the activity's intent so getIntent() returns the latest one.
        setIntent(intent)
        // Forward the new intent's URI to the Rust layer.
        uriFromIntent(intent)?.let { RustBridge.setPendingIntentData(it) }
    }

    /**
     * Extract the document URI an intent refers to.
     *
     * `ACTION_VIEW` puts it in the intent's data, but `ACTION_SEND` — which
     * this activity also advertises in its manifest — puts it in the
     * `EXTRA_STREAM` extra instead. Reading only `intent.data` meant sharing a
     * file to mdview opened an empty window.
     */
    private fun uriFromIntent(intent: Intent?): String? {
        if (intent == null) return null
        intent.data?.let { return it.toString() }
        if (intent.action == Intent.ACTION_SEND) {
            val stream: Uri? =
                if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
                    intent.getParcelableExtra(Intent.EXTRA_STREAM, Uri::class.java)
                } else {
                    @Suppress("DEPRECATION")
                    intent.getParcelableExtra(Intent.EXTRA_STREAM)
                }
            return stream?.toString()
        }
        return null
    }

    override fun onDestroy() {
        RustBridge.setActivity(null)
        super.onDestroy()
    }

    /**
     * Launch the system file picker.
     */
    fun launchFilePicker(mimeType: String) {
        try {
            pickFileLauncher.launch(arrayOf(mimeType))
        } catch (e: Exception) {
            Log.e(TAG, "Failed to launch file picker: ${e.message}", e)
            RustBridge.setPendingPickResult(null)
        }
    }

    /**
     * Launch the system folder picker.
     */
    fun launchFolderPicker() {
        try {
            pickFolderLauncher.launch(null)
        } catch (e: Exception) {
            Log.e(TAG, "Failed to launch folder picker: ${e.message}", e)
            RustBridge.setPendingFolderResult(null)
        }
    }

    /**
     * List markdown/text files within a granted folder tree, returning each
     * file's path relative to the tree root, one per line. Walks subdirectories
     * (bounded in depth and count to stay responsive on large trees).
     */
    fun listTreeMarkdownImpl(treeUriString: String): String? {
        return try {
            val treeUri = Uri.parse(treeUriString)
            val rootId = DocumentsContract.getTreeDocumentId(treeUri)
            val out = StringBuilder()
            walkTree(treeUri, rootId, "", out, 0)
            out.toString()
        } catch (e: Exception) {
            Log.e(TAG, "Failed to list tree $treeUriString: ${e.message}")
            null
        }
    }

    private fun walkTree(
        treeUri: Uri,
        parentDocId: String,
        prefix: String,
        out: StringBuilder,
        depth: Int,
    ) {
        if (depth > 8 || out.length > 256_000) return
        val childrenUri =
            DocumentsContract.buildChildDocumentsUriUsingTree(treeUri, parentDocId)
        contentResolver.query(
            childrenUri,
            arrayOf(
                DocumentsContract.Document.COLUMN_DOCUMENT_ID,
                DocumentsContract.Document.COLUMN_DISPLAY_NAME,
                DocumentsContract.Document.COLUMN_MIME_TYPE,
            ),
            null, null, null,
        )?.use { c ->
            while (c.moveToNext()) {
                val id = c.getString(0)
                val name = c.getString(1) ?: continue
                val mime = c.getString(2)
                val rel = if (prefix.isEmpty()) name else "$prefix/$name"
                if (mime == DocumentsContract.Document.MIME_TYPE_DIR) {
                    walkTree(treeUri, id, rel, out, depth + 1)
                } else if (isMarkdownLike(name)) {
                    out.append(rel).append('\n')
                }
            }
        }
    }

    private fun isMarkdownLike(name: String): Boolean {
        val lower = name.lowercase()
        return listOf(
            ".md", ".markdown", ".mdown", ".mkd", ".mkdn",
            ".txt", ".text",
        ).any { lower.endsWith(it) }
    }

    /**
     * Resolve a tree-relative path (e.g. `sub/other.md`) to a document URI
     * within a granted folder tree, by walking the tree one path segment at a
     * time and matching display names. Returns null if not found.
     */
    fun resolveTreePathImpl(treeUriString: String, relPath: String): String? {
        return try {
            val treeUri = Uri.parse(treeUriString)
            var currentId = DocumentsContract.getTreeDocumentId(treeUri)
            val segments = relPath.split('/').filter { it.isNotEmpty() && it != "." }
            for (seg in segments) {
                val childId = findChildByName(treeUri, currentId, seg) ?: return null
                currentId = childId
            }
            DocumentsContract.buildDocumentUriUsingTree(treeUri, currentId).toString()
        } catch (e: Exception) {
            Log.e(TAG, "Failed to resolve $relPath in $treeUriString: ${e.message}")
            null
        }
    }

    private fun findChildByName(treeUri: Uri, parentDocId: String, name: String): String? {
        val childrenUri =
            DocumentsContract.buildChildDocumentsUriUsingTree(treeUri, parentDocId)
        contentResolver.query(
            childrenUri,
            arrayOf(
                DocumentsContract.Document.COLUMN_DOCUMENT_ID,
                DocumentsContract.Document.COLUMN_DISPLAY_NAME,
            ),
            null, null, null,
        )?.use { c ->
            while (c.moveToNext()) {
                if (c.getString(1) == name) {
                    return c.getString(0)
                }
            }
        }
        return null
    }

    /**
     * Read the bytes of a URI (file:// or content://).
     * Returns null on failure.
     */
    fun readUriBytes(uriString: String): ByteArray? {
        return try {
            val uri = Uri.parse(uriString)
            when (uri.scheme?.lowercase()) {
                "file" -> {
                    val path = uri.path
                    if (path != null) {
                        java.io.File(path).readBytes()
                    } else {
                        null
                    }
                }
                "content", null -> {
                    contentResolver.openInputStream(uri)?.use { it.readBytes() }
                }
                else -> {
                    contentResolver.openInputStream(uri)?.use { it.readBytes() }
                }
            }
        } catch (e: Exception) {
            Log.e(TAG, "Failed to read URI $uriString: ${e.message}")
            null
        }
    }

    /**
     * Get the launching intent's URI (for "open with" and "share to" support).
     */
    fun getIntentDataString(): String? = uriFromIntent(intent)

    /**
     * Look up a document's human-readable display name.
     *
     * Without this the app titles the document with the tail of its URI, which
     * for a SAF document is a percent-encoded ID like
     * `primary%3ADocuments%2Fnotes.md`.
     */
    fun displayNameImpl(uriString: String): String? {
        return try {
            val uri = Uri.parse(uriString)
            contentResolver.query(
                uri,
                arrayOf(OpenableColumns.DISPLAY_NAME),
                null, null, null,
            )?.use { c ->
                if (c.moveToFirst()) c.getString(0) else null
            }
        } catch (e: Exception) {
            Log.w(TAG, "Failed to read display name for $uriString: ${e.message}")
            null
        }
    }

    /**
     * Get the app's files directory (for caching).
     */
    fun getFilesDirPath(): String? {
        return filesDir?.absolutePath
    }

    /**
     * Open an external URL via the system browser.
     * Returns "ok" on success, "fail" on failure.
     */
    fun openExternalUrl(url: String): String {
        return try {
            val intent = Intent(Intent.ACTION_VIEW, Uri.parse(url))
            intent.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
            startActivity(intent)
            "ok"
        } catch (e: Exception) {
            Log.e(TAG, "Failed to open external URL $url: ${e.message}")
            "fail"
        }
    }
}