# Architecture

EmuLabZip is a Cargo workspace. Platform code lives in small crates behind one trait, and two front ends (the desktop
app and the CLI) use them the same way.

```text
                ┌──────────────────────┐      ┌──────────────────┐
                │  apps/desktop        │      │  emulabzip-cli   │
                │  Svelte UI ◄─► Tauri │      │  (clap)          │
                └──────────┬───────────┘      └────────┬─────────┘
                           │        Lab (routes by platform)
                           └───────────────┬───────────┘
                                 ┌─────────┴─────────┐
                      Backend    │                   │   Backend
                ┌────────────────┴───┐        ┌──────┴──────────────┐
                │ emulabzip-android  │        │ emulabzip-apple     │
                │ logcat, getprop,   │        │ simctl, log stream, │
                │ dumpsys, pm, ps    │        │ listapps, plutil    │
                └─────────┬──────────┘        └──────┬──────────────┘
                ┌─────────┴──────────┐               │
                │ emulabzip-adb      │          xcrun simctl
                │ ADB host protocol  │
                └─────────┬──────────┘
                    adb server :5037
```

## Crates

| Crate | Responsibility |
|-------|----------------|
| `emulabzip-core` | The data model (`Device`, `App`, `InfoSection`, `LogEntry`), log filters, a bounded ring buffer, the `Stream` type and the `Backend` trait. No platform code. |
| `emulabzip-adb` | An async client for the ADB host protocol: device listing and tracking, `shell:`, `exec:` and the `sync:` file protocol. |
| `emulabzip-android` | Implements `Backend` for Android on top of `emulabzip-adb`. Parsers for `getprop`, logcat, `dumpsys battery`, `wm`, `ps` and `pm`. |
| `emulabzip-apple` | Implements `Backend` for the iOS Simulator with `xcrun simctl`. Parsers for the device list, ndjson logs and app lists. |
| `emulabzip-storage` | Viewers that do not depend on a platform: file type detection, a read-only SQLite inspector, SharedPreferences and property lists. |
| `emulabzip-cli` | The `emulabzip` command. |
| `apps/desktop` | The Tauri 2 shell (`src-tauri`) and the Svelte 5 interface (`src`). |

## The backend trait

```rust
#[async_trait]
pub trait Backend: Send + Sync {
    fn platform(&self) -> Platform;
    async fn is_available(&self) -> bool;
    async fn devices(&self) -> Result<Vec<Device>>;
    async fn info(&self, id: &str) -> Result<Vec<InfoSection>>;
    async fn apps(&self, id: &str) -> Result<Vec<App>>;
    async fn logs(&self, id: &str, options: LogOptions) -> Result<Stream<LogEntry>>;
    async fn screenshot(&self, id: &str) -> Result<Vec<u8>>;

    // Optional: the default implementations return `Error::Unsupported`.
    async fn perf(&self, id: &str, options: PerfOptions) -> Result<Stream<PerfSample>>;
    async fn memory(&self, id: &str, app: &str) -> Result<MemoryReport>;
    async fn heap_dump(&self, id: &str, app: &str) -> Result<HeapDump>;
    async fn storage_roots(&self, id: &str, app: &str) -> Result<Vec<StorageRoot>>;
    async fn list_files(&self, id: &str, app: &str, root: &str, path: &str) -> Result<Vec<FileEntry>>;
    async fn read_file(&self, id: &str, app: &str, root: &str, path: &str) -> Result<Vec<u8>>;
}
```

`Lab` holds every backend and routes calls by `Target { platform, id }`. A missing tool (`adb`, `xcrun`) makes a
backend unavailable without affecting the others.

## Streams

Logs are produced by a background task and delivered through a bounded channel (`Stream<T>`). Dropping the stream
aborts the task, which closes the ADB socket or kills the `log stream` child process, so nothing outlives the reader.

In the desktop app, each stream is a *session* owned by the Tauri state. Lines are batched every 100 ms and sent to the
interface over a Tauri IPC channel. A batch holds at most 4,000 lines; any surplus is counted and reported as dropped
rather than slowing the device or the UI. The interface keeps its own bounded buffer and renders only the visible rows.

## Storage paths

File paths are always relative to a *storage root* chosen by the backend (`data`, `external`, `bundle`…). `Lab`
rejects absolute paths and `..` before a backend sees them, and the iOS backend also resolves symbolic links and
refuses anything that ends up outside the container.

## Design rules

- **Public interfaces only.** Everything EmuLabZip reads is something you could get with `adb shell` or `xcrun simctl`.
  See [How the official tools work](research/how-the-official-tools-work.md).
- **No shell injection.** Values that reach the device shell are quoted (`emulabzip_adb::quote`), package names are
  validated, and `simctl` is invoked without a shell.
- **Batch the device round trips.** The Overview is one shell call on Android; parsers split its output.
- **Parsers are tested with real output.** Fixtures captured from emulators and simulators live next to each crate.
- **Explicit writes only.** Files are written only after a save dialog or a CLI flag.
