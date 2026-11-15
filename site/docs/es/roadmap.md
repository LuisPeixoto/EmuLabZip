---
source: e1df661a2578
description: "Las fases de desarrollo de EmuLabZip, de la conexión y observación de dispositivos al 1.0: qué trae cada versión, sus límites y las ideas todavía sin fase."
---
# Hoja de ruta

EmuLabZip crece por fases. Cada fase termina con una versión, y cada elemento que incluye funciona en emuladores de Android
y, donde la plataforma lo permite, en simuladores de iOS. Los elementos están más o menos en orden de prioridad dentro de cada fase. Las ideas y los pull
requests son bienvenidos; abre un issue para hablar antes de cualquier cosa grande.

## ✅ Fase 1: Conectar y observar (v0.1)

Descubrimiento automático de emuladores y dispositivos Android (cliente ADB en Rust puro) y de simuladores de iOS arrancados (`simctl`) ·
selector de dispositivos con estado en vivo · selector de apps con los ids de proceso en ejecución · **Overview** con datos del build, el hardware, la pantalla,
la batería, el almacenamiento, el idioma y el host · vista previa de la pantalla en vivo y capturas de pantalla · **visor de logs en streaming** para logcat y
el log unificado: filtros por nivel, etiqueta, texto y expresión regular, filtro por app, pausa, lista virtualizada que soporta
miles de líneas por segundo, exportación a texto plano y a líneas JSON · línea de comandos `emulabzip`: `devices`, `info`, `apps`,
`logs`, `screenshot`.

## ✅ Fase 2: Rendimiento, memoria y almacenamiento (v0.2)

Gráficos en vivo de CPU, memoria, tasa de fotogramas, fotogramas con jank y batería de la app seleccionada · desglose de memoria (Java, nativa,
gráficos, código y stack en Android; regiones de la VM en iOS) con snapshots y un diff · volcados de heap (`.hprof`) en Android y
grafos de memoria (`.memgraph`) en iOS · explorador de archivos del sandbox de la app (`run-as` para apps Android depurables, almacenamiento
compartido, y los contenedores de datos, de bundle y de grupo de apps en iOS) · inspector de SQLite con filas paginadas y consultas
de solo lectura · visores de SharedPreferences, listas de propiedades, JSON, imágenes, texto y hexadecimal · `emulabzip perf`, `mem`, `heapdump`,
`files`, `pull` y `db`. Consulta [Rendimiento](performance.md), [Memoria](memory.md) y [Almacenamiento](storage.md).

## ✅ Fase 3: Red (v0.3)

Un proxy HTTP(S) independiente escrito en Rust (`emulabzip-net`) que descifra HTTP/1.1 y HTTP/2 · enrutamiento con un clic de
un dispositivo Android (`adb reverse` y el ajuste global de proxy) o del simulador de iOS (el proxy del Mac, guardado y
restaurado exactamente) · instalación de certificados: el almacén del sistema en emuladores con root, un certificado de usuario guiado
en el resto, `simctl keychain` en simuladores · lista de peticiones con filtros, cabeceras, cuerpos decodificados y tiempos · los hosts que
rechazan el certificado se dejan pasar · WebSocket y otras conexiones tunelizadas y listadas · exportación a HAR ·
`emulabzip net` y `emulabzip ca`. Consulta [Red](network.md).

## ✅ Fase 4: Trazado (v0.4)

Trazas de sistema de Perfetto con preajustes (CPU y planificación, fotogramas y jank, memoria, personalizado) · un resumen y una consola SQL
respaldados por el `trace_processor` oficial, descargado una vez, fijado y verificado · flame graphs de CPU de `simpleperf`
(una app o todo el sistema) · grabaciones de Instruments con `xctrace`, con un flame graph para Time Profiler · apertura de
trazas en la interfaz de Perfetto o en Instruments · una biblioteca de grabaciones · `emulabzip trace`. Consulta [Trazas](traces.md).

## ✅ Fase 5: Fallos y automatización (v0.5)

Recogida de fallos, fallos nativos y ANR (DropBox en Android, informes `.ips` en el simulador de iOS) con stacks, los datos
que escribió el dispositivo, pistas para la simbolicación y una notificación cuando ocurre uno nuevo · controles de ubicación,
apariencia, barra de estado, batería, permisos, notificaciones y enlaces · un **servidor MCP** para que los asistentes de IA puedan leer
dispositivos, logs, fallos y trazas y actuar sobre ellos · plugins: tus propios recolectores como programas que imprimen líneas JSON,
activados por el usuario. Consulta [Fallos](crashes.md), [Controles](controls.md), [Servidor MCP](mcp.md) y
[Plugins](plugins.md).

## ✅ Fase 6: Herramientas avanzadas de red, diseño y sesiones (v0.6)

**Mocks, retrasos, bloqueos y breakpoints** en peticiones y respuestas · **mensajes de WebSocket**, incluido
`permessage-deflate` · **importación de capturas de paquetes** (`emulator -tcpdump`, `.pcap` y `.pcapng`) con HTTP, nombres de servidor TLS
y DNS · **HTTP/3** del proxy a los servidores · **inspección del diseño** y una **auditoría de accesibilidad** (etiquetas, áreas
táctiles, contraste) en Android y en el simulador de iOS · **sesiones** grabadas en un solo archivo y **modo CI** que
graba alrededor de una ejecución de pruebas · **comparación de dos dispositivos**. Consulta [Mocks](mocks.md), [WebSockets](websockets.md),
[Importar capturas](pcap.md), [Layout](layout.md), [Sesiones](sessions.md), [Modo CI](ci.md) y [Comparar](compare.md).

## Fase 7: Rendimiento y calidad (v0.7) ✅

**Tiempo de arranque de la app**, en frío, templado y en caliente: `am start -W` y las líneas `Displayed` en Android, `simctl launch` con las
signposts de arranque del log unificado en iOS, repetido N veces mostrando la mediana · **energía** por app: `dumpsys
batterystats` (CPU, wakelocks, radio, GPS) en Android, el estado térmico y el tiempo de CPU en el simulador · un **buscador de
fugas** para volcados de heap `.hprof`: tamaños retenidos, dominadores y Activities y Fragments destruidos que siguen
siendo alcanzables, con la ruta hasta la raíz del GC · **presupuestos de rendimiento** en un `emulabzip.toml` (tiempo de arranque, pico de memoria,
fotogramas con jank, número de peticiones) que `emulabzip ci` comprueba y hace fallar la ejecución cuando se incumple uno · **líneas base**: compara una
ejecución con la última buena y di qué se volvió más lento.

## Fase 8: Dentro de los frameworks (v0.8) ✅

**Flutter**: el servicio de la VM de Dart de los builds de depuración (árbol de widgets, tiempos de fotogramas, memoria) · **React Native**: Hermes y el
árbol de React mediante el protocolo del depurador · **Jetpack Compose y SwiftUI** en Layout mediante sus árboles de semántica, con
conteo de recomposiciones donde Android lo expone · decodificación de red para **gRPC y protobuf** (con archivos `.proto`, o
en bruto) y **GraphQL**, con las operaciones nombradas en la lista · **editar y reenviar** una petición · **condiciones de red** en el
proxy: preajustes para 3G, Wi-Fi lenta, latencia alta, sin conexión y pérdida de paquetes.

Pendiente para más adelante de esta fase: los **conteos de recomposición** (Android Studio los obtiene de un agente cargado en la app,
que usa interfaces que no son públicas), los nombres de tipo de vista propios de SwiftUI (la accesibilidad no los expone) y
registrar las condiciones de red en una sesión.

## Fase 9: Stacks legibles e informes de error con un clic (v0.9) ✅

**Simbolicación** de fallos y stacks: retrace de R8 y ProGuard desde `mapping.txt`, dSYM con `atos` en un Mac y
**source maps** de Hermes y Metro, para que los stacks de JavaScript de React Native apunten al archivo y la línea correctos · archivos de mapeo
guardados por app y versión, y `--mapping` en la línea de comandos · un **informe de error** con un clic: la sesión, una
captura de pantalla, los detalles del dispositivo, los logs filtrados y las peticiones fallidas, empaquetados en un archivo HTML o abiertos como
issue en GitHub, GitLab o Jira.

## Fase 10: Línea de tiempo y repetición (v0.10) ✅

Una **línea de tiempo** que alinea toques, problemas del log, peticiones, fotogramas lentos, recolecciones de basura y fallos en un mismo eje
de tiempo, sobre un reloj medido frente al del dispositivo · **vídeo de la pantalla** grabado en una sesión en fragmentos cortos (`screenrecord`
en Android, `simctl io recordVideo` en iOS), sincronizado con la línea de tiempo en la app de escritorio, y el clip alrededor del fallo
añadido a los informes de error · **repetición de red**: una sesión o un archivo HAR responde a las peticiones de la app, que así corre sin conexión contra
respuestas grabadas, para errores cuyo backend ha cambiado desde entonces y para ejecuciones de CI deterministas (`--replay`,
`--fail-on-miss`).

Pendiente para más adelante de esta fase: toques en iOS (el simulador no se los da a nada fuera de él), jank por fotograma en iOS,
repetir WebSockets y llamadas gRPC (un archivo HAR no guarda sus mensajes ni sus trailers) y emparejar una petición
que lleva una marca de tiempo o un id aleatorio.

## ✅ Fase 11: El estado de la app como datos (v0.11)

**[Snapshots del estado de la app](app-state.md)**: guarda todo el sandbox en un archivo `.ezstate` y restáuralo (`run-as` y tar para
apps Android depurables, el contenedor de datos en el simulador), con una copia de seguridad automática de lo que reemplaza una restauración ·
**[snapshots de emulador y ciclo de vida de dispositivos](devices.md)**: crea, inicia, detén, borra los datos y elimina AVDs y simuladores,
y `avd snapshot save|load|delete` · un **[modo de escritura](storage-editing.md)** en Storage para filas de SQLite y
SharedPreferences (listas de propiedades desde la línea de comandos), tras una confirmación, con Undo, y con el solo lectura como
opción predeterminada · visores para **DataStore, MMKV, AsyncStorage, Core Data y Realm** (con un lector del formato de archivo de Realm) · un
**diff de esquemas** entre dos estados, para probar migraciones · **[estado de React Native](react-native-state.md)**: Redux, React
Query, contexto y stores registrados, con un registro de acciones de Redux.

Límites: no se leen los sets, diccionarios, mixed, decimal ni las columnas UUID de Realm; los tipos de MMKV se adivinan; los nombres de
atributo de Core Data pierden las mayúsculas; AsyncStorage y Core Data de un dispositivo iOS se probaron con archivos de ejemplo, no en un
simulador; no se lee el estado de Flutter.

## ✅ Fase 12: Calidad más allá del rendimiento (v0.12)

Una **[auditoría de seguridad y privacidad](security-audit.md)** mientras la app corre: tráfico en texto claro, tokens y datos personales en logs y
preferencias, componentes exportados (`dumpsys package`), los flags `debuggable` y `allowBackup`, excepciones de App Transport Security
en `Info.plist` y dominios que faltan en el manifiesto de privacidad · un **[analizador de APK, AAB e IPA](package-size.md)**: tamaño por
carpeta, dex y assets, un diff entre dos builds y un presupuesto de tamaño en `emulabzip.toml` · **[comprobaciones de localización](localization.md)**:
pseudo-locales (`en-XA`, `ar-XB`), controles de escala de fuente y tamaño de pantalla, un barrido por idiomas y tamaños, y texto que
no cabe detectado mediante Layout · **más comprobaciones de accesibilidad**: orden del foco y simulación de daltonismo (el escalado de
texto mediante el barrido) · **[regresión visual](visual-regression.md)**: capturas de pantalla comparadas con una línea base, con una
imagen de diferencias y una comprobación en CI · un **[inspector de analítica](analytics.md)**: eventos de Segment, Amplitude, Mixpanel y Firebase
decodificados por el proxy, y el log de depuración de Firebase.

Límites: la auditoría no puede ver el atributo `exported` (`dumpsys` no lo imprime) y juzga el texto por patrones; el analizador de
tamaño lee tamaños, carpetas y cabeceras dex, no bytecode, y todavía no una app instalada; el texto cortado es una heurística;
Android cambia el idioma por app, no por dispositivo; el decodificador de Firebase se probó con payloads hechos según el formato de Google,
no con tráfico real; no hay una herramienta de analítica sobre MCP.

## ✅ Fase 13: Automatización y apps híbridas (v0.13)

**WebViews** en Android mediante el Chrome DevTools Protocol, sobre los sockets `localabstract` de ADB sin reenviar nada: su
consola, JavaScript y peticiones dentro de EmuLabZip (las peticiones en la vista Network, marcadas WEBVIEW), y un atajo guiado al
Web Inspector de Safari en iOS · **entrada** en Android (toque, deslizamiento, texto, teclas y lanzar una app) en la app, la línea
de comandos y MCP, y **grabación de un flujo** a partir de `getevent`, que se repite y se exporta como un flujo de Maestro o un script de shell; iOS no
tiene una interfaz pública para los toques, así que esto queda solo para Android · un **probador de deep links** con historial y fijados, que comprueba el
`assetlinks.json` o el `apple-app-site-association` del dominio, la clave de firma de la app, el id de equipo y el entitlement, lo que decidió el dispositivo
(`pm get-app-links`) y quién abre el enlace · **reglas de red programables**: map local y map remote, encadenamiento de reglas,
respuestas comprobadas contra un archivo OpenAPI 3 y scripts de reglas en WebAssembly aislado que se ejecutan solo después de aprobar sus bytes
exactos. Consulta [Páginas web en apps](webviews.md), [Entrada y flujos](automation.md), [Deep links](deep-links.md),
[Reglas de red](network-rules.md) y [Scripts de reglas](rule-scripts.md).

Límites: la inspección de WebViews necesita una app depurable, y iOS es un atajo a Safari, no una consola dentro de la app; los flujos graban
solo la pantalla táctil (no los toques inyectados ni el texto escrito) y etiquetan un toque solo cuando la pantalla se leyó a tiempo; la comprobación
de deep links en un simulador no puede verificar dominios como lo hace un dispositivo; las comprobaciones de OpenAPI cubren respuestas JSON, no peticiones; Map
local y Map remote no se aplican a las llamadas gRPC; los scripts no guardan nada entre llamadas; los asistentes pueden listar WebViews, leer su
consola, comprobar enlaces y reproducir flujos, pero no pueden ejecutar JavaScript en una página ni aprobar un script.

## ✅ Fase 14: IA con contexto (v0.14)

El servidor MCP pasó de 67 herramientas sin estado a **118 herramientas, 8 prompts y 13 recursos** (130 herramientas con los cambios activados) que
mantienen estado mientras se ejecutan: el **proxy y las peticiones capturadas** (iniciar, conectar un dispositivo, filtrar y leer peticiones, reglas,
condiciones, repetición, breakpoints, exportación a HAR, comprobaciones de OpenAPI), **vigilancias en vivo** de logs, rendimiento, toques y flujos de WebView y
React Native, **grabar una sesión** y comparar dos, **datos de la app de solo lectura** (SQLite, Realm, Core Data, preferencias,
DataStore, MMKV, AsyncStorage), lectura de fallos con mapeos guardados, creación de dispositivos, flujos, líneas base visuales, barridos de idioma
y presupuestos de build · cada herramienta tiene un título e indica si lee, cambia o destruye · **prompts** para las tareas comunes
(explicar un fallo, por qué va lento, resumir una sesión, reproducir un error, depurar una petición, revisar una versión, comprobar un enlace, simular un
backend) y **recursos** que un asistente puede leer (dispositivos, sesiones, flujos, la captura en vivo, una guía) · **las escrituras son opcionales**:
`emulabzip mcp --allow-writes` ofrece las herramientas que borran, eliminan, restauran o reescriben datos, y cada llamada pide a la persona que
confirme a través del cliente; aprobar scripts y plugins, y `ci`, nunca se ofrecen · una tarjeta **AI assistants** en Settings
con la configuración para Claude Code, Claude Desktop, Cursor y VS Code · una [referencia de MCP](mcp-tools.md) generada que una
prueba mantiene sincronizada con el servidor. Consulta [Servidor MCP](mcp.md).

Límites: editar filas y preferencias, deshacer una edición, grabar un flujo por toques e importar una captura de paquetes no se ofrecen
sobre MCP (siguen en la app y en la línea de comandos); una herramienta larga todavía no informa del progreso; los fotogramas de vídeo de una sesión no
se devuelven como imágenes; un cliente que no puede pedir confirmación no puede usar las herramientas de escritura salvo que se indique `--no-confirm`;
las vigilancias guardan los últimos 5,000 elementos y terminan tras diez minutos de inactividad.

## Fase 15: Dispositivos y plataformas (v0.15)

**iPhones y iPads físicos** mediante las herramientas que incluye Xcode (`devicectl`, `usbmuxd`): logs, apps, capturas de pantalla, fallos
y archivos donde iOS lo permite, y una nota clara donde no · **Android por Wi-Fi**, con el emparejamiento de la depuración inalámbrica desde la app · emuladores de **Wear OS y Android TV**, y simuladores de **watchOS, tvOS y visionOS**, con las
vistas que se aplican a cada uno · **varios dispositivos a la vez**: logs y peticiones de dos dispositivos en una sola lista, etiquetados por dispositivo.

## Fase 16: Equipo y ecosistema, 1.0 (v1.0)

**Comparar sesiones** lado a lado, y un **informe HTML para compartir** de una sesión (un archivo, sin servidor) · **bibliotecas de reglas
compartidas**: un archivo de reglas guardado en el repositorio de la app, cargado por la app y por la línea de comandos · una **extensión de VS Code**
con dispositivos, logs y el servidor MCP en el editor · un **catálogo de plugins**: plugins de la comunidad listados con checksums, que el usuario
sigue activando uno por uno · una **interfaz en portugués** y otros idiomas · **actualizaciones**: actualización automática firmada en
la app, y paquetes de Homebrew, winget y Scoop · la **promesa de 1.0**: las opciones y el JSON de la línea de comandos, las herramientas y los prompts de MCP
y el formato `.ezsession` son estables y solo cambian con una versión mayor.

## Ideas

Lo que todavía no está en una fase: layouts para apps que dibujan su propia interfaz (juegos, canvas) · streaming de HTTP/3 para respuestas muy grandes ·
plugins que lean sesiones · un **SDK acompañante** opcional (npm, Gradle, SwiftPM) para lo que solo el código dentro de
la app puede ver, como el estado de Redux, Zustand o React Query, conteos de recomposición y feature flags, separado de
la opción predeterminada, que no instala nada en el dispositivo.
