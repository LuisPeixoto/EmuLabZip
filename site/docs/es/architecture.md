---
source: a1e5e440e93b
description: "Cómo está organizado EmuLabZip: el workspace de Cargo, los crates por plataforma, el trait Backend, los streams, el proxy, el eje de tiempo y las reglas de diseño."
---
# Arquitectura

EmuLabZip es un workspace de Cargo. El código de cada plataforma vive en crates pequeños detrás de un único trait, y dos front ends (la
app de escritorio y la CLI) los usan de la misma manera.

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

| Crate | Responsabilidad |
|-------|-----------------|
| `emulabzip-core` | El modelo de datos (`Device`, `App`, `InfoSection`, `LogEntry`), los filtros de logs, un búfer circular acotado, el tipo `Stream` y el trait `Backend`. Sin código de plataforma. |
| `emulabzip-adb` | Un cliente asíncrono del protocolo host de ADB: listado y seguimiento de dispositivos, `shell:`, `exec:` y el protocolo de archivos `sync:`. |
| `emulabzip-android` | Implementa `Backend` para Android sobre `emulabzip-adb`. Parsers para `getprop`, logcat, `dumpsys battery`, `wm`, `ps` y `pm`. |
| `emulabzip-apple` | Implementa `Backend` para el simulador de iOS con `xcrun simctl`. Parsers para la lista de dispositivos, los logs ndjson y las listas de apps. |
| `emulabzip-audit` | Auditorías de calidad que miran una app desde fuera: las reglas de seguridad y privacidad y la tabla de patrones de secretos con redacción, los tipos `Finding` y `Audit`, la recogida de sus entradas desde un dispositivo a través de `Lab`, el analizador de APK, AAB e IPA con el diff de tamaño y el presupuesto, la comparación de imágenes y las líneas base, y el barrido de idiomas y tamaños de texto. |
| `emulabzip-storage` | Visores que no dependen de una plataforma: detección del tipo de archivo, un inspector de SQLite de solo lectura, SharedPreferences y listas de propiedades, Preferences y Proto DataStore, MMKV, AsyncStorage, Core Data y un lector del formato de archivo de Realm; el **editor** (filas de SQLite, preferencias, listas de propiedades, con copia de seguridad y deshacer); el formato de snapshot `.ezstate` y el diff de esquemas. |
| `emulabzip-net` | El proxy de depuración: una autoridad de certificación, interceptación de TLS, reenvío de HTTP/1.1 y HTTP/2, gRPC en streaming, clasificación y decodificación de GraphQL y gRPC, reenvío de peticiones, condiciones de red (y HTTP/3 hacia los servidores mediante `quinn` y `h3`), reglas que simulan, retrasan, bloquean o retienen peticiones, **repetición** (replay) de una grabación HAR (un `ReplaySet` al que el proxy consulta antes que a cualquier servidor), un parser de tramas de WebSocket, un lector de capturas de paquetes, un almacén acotado de flujos con un feed de cambios, decodificación de cuerpos y exportación a HAR. Sin código de plataforma. |
| `emulabzip-trace` | Dar sentido a las grabaciones: el modelo del flame graph, parsers de informes de simpleperf y de exportaciones de `xctrace`, configuraciones de Perfetto, el procesador de trazas (descarga, SQL, resúmenes), el servidor de un solo uso para la interfaz de Perfetto y la biblioteca de grabaciones. Sin código de plataforma. |
| `emulabzip-frameworks` | Mirar dentro de las apps de Flutter y React Native: un pequeño cliente JSON sobre WebSocket (`Rpc`), el servicio de la VM de Dart (árbol de widgets, fotogramas, memoria) y Metro con Hermes (árbol de React, heap, consola y el **estado** de Redux, React Query, contextos y stores registrados, con un registro de acciones y un diff de estado). Llega al puerto de un dispositivo con `Lab::connect_port`, así que aquí no vive código de plataforma. |
| `emulabzip-symbols` | Hacer legibles los stacks de los fallos: el retracer de `mapping.txt` de R8 y ProGuard (con frames inlined), source maps con un decodificador VLQ (mapas compuestos de Metro y Hermes), búsquedas en dSYM con `atos`, el almacén por app y por versión de estos archivos y `symbolicate`, que aplica lo que corresponde a un fallo e indica qué falta. Trabaja con texto y números, así que corre sobre archivos sin dispositivo. |
| `emulabzip-report` | Informes de error: reúne el dispositivo, una captura de pantalla, el log, las peticiones fallidas y los fallos legibles (de un dispositivo o de una sesión), escribe un único archivo HTML autocontenido y un resumen en Markdown, y construye enlaces de issue prerrellenados para GitHub, GitLab y Jira. No envía nada. |
| `emulabzip-heap` | Leer volcados de heap `.hprof` y encontrar fugas: un parser de dos pasadas, el grafo de objetos, dominadores y tamaños retenidos, y las reglas de fugas. Sin código de plataforma y sin dependencias, así que también corre sobre un volcado desde un archivo. |
| `emulabzip-session` | Sesiones: el formato `.ezsession` (versión 2, con el reloj del dispositivo), un grabador que sigue a un dispositivo a través de `Lab` (logs, muestras, fallos, capturas de pantalla, toques, fotogramas lentos y fragmentos de vídeo), un lector que también abre la carpeta de una grabación interrumpida y sigue abriendo archivos de la versión 1, el resumen, la **línea de tiempo** que pone cada stream en un solo eje, el cargador de una grabación para repetirla, los presupuestos de `emulabzip.toml` y la comparación con una línea base. |
| `emulabzip-mcp` | El servidor MCP: herramientas sobre `Lab`, servidas por la entrada y la salida estándar con `rmcp`. |
| `emulabzip-plugin` | Plugins: manifiestos, el paso de aprobación, el protocolo de líneas JSON y el ejecutor. Sin código de plataforma. |
| `emulabzip-cli` | El comando `emulabzip`. |
| `apps/desktop` | El shell de Tauri 2 (`src-tauri`) y la interfaz de Svelte 5 (`src`). |

## El trait Backend

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
    async fn trace_templates(&self, id: &str) -> Result<Vec<String>>;
    async fn record_trace(&self, id: &str, request: &TraceRequest, out_dir: &Path, stop: watch::Receiver<bool>) -> Result<TraceFile>;
    async fn clock(&self, id: &str) -> Result<DeviceClock>;
    async fn input(&self, id: &str) -> Result<Stream<InputEvent>>;
    async fn slow_frames(&self, id: &str, app: &str) -> Result<Stream<SlowFrame>>;
    async fn record_video(&self, id: &str, out_dir: &Path, segment_secs: u32, stop: watch::Receiver<bool>, segments: mpsc::UnboundedSender<VideoSegment>) -> Result<()>;
    async fn crashes(&self, id: &str, app: Option<&str>) -> Result<Vec<CrashSummary>>;
    async fn crash(&self, id: &str, crash_id: &str) -> Result<CrashReport>;
    async fn controls(&self, id: &str, app: Option<&str>) -> Result<ControlInfo>;
    async fn control(&self, id: &str, control: &Control) -> Result<()>;
    async fn network_status(&self, id: &str) -> Result<NetworkStatus>;
    async fn connect_network(&self, id: &str, port: u16) -> Result<NetworkStatus>;
    async fn disconnect_network(&self, id: &str) -> Result<NetworkStatus>;
    async fn install_ca(&self, id: &str, pem: &str, der: &[u8]) -> Result<CaInstall>;
}
```

`Lab` contiene todos los backends y enruta las llamadas por `Target { platform, id }`. Una herramienta ausente (`adb`, `xcrun`) hace que
un backend no esté disponible sin afectar a los demás.

## Streams

Los logs los produce una tarea en segundo plano y se entregan a través de un canal acotado (`Stream<T>`). Al soltar el stream se
aborta la tarea, lo que cierra el socket de ADB o mata el proceso hijo de `log stream`, de modo que nada sobrevive al lector.

En la app de escritorio, cada stream es una *sesión* que pertenece al estado de Tauri. Las líneas se agrupan cada 100 ms y se envían a la
interfaz por un canal IPC de Tauri. Un lote contiene como máximo 4,000 líneas; el excedente se cuenta y se informa como descartado
en lugar de ralentizar el dispositivo o la interfaz. La interfaz mantiene su propio búfer acotado y dibuja solo las filas visibles.

## Un solo eje de tiempo

Cada stream marca el tiempo a su manera (el log imprime la hora local del dispositivo sin año, las muestras cuentan desde que
empezaron, los toques y los fotogramas cuentan desde que arrancó el dispositivo), así que una grabación guarda un `DeviceClock`: el reloj
de pared del dispositivo frente al de este equipo, su zona horaria y el momento en que su reloj monotónico marcaba cero. `timeline::build`
de `emulabzip-session` lo usa para poner toques, problemas del log, peticiones, fotogramas lentos, recolecciones de basura, fallos,
capturas de pantalla y vídeo en un solo eje, en milisegundos desde el inicio. Un fotograma lento se ubica con el `Uptime` que
imprime el volcado de `gfxinfo`, que es el reloj en el que están los tiempos de los fotogramas. El vídeo de la pantalla se graba en fragmentos cortos (consulta
[Línea de tiempo y vídeo de pantalla](timeline.md)), cada uno notificado al completarse, así que una grabación interrumpida conserva lo que tenía.

## El proxy

`emulabzip-net` no sabe nada de dispositivos. Los backends solo deciden cómo llega un dispositivo hasta él y cómo llega a confiar en el
certificado; la CLI y la app de escritorio inician el proxy y le entregan un `FlowStore`.

```text
 device ──► 127.0.0.1:<port> ──► proxy ──► the internet
                                   │
              CONNECT, first byte is 0x16 (TLS)?
                 yes ─► terminate TLS with a leaf for the SNI name, serve HTTP/1.1 or HTTP/2,
                        forward over a new TLS connection, record the flow
                 no  ─► copy bytes both ways, record a tunnel
              handshake refused ─► remember the host, pass it through next time
```

Una **repetición** (replay) es un paso más antes de consultar al servidor: tras las reglas, `forward` busca la petición en el
`ReplaySet` y responde desde él, o responde 504 cuando no está. Mientras se repite una grabación, se rechazan los caminos que
no se pueden responder desde un HAR (un túnel que el proxy no puede decodificar, un WebSocket, una llamada gRPC, un reenvío), de modo que
nada llega a la red. Consulta [Repetir una grabación](replay.md).

Los cuerpos de las respuestas se transmiten al cliente mientras se graba una copia (hasta 10 MB), así que las descargas y los flujos de eventos
siguen funcionando. Los cuerpos de las peticiones se leen completos antes de reenviarlos. Las actualizaciones a WebSocket se tunelizan tras el handshake, y
los bytes también se pasan a un parser de tramas que registra los mensajes.

Antes de reenviar una petición, el proxy consulta las **reglas** (gana la primera que coincida): un mock responde de inmediato, un retraso espera, un
bloqueo falla y un breakpoint retiene la petición, o la respuesta, en un registro hasta que la interfaz la continúa. Con HTTP/3
activado, un host que anunció `Alt-Svc: h3` se alcanza por QUIC, con una alternativa a TCP.

## Rutas de almacenamiento

Las rutas de archivos son siempre relativas a una *raíz de almacenamiento* elegida por el backend (`data`, `external`, `bundle`…). `Lab`
rechaza las rutas absolutas y `..` antes de que un backend las vea, y el backend de iOS además resuelve los enlaces simbólicos y
rechaza cualquier cosa que acabe fuera del contenedor.

## Reglas de diseño

- **Solo interfaces públicas.** Todo lo que lee EmuLabZip es algo que podrías obtener con `adb shell` o `xcrun simctl`.
  Consulta [Cómo funcionan las herramientas oficiales](research/how-the-official-tools-work.md).
- **Sin inyección de shell.** Los valores que llegan al shell del dispositivo se entrecomillan (`emulabzip_adb::quote`), los nombres de paquete se
  validan y `simctl` se invoca sin shell.
- **Agrupa las idas y vueltas al dispositivo.** El Overview es una sola llamada de shell en Android; los parsers dividen su salida.
- **Los parsers se prueban con salida real.** Los fixtures capturados de emuladores y simuladores viven junto a cada crate.
- **Solo escrituras explícitas.** Los archivos se escriben únicamente tras un diálogo de guardado o un flag de la CLI.
