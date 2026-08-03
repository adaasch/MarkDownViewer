//! Android-only JNI bridge between the Rust renderer and the Java/Kotlin side
//! of the application.
//!
//! The Java side (`MainActivity.kt` / `RustBridge.kt`) owns a `RustBridge`
//! object with static methods that this module calls into via JNI. The
//! methods cover:
//!
//! - `pickFile()` / `pickFileResult()` — launch the SAF file picker and poll
//!   for the picked URI.
//! - `pickFolder()` / `pickFolderResult()` — launch the SAF folder picker and
//!   poll for the granted tree URI.
//! - `listTreeMarkdown(tree)` — list the readable documents in a granted tree.
//! - `resolveTreePath(tree, rel)` — resolve a tree-relative path to a document
//!   URI.
//! - `openExternal(uri)` — dispatch an `Intent.ACTION_VIEW` for `uri`.
//! - `readUri(uri)` — read the bytes at `uri` via `ContentResolver`.
//! - `displayName(uri)` — the document's human-readable name.
//! - `intentDataString()` / `consumeIntentData()` — the URI the activity was
//!   launched with, and any URI delivered later via `onNewIntent`.
//! - `filesDir()` — return the activity's `getFilesDir()` absolute path.
//!
//! All public functions in this module are no-ops until [`init`] has been
//! called from `android_main` with a valid `AndroidApp`.

#![cfg(target_os = "android")]

use android_activity::AndroidApp;
use std::sync::OnceLock;

/// Sentinel returned by [`pick_file_result`] / [`pick_folder_result`] when the
/// user dismissed the picker without choosing anything.
///
/// A cancelled picker has to be distinguishable from "no answer yet" — both
/// used to arrive as `None`, so the Rust side could never clear its in-flight
/// flag and a single cancel left the picker button disabled and a repaint timer
/// spinning for the rest of the session. The Kotlin side sets this empty string
/// on cancel; an empty string can never be a real URI.
pub const PICKER_CANCELLED: &str = "";

/// Fully-qualified Java class name of the Kotlin bridge that the Java side
/// implements. The class lives in the `eu.io_com.mdview` Kotlin package.
const BRIDGE_CLASS: &str = "eu/io_com/mdview/RustBridge";

/// Binary (dot-separated) form of [`BRIDGE_CLASS`], required by the
/// class-loader based lookup in [`bridge_class`].
const BRIDGE_CLASS_BINARY: &str = "eu.io_com.mdview.RustBridge";

/// Cached state captured at startup. The JavaVM pointer is valid for the
/// lifetime of the process.
struct Bridge {
    vm: jni::JavaVM,
    /// The application class loader, captured from the launching `Activity`.
    ///
    /// `android_main` — and therefore every JNI call in this module — runs on a
    /// thread spawned by the GameActivity glue rather than by the JVM. JNI's
    /// `FindClass` resolves against the *system* class loader on such threads,
    /// which only knows bootstrap classes; it cannot find application classes
    /// like `RustBridge`. We capture the Activity's own (application) class
    /// loader here and use it to look up `RustBridge` for every call. Without
    /// it, every bridge call fails silently (no file picker, no "open with").
    loader: Option<jni::objects::Global<jni::objects::JClassLoader<'static>>>,
}

static BRIDGE: OnceLock<Bridge> = OnceLock::new();

/// Capture the `JavaVM` pointer and the application class loader so we can
/// attach the render thread to the VM and look up app classes for JNI calls.
/// Must be called from `android_main` before any of the other functions in this
/// module are called.
pub fn init(app: AndroidApp) {
    // SAFETY: the JavaVM pointer is valid for the lifetime of the process.
    let vm = unsafe { jni::JavaVM::from_raw(app.vm_as_ptr() as *mut _) };

    // Capture the application class loader from the Activity object so later
    // JNI calls can resolve `RustBridge` (see the `loader` field docs above).
    let activity_ptr = app.activity_as_ptr() as jni::sys::jobject;
    let loader = vm
        .attach_current_thread(
            |env: &mut jni::Env<'_>| -> jni::errors::Result<
                jni::objects::Global<jni::objects::JClassLoader<'static>>,
            > {
                // SAFETY: `activity_ptr` references the Activity, which is valid
                // for the lifetime of the process.
                let activity = unsafe { jni::objects::JObject::from_raw(env, activity_ptr) };
                let activity_class = env.get_object_class(&activity)?;
                let loader = activity_class.get_class_loader(env)?;
                env.new_global_ref(loader)
            },
        )
        .ok();

    let _ = BRIDGE.set(Bridge { vm, loader });
}

/// Launch the system file picker (SAF) and return immediately. The actual
/// picked URI is delivered asynchronously; use [`pick_file_result`] to poll
/// for it.
pub fn pick_file() -> Option<String> {
    call_string_method("pickFile", &["*/*"], "(Ljava/lang/String;)Ljava/lang/String;")
}

/// Poll for the most recent file picker result. Returns:
/// - `Some(uri)` — the user picked a file and we got a URI string.
/// - `None` — no result yet, or the bridge is not wired up.
pub fn pick_file_result() -> Option<String> {
    call_string_method("pickFileResult", &[], "()Ljava/lang/String;")
}

/// Launch the system folder picker (`ACTION_OPEN_DOCUMENT_TREE`) and return
/// immediately. The granted tree URI is delivered asynchronously; poll for it
/// with [`pick_folder_result`].
pub fn pick_folder() -> Option<String> {
    call_string_method("pickFolder", &[], "()Ljava/lang/String;")
}

/// Poll for the most recent folder-picker result. Returns the granted tree URI
/// string once, or `None` if there is no result yet.
pub fn pick_folder_result() -> Option<String> {
    call_string_method("pickFolderResult", &[], "()Ljava/lang/String;")
}

/// List the markdown/text files within a granted folder tree. Returns each
/// file's path **relative to the tree root** (e.g. `notes/todo.md`). The Java
/// side returns the paths newline-separated; we split them here.
pub fn list_tree_markdown(tree_uri: &str) -> Vec<String> {
    call_string_method(
        "listTreeMarkdown",
        &[tree_uri],
        "(Ljava/lang/String;)Ljava/lang/String;",
    )
    .map(|s| {
        s.lines()
            .filter(|l| !l.is_empty())
            .map(|l| l.to_string())
            .collect()
    })
    .unwrap_or_default()
}

/// Resolve a tree-relative path (e.g. `sub/other.md`) to a document URI within
/// a granted folder tree, by walking the tree by display name. Returns the
/// `content://.../document/...` URI string, or `None` if the file isn't found.
pub fn resolve_tree_path(tree_uri: &str, rel_path: &str) -> Option<String> {
    call_string_method(
        "resolveTreePath",
        &[tree_uri, rel_path],
        "(Ljava/lang/String;Ljava/lang/String;)Ljava/lang/String;",
    )
}

/// Open an external URL via the system handler (`Intent.ACTION_VIEW`). Returns
/// `true` if the intent was dispatched.
pub fn open_external(uri: &str) -> bool {
    call_string_method("openExternal", &[uri], "(Ljava/lang/String;)Ljava/lang/String;")
        .map(|s| s.eq_ignore_ascii_case("ok"))
        .unwrap_or(false)
}

/// Read the bytes at a `content://` URI returned by SAF. Returns `None` on
/// failure.
pub fn read_uri(uri: &str) -> Option<Vec<u8>> {
    call_byte_array_method("readUri", uri, "(Ljava/lang/String;)[B")
}

/// Return the activity's launching intent's `data` string (the URI the activity
/// was started with), if any.
pub fn intent_data_string() -> Option<String> {
    call_string_method("intentDataString", &[], "()Ljava/lang/String;")
}

/// Consume the pending intent data URI. Returns the URI string and clears it
/// so it's only consumed once. This is called by the Rust layer's update loop
/// to pick up new intents delivered via `onNewIntent`.
pub fn consume_intent_data() -> Option<String> {
    call_string_method("consumeIntentData", &[], "()Ljava/lang/String;")
}

/// Return the activity's `getFilesDir()` absolute path.
pub fn files_dir() -> Option<String> {
    call_string_method("filesDir", &[], "()Ljava/lang/String;")
}

/// Ask the content resolver for a document's human-readable display name
/// (`OpenableColumns.DISPLAY_NAME`). Returns `None` if the provider does not
/// supply one, in which case the caller should fall back to
/// [`crate::android_paths::uri_display_name`].
pub fn display_name(uri: &str) -> Option<String> {
    call_string_method(
        "displayName",
        &[uri],
        "(Ljava/lang/String;)Ljava/lang/String;",
    )
    .filter(|s| !s.is_empty())
}

// ---------------------------------------------------------------------------
// JNI helpers
// ---------------------------------------------------------------------------

/// Attach the current thread to the Java VM and run `f` with a `&mut Env`.
/// Returns `None` if the bridge is not initialised or the thread cannot be
/// attached.
fn with_env<F, R>(f: F) -> Option<R>
where
    F: FnOnce(&mut jni::Env<'_>) -> jni::errors::Result<R>,
{
    let bridge = BRIDGE.get()?;
    // `attach_current_thread<F, T, E>` where `F: FnOnce(&mut Env) -> Result<T, E>` returns
    // `Result<T, E>` (a single Result). The closure returns `Result<R, Error>`, so T = R.
    match bridge.vm.attach_current_thread(f) {
        Ok(inner) => Some(inner),
        Err(_) => None,
    }
}

/// Resolve the `RustBridge` class using the captured application class loader,
/// falling back to `FindClass` only if the loader was not captured. See the
/// [`Bridge::loader`] docs for why `FindClass` alone is insufficient on the
/// render thread.
fn bridge_class<'local>(
    env: &mut jni::Env<'local>,
) -> jni::errors::Result<jni::objects::JClass<'local>> {
    match BRIDGE.get().and_then(|b| b.loader.as_ref()) {
        Some(loader) => jni::objects::LoaderContext::Loader(&**loader).load_class(
            env,
            jni::strings::JNIString::new(BRIDGE_CLASS_BINARY),
            false,
        ),
        None => env.find_class(jni::strings::JNIString::new(BRIDGE_CLASS)),
    }
}

/// Call a static method on the bridge class that returns `String` (or null).
fn call_string_method(name: &str, args: &[&str], sig: &str) -> Option<String> {
    let runtime_sig = jni::signature::RuntimeMethodSignature::from_str(sig).ok()?;
    with_env(|env| -> jni::errors::Result<Option<String>> {
        let class = bridge_class(env)?;
        let method_sig = runtime_sig.method_signature();
        let method_name = jni::strings::JNIString::new(name);
        let jstring_args: Vec<jni::objects::JString> = args
            .iter()
            .map(|s| jni::objects::JString::new(env, *s))
            .collect::<jni::errors::Result<_>>()?;
        let jargs: Vec<jni::objects::JValue> = jstring_args
            .iter()
            .map(jni::objects::JValue::from)
            .collect();
        let ret = env.call_static_method(&class, &method_name, &method_sig, &jargs)?;
        if ret.is_null() {
            return Ok(None);
        }
        let obj = ret.l()?;
        // The method signature declares `Ljava/lang/String;` so the
        // returned object is guaranteed to be a `java.lang.String` instance.
        let jstring: jni::objects::JString<'_> = env
            .cast_local::<jni::objects::JString>(obj)
            .map_err(|_| jni::errors::Error::WrongObjectType)?;
        let java_str = jstring.try_to_string(env)?;
        Ok(Some(java_str))
    })
    .flatten()
}

/// Call a static method on the bridge class that returns `byte[]`.
fn call_byte_array_method(name: &str, arg: &str, sig: &str) -> Option<Vec<u8>> {
    let runtime_sig = jni::signature::RuntimeMethodSignature::from_str(sig).ok()?;
    with_env(|env| -> jni::errors::Result<Option<Vec<u8>>> {
        let class = bridge_class(env)?;
        let method_sig = runtime_sig.method_signature();
        let method_name = jni::strings::JNIString::new(name);
        let jstring = jni::objects::JString::new(env, arg)?;
        let jarg = jni::objects::JValue::from(&jstring);
        let ret = env.call_static_method(&class, &method_name, &method_sig, &[jarg])?;
        if ret.is_null() {
            return Ok(None);
        }
        let obj = ret.l()?;
        // The method signature declares `[B` so the returned object is
        // guaranteed to be a `byte[]` instance.
        let arr: jni::objects::JByteArray<'_> = env
            .cast_local::<jni::objects::JByteArray>(obj)
            .map_err(|_| jni::errors::Error::WrongObjectType)?;
        let len = arr.len(env)? as usize;
        let mut buf = vec![0i8; len];
        arr.get_region(env, 0, &mut buf)?;
        Ok(Some(buf.into_iter().map(|b| b as u8).collect()))
    })
    .flatten()
}