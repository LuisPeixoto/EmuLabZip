---
source: 951822beac6a
description: "Referencia de la línea de comandos emulabzip: dispositivos, logs, rendimiento, red, almacenamiento, sesiones, controles, plugins y servidor MCP."
---
# Línea de comandos

El comando `emulabzip` te da los mismos datos que la app de escritorio, en un terminal o en un script. Es un único binario
estático sin dependencias de ejecución aparte de `adb` y Xcode.

```bash
cargo install --git https://github.com/LuisPeixoto/EmuLabZip emulabzip-cli
```

Los archivos precompilados para macOS, Linux y Windows se adjuntan a cada
[release](https://github.com/LuisPeixoto/EmuLabZip/releases).

## Elegir un dispositivo

Todos los comandos excepto `devices` trabajan con un solo dispositivo. Cuando solo hay uno en marcha se usa automáticamente. En caso contrario pasa
`--device` (o define `EMULABZIP_DEVICE`) con un serial de adb, un UDID de simulador, un nombre o parte de un nombre:

```bash
emulabzip --device pixel info
emulabzip -d "iPhone 17" logs
```

## Comandos

### `emulabzip devices`

```text
PLATFORM  NAME       OS                   STATE   ID
Android   Pixel 9    Android 16 (API 36)  online  emulator-5554
iOS       iPhone 17  iOS 26.2             online  F8CE84C1-04D1-4E33-B52B-02B59E8A09C5
```

`--json` imprime la lista como JSON. El alias `emulabzip ls` hace lo mismo.

### `emulabzip info`

Imprime los mismos grupos que Overview (dispositivo, software, hardware, pantalla, batería, configuración regional). `--json` imprime un
objeto de objetos.

### `emulabzip apps`

Lista las apps de usuario con su versión y su id de proceso. `--all` añade las apps del sistema, `--running` deja solo las que están en ejecución.

### `emulabzip logs`

Sigue el log del dispositivo, coloreado por nivel, hasta que pulses <kbd>Ctrl+C</kbd>.

| Opción | Significado |
|--------|-------------|
| `-a, --app <id>` | Solo las líneas de esta app. El id de proceso se actualiza cada pocos segundos, así que se siguen los reinicios. |
| `-l, --level <level>` | Nivel mínimo: `verbose`, `debug`, `info`, `warn`, `error`, `fatal`, o una letra. |
| `-t, --tag <text>` | El tag contiene el texto. |
| `-g, --grep <text>` | El mensaje o el tag contiene el texto. |
| `-E, --regex` | Trata `--grep` como una expresión regular. |
| `--tail <n>` | Líneas recientes que se imprimen primero (Android, 100 por defecto). |
| `--json` | Un objeto JSON por línea, listo para `jq`. |

```bash
emulabzip logs --app com.example.app --level warn
emulabzip logs -g 'skipped \d+ frames' -E
emulabzip logs --json | jq -r 'select(.level == "error") | .message'
```

### `emulabzip screenshot`

Guarda un PNG de la pantalla. `-o, --output <file>` elige la ruta; por defecto el archivo se nombra según el dispositivo y la
hora.

### `emulabzip perf`

Imprime una línea por muestra: CPU de la app y del dispositivo, memoria de la app y del dispositivo, tasa de fotogramas, fotogramas con tirones y batería.

| Opción | Significado |
|--------|-------------|
| `-a, --app <id>` | App que se muestrea. Sin ella, solo valores de todo el dispositivo. |
| `-i, --interval <ms>` | Tiempo entre muestras (1000 por defecto, mínimo 250). |
| `--json` | Un objeto JSON por muestra. |

### `emulabzip mem <app>`

Un desglose de memoria: PSS por categoría en Android, huella por región de VM en iOS. Consulta [Memory](memory.md).

### `emulabzip heapdump <app>`

Guarda un volcado de heap `.hprof` (Android, apps depurables) o un grafo de memoria `.memgraph` (iOS). `-o` elige el archivo. Con
`--analyze` también busca fugas, consulta [Encontrar fugas](leaks.md).

### `emulabzip heap analyze <file>`

Busca fugas en un `.hprof` (cualquier plataforma) o un `.memgraph` (en un Mac). `--json`; `--top <n>` fija cuántos objetos y
clases se listan; `--all-heaps` mira más allá del heap propio de la app. Consulta [Encontrar fugas](leaks.md).

### `emulabzip startup <app>`

Mide cuánto tarda la app en abrirse. `-k, --kind cold|warm|hot` (`cold` por defecto), `-r, --runs <n>` (1 a 50, 5 por defecto),
`--json`. Consulta [Tiempo de arranque de la app](startup.md).

### `emulabzip energy <app>`

Mide la energía que usa la app mientras la usas. `-s, --seconds <n>` (30 por defecto; <kbd>Ctrl+C</kbd> lee lo que haya),
`--json`. Consulta [Energy](energy.md).

### `emulabzip files <app> [path]`

Lista una carpeta de la app. `--root` elige la raíz de almacenamiento (`data` o `external` en Android; `data`, `bundle` o
`group:<id>` en iOS); por defecto se usa la primera legible. Consulta [Storage](storage.md).

### `emulabzip pull <app> <path>`

Copia un archivo a este equipo. `-o` elige el destino.

### `emulabzip db <app> <path>`

Abre una base de datos SQLite en solo lectura. Sin opciones lista las tablas y las vistas; `--table` imprime filas (`--limit`,
`--offset`); `-q, --query` ejecuta una sentencia de solo lectura. Todas aceptan `--json`.

### `emulabzip net`

Ejecuta el proxy, apunta el dispositivo hacia él e imprime cada petición cuando termina, hasta <kbd>Ctrl+C</kbd>. El
dispositivo se restablece cuando se detiene, también tras un error. Consulta [Network](network.md).

| Opción | Significado |
|--------|-------------|
| `-p, --port <n>` | Puerto en `127.0.0.1` (9090 por defecto, o `EMULABZIP_PORT`). |
| `--no-connect` | Solo ejecuta el proxy; no cambia ningún dispositivo. |
| `--har <file>` | Guarda las peticiones como un archivo HAR al detenerse. |
| `--json` | Un objeto JSON por petición. |
| `--rules <file>` | Simula, retrasa, bloquea, redirige a un archivo u otro servidor, comprueba contra OpenAPI o ejecuta scripts sobre las peticiones con las reglas de un archivo JSON. Consulta [Mocks y breakpoints](mocks.md) y [Reglas de red](network-rules.md). |
| `--http3` | Habla HTTP/3 con los servidores que lo ofrecen. `--show-h3` también muestra las apps que lo ofrecen. |
| `--conditions <preset>` | Ralentiza la red: `fast-3g`, `slow-3g`, `slow-wifi`, `high-latency`, `packet-loss` u `offline`. Consulta [Condiciones de red](network-conditions.md). |
| `--down <kbps>`, `--up <kbps>`, `--latency <ms>`, `--loss <percent>` | Números propios, o cambios a un preset. |

Las operaciones GraphQL y los métodos y estados gRPC se imprimen después de cada petición.

`emulabzip net import <file> [--har <file>] [--json] [--dns]` lee una captura `.pcap` o `.pcapng` e imprime las
peticiones que contiene, con las consultas DNS cuando se piden. Consulta [Importar una captura](pcap.md).

### `emulabzip layout` y `emulabzip a11y`

`layout` imprime las vistas de la pantalla como un árbol (`--json` para todo, `--png <file>` para guardar la captura tomada
con él, `--depth <n>`, `--app <id>` para marcar una pantalla de iOS con el framework de su app). `a11y` audita la pantalla e imprime los resultados; `--fail-on note|warning|error` sale con 1 cuando
encuentra algo al menos así de grave. Consulta [Diseño y accesibilidad](layout.md).

### `emulabzip frameworks <app>`

Lista los frameworks de interfaz con los que está construida la app (Flutter, React Native, Hermes, Jetpack Compose, SwiftUI) y qué
los delató. `--json`. Consulta [Dentro de los frameworks](frameworks.md).

### `emulabzip flutter tree|frames|memory <app>`

Mira dentro de una app Flutter en modo debug o profile a través de su servicio de la VM de Dart. `--uri <address>` indica dónde escucha cuando
no está en el log; `tree` tiene `--depth <n>`, `frames` tiene `-s, --seconds <n>` (10 por defecto), `memory` tiene `--top <n>`.
Todos aceptan `--json`.

### `emulabzip rn tree|heap|console|state <app>`

Mira dentro de una app de desarrollo de React Native a través de Metro (`--metro <port>`, 8081 por defecto). `tree` tiene `--depth <n>`,
`console` tiene `-s, --seconds <n>`. `state` lee Redux, React Query, contextos y stores registrados; `--watch` sigue las acciones de
Redux e imprime lo que cambió. Consulta [Estado de React Native](react-native-state.md). Todos aceptan `--json`. Metro permite que un
solo depurador se conecte a la vez.

### `emulabzip state save|restore|show|diff`

`save <app> -o file.ezstate [--external]` escribe los datos de una app en un solo archivo. `restore <file> --yes` reemplaza los datos de la app
con él, después de guardar lo que reemplaza. `show <file>` dice qué contiene un archivo. `diff <a> [b]` lista las tablas, columnas,
índices y claves que cambiaron entre dos archivos, o entre uno y la app ahora. Consulta [Estado de la app](app-state.md).

### `emulabzip edit`

Cambia datos en el dispositivo: `cell` (`-t <table>`, `--rowid <n>` o `--pk col=value`, `--set`, `--text`, `--null`), `insert`,
`delete`, `pref-set <key> <value> [--as type]`, `pref-remove`, `plist-set <key/path> <value> [--as type]`, `plist-remove`
y `undo <folder>`. Cada uno recibe la app y el archivo, y **todos necesitan `--yes`**. Consulta
[Cambiar los datos de una app](storage-editing.md).

### `emulabzip avd`, `sim` y `snapshot`

`avd` y `sim` tienen `list`, `options`, `create <name> --image <id> --model <id>`, `boot <device>` (`--headless`, `--cold`,
`--wipe`), `shutdown`, `wipe` (`erase` en `sim`) y `delete`. `snapshot` tiene `list`, `save`, `load` y `delete` para el
emulador en marcha elegido con `-d`. `wipe`, `delete`, `boot --wipe`, `snapshot load` y `snapshot delete` necesitan `--yes`.
`emulabzip devices --all` también lista los dispositivos que no están en marcha. Consulta [Devices](devices.md).

### `emulabzip audit security <app>`

Busca secretos y datos personales en el log, las preferencias y las bases de datos, HTTP sin cifrar y secretos en las peticiones (`--har`),
los indicadores `debuggable` y `allowBackup`, componentes a los que pueden llegar otras apps, excepciones de App Transport Security y lagunas
en el manifiesto de privacidad. `--only manifest,logs,storage,traffic`, `--json`, `--fail-on <note|warning|error>`. La evidencia se recorta para que un
secreto nunca se repita. Consulta [Auditoría de seguridad](security-audit.md).

### `emulabzip size show|diff`

`show <file>` desglosa un `.apk`, `.aab` o `.ipa` por tipo, ABI nativa y archivo dex (`--top`, `--json`); con
`--budget` somete el build a la sección `[size]` de `emulabzip.toml` (`--config`, `--baseline`) y sale con 1 al superar un límite.
`diff <before> <after>` dice qué cambió (`--fail-on-growth <percent>`). Consulta [Tamaño del build](package-size.md).

### `emulabzip i18n check|sweep`

`check` busca texto cortado, fuera de la pantalla, recortado o sin reflejar en la pantalla tal como está (`--rtl`, `--fail-on`).
`sweep` lo prueba en otros idiomas y tamaños de texto (`--locales en-XA,ar-XB`, `--scales 100,130,200`, `--app`,
`--fail-on-worse`) y restablece el dispositivo. Consulta [Idioma, tamaño de texto y texto que no cabe](localization.md).

### `emulabzip control locale|reset-locale|font-scale|display|reset-display`

Cambian el idioma de una app (Android) o del simulador (iOS), el tamaño de texto y el tamaño de pantalla de Android.
`a11y --simulate <kind> [-o file]` muestra la pantalla como la ve alguien con daltonismo.

### `emulabzip visual capture|update|compare|list|diff|session`

Líneas base de la pantalla, y lo que cambió desde entonces. `capture` y `compare` aceptan `--status-bar` (una barra limpia de 9:41), `--dir`,
`--threshold`; `update` necesita `--yes`. `compare` sale con 1 al superar el límite y dibuja la diferencia. Consulta
[Regresión visual](visual-regression.md).

### `emulabzip analytics`

Ejecuta el proxy e imprime los eventos que una app envía a Segment, Amplitude, Mixpanel y Firebase (`--service`, `--port`,
`--no-connect`, `--json`), o con `--firebase-log` las líneas de depuración de Firebase del log del dispositivo. Consulta
[Inspector de analytics](analytics.md).

### `emulabzip session` y `emulabzip ci`

`session record -o run.ezsession [--app <id>] [--network] [--screens <s>] [--duration <s>] [--video [<s>]] [--taps]
[--replay <file> [--passthrough] [--timing]]` graba hasta <kbd>Ctrl+C</kbd>; `--video` graba la pantalla en trozos
(30 s por defecto, de 1 a 180), `--taps` graba los toques, y `--replay` responde mientras tanto a las peticiones de la app desde una grabación
(necesita `--network`). `session show <file> [--json | --markdown]` resume una sesión o la carpeta `.partial`
de una que se cortó. `session timeline <file> [--lane <name>…] [--json]` imprime toques, peticiones, problemas de log, fotogramas lentos, recolecciones de basura, fallos,
capturas de pantalla y video en un mismo eje de tiempo, y `session video <file> [-o
<dir>]` escribe los trozos de la grabación de pantalla. `ci -o run.ezsession [--app <id>] [--network] [--screens <s>]
[--fail-on-crash] [--summary <file>] [--video [<s>]] [--taps] [--replay <file> [--fail-on-miss]] -- <command…>` graba
alrededor de un comando y sale con su código. Consulta [Sessions](sessions.md), [Línea de tiempo y video de pantalla](timeline.md),
[Repetir una grabación](replay.md) y [Modo CI](ci.md).

### `emulabzip compare <a> <b>`

Lista lo que difiere entre dos dispositivos (`--all` para lo que coincide también, `--json`). Consulta [Comparar dispositivos](compare.md).

### `emulabzip crashes` y `emulabzip crash <id>`

`crashes` lista lo que registró el dispositivo, lo más reciente primero, con un id para cada uno; `-a, --app` y `-k, --kind
crash|native|anr` lo acotan. `crash` muestra uno con su stack y pistas: `--all-threads` imprime todos los hilos,
`--raw` imprime el informe tal como lo escribió el dispositivo, y `-m, --mapping <file>` lo lee con un `mapping.txt`, un source map
o un `.dSYM` que indiques, además de los guardados para su app y versión. Ambos aceptan `--json`. Consulta
[Crashes](crashes.md) y [Stacks legibles](symbolication.md).

### `emulabzip mappings` y `emulabzip symbolicate`

`mappings add <app> <version> <file>` guarda un `mapping.txt`, un source map o un `.dSYM` para un build (`-k, --kind
mapping|sourcemap|dsym` cuando no se puede deducir del archivo), `mappings list [app] [--json]` muestra lo que se guarda, y
`mappings remove <app> <version> [-k <kind>]` lo olvida. `symbolicate <stack.txt|-> -m <file>` lee un stack que
pegaste, desde un archivo o la entrada estándar. Consulta [Stacks legibles](symbolication.md).

### `emulabzip report`

Escribe lo que necesita un informe de error en un solo archivo HTML: `[--session <file>] [-a, --app <id>] [-o <file>] [--title <text>]
[--description <text>] [--without screenshot|logs|requests|crashes|video] [--issue github:owner/repo | gitlab:[host/]group/project |
jira:site/PROJECTID/ISSUETYPEID] [--open]`. Con `--issue` imprime un enlace que abre el issue con un resumen ya
rellenado, y `--open` lo abre en el navegador. Consulta [Informes de error](bug-reports.md).

### `emulabzip control`

Cambia el dispositivo. Consulta [Controls](controls.md).

| Comando | Qué hace |
|---------|----------|
| `control info [-a <app>]` | Lo que admite el dispositivo, su apariencia y los permisos de la app. |
| `control location set <lat> <lon>` / `location clear` | Fija o restablece la posición. |
| `control appearance light\|dark` | Modo claro u oscuro. |
| `control statusbar set [--time 9:41] [--battery 100] [--wifi true] [--cellular true]` / `statusbar clear` | Una barra de estado limpia, o la real de vuelta. |
| `control battery set <level> [--charging]` / `battery reset` | Finge que la batería está en un nivel (Android). |
| `control permission <app> <permission> grant\|revoke\|reset` | Un nombre de permiso de Android, o un servicio de iOS como `photos`. |
| `control notify [<app>] [--title …] [--body …] [--payload <file>]` | Una notificación (Android) o un push simulado (iOS). |
| `control open <url>` | Abre un enlace en el dispositivo. |
| `control tap <x> <y>` / `control swipe <x1> <y1> <x2> <y2> [--ms n]` | Un toque o un arrastre, en píxeles (Android). Consulta [Entrada y flujos](automation.md). |
| `control text <text>` / `control key <name>` | Escribe ASCII imprimible, o pulsa una tecla (Android). |
| `control launch <app>` | Inicia una app. |

### `emulabzip webview`

Las páginas dentro del WebView de una app, o en Chrome, en un dispositivo Android. Consulta [Páginas web en apps](webviews.md).

| Comando | Qué hace |
|---------|----------|
| `webview list [--json]` | Los sockets que se pueden inspeccionar, y sus páginas. |
| `webview console [--seconds n]` | Lo que una página escribe en su consola, y los errores que no captura. |
| `webview eval <expression>` | Ejecuta JavaScript en una página e imprime el valor. Falla cuando el script lanza una excepción. |
| `webview network [--seconds n] [--reload] [--har <file>]` | Las peticiones que hace la página, como las ve el navegador. |

Cada uno acepta `--app <package>`, `--socket <name>` y `--page <id or part of its address>` para indicar qué página, y `--json`.

### `emulabzip flow`

Graba toques en un flujo, lo reproduce y lo exporta. Consulta [Entrada y flujos](automation.md).

| Comando | Qué hace |
|---------|----------|
| `flow record --name <name> [--app <app>] [--no-launch] [--seconds n] [--out <file>]` | Graba los toques en el dispositivo hasta <kbd>Ctrl+C</kbd>. |
| `flow list`, `flow show <flow> [--json]`, `flow remove <flow>` | Los flujos guardados, los pasos de uno y eliminar uno. Un flujo es su id o un archivo `.json`. |
| `flow play <flow>` | Lo reproduce en el dispositivo, escalado a su pantalla. |
| `flow export <flow> --maestro\|--script [--out <file>]` | Un flujo de Maestro, o un script de shell de comandos `emulabzip control`. |

### `emulabzip link`

Abre un deep link y comprueba que el dominio y la app estén de acuerdo sobre él. Consulta [Deep links](deep-links.md).

| Comando | Qué hace |
|---------|----------|
| `link open <url> [--app <app>]` | Lo abre en el dispositivo y lo guarda en el historial. |
| `link check <url> [--app <app>] [--json]` | Lee el `assetlinks.json` o el `apple-app-site-association` del dominio, pregunta al dispositivo y lista los resultados con su solución. Sale con un error cuando uno es un problema. |
| `link verify <app>` | Pide a Android que verifique de nuevo los dominios de la app. |
| `link history [--json]` | Los enlaces abiertos y comprobados antes. |

### `emulabzip openapi check`

`openapi check --spec <file> --har <file> [--json]` comprueba cada respuesta de un archivo HAR contra una descripción OpenAPI 3, y
sale con un error cuando una no coincide. Consulta [Reglas de red](network-rules.md#contract).

### `emulabzip script`

Aprueba los scripts WebAssembly que ejecutan las reglas. `script show <file>`, `script approve <file>`, `script revoke <file or hash>`
y `script list`. Consulta [Scripts de reglas](rule-scripts.md).

### `emulabzip plugin`

Ejecuta tus propios colectores. Consulta [Plugins](plugins.md).

| Comando | Qué hace |
|---------|----------|
| `plugin list [--json]` | Los plugins instalados y si cada uno está activado. |
| `plugin new <name>` | Inicia un plugin desde una plantilla. |
| `plugin install <folder> [--force]` | Copia una carpeta de plugin. Empieza desactivado. |
| `plugin enable <name>` / `plugin disable <name>` | Permite o deja de permitir que un plugin se ejecute. |
| `plugin run <name> [-a <app>] [--seconds <n>] [--json]` | Ejecuta un plugin en el dispositivo e imprime lo que informa. |

### `emulabzip mcp [--allow-writes [--no-confirm]]`

Sirve los dispositivos a un asistente de IA por MCP en la entrada y salida estándar. Consulta [Servidor MCP](mcp.md) y la
[referencia de MCP](mcp-tools.md).

| Indicador | Qué hace |
|-----------|----------|
| `--allow-writes` | También ofrece las herramientas que borran, eliminan, restauran o reescriben datos. Cada llamada te pide confirmar primero |
| `--no-confirm` | Con `--allow-writes`, omite la pregunta. Solo para uso desatendido en un dispositivo desechable |

### `emulabzip trace`

Graba y analiza trazas. Consulta [Traces](traces.md). <kbd>Ctrl+C</kbd> durante una grabación la detiene antes y conserva
lo grabado.

| Comando | Qué hace |
|---------|----------|
| `trace record` | Graba una traza de sistema de Perfetto (Android 9+). `-p, --preset cpu\|frames\|memory\|custom`, `--config <file>` para `custom`, `-D, --duration <s>` (10 por defecto), `-a, --app <pkg>`, `-o, --output <file>`, `--open` para mostrarla en la interfaz de Perfetto. |
| `trace cpu` | Muestrea una app (`-a, --app`) o todo el sistema (`--system`) con simpleperf e imprime las funciones calientes. `-o` guarda el informe, `--folded <file>` los stacks plegados, `--top <n>` la longitud de la lista. |
| `trace instruments` | Graba una plantilla de Instruments desde un simulador (`-t, --template`, Time Profiler por defecto) en un documento `.trace`. Necesita el Developer Mode. |
| `trace summary <file>` | CPU, fotogramas, memoria y los slices más ocupados de una traza de Perfetto. `--json` para scripts. |
| `trace query <file> "<sql>"` | Ejecuta PerfettoSQL sobre una traza de Perfetto. `--json` para scripts. |
| `trace open <file>` | Abre una traza de Perfetto en la interfaz de Perfetto. |
| `trace list` | Lista las grabaciones hechas en la app de escritorio. |

`summary` y `query` descargan el procesador de trazas la primera vez (consulta [Traces](traces.md#the-trace-processor)).

### `emulabzip ca install`

Hace que el dispositivo confíe en el certificado: el almacén del sistema en un emulador Android con acceso root, el llavero en un
simulador de iOS, y para otros dispositivos Android copia el archivo e imprime los pasos. `emulabzip ca path` imprime dónde está
el archivo del certificado.

## Entorno

| Variable | Efecto |
|----------|--------|
| `EMULABZIP_DEVICE` | Valor por defecto de `--device`. |
| `EMULABZIP_LOG=debug` | Diagnósticos detallados en stderr. |
| `EMULABZIP_HOME` | Carpeta de la autoridad certificadora, los ajustes de proxy guardados, las grabaciones y el procesador de trazas. |
| `EMULABZIP_PORT` | Valor por defecto de `net --port`. |
| `ADB_SERVER_SOCKET`, `ANDROID_ADB_SERVER_PORT`, `EMULABZIP_ADB` | Consulta [Android](android.md). |
