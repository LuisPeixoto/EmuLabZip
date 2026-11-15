---
source: 3928a9800ad4
description: Qué descubrimos al estudiar cómo Xcode Instruments y los profilers de Android Studio recogen sus datos, y qué interfaces públicas usa EmuLabZip en su lugar.
---
# Cómo funcionan las herramientas oficiales

Antes de escribir EmuLabZip estudiamos cómo Xcode Instruments y los profilers de Android Studio recogen sus datos. El
estudio fue estático: miramos los paquetes de las aplicaciones tal como se distribuyen, las bibliotecas que enlazan, las clases y paquetes
dentro de los archivos JAR y las cadenas de los binarios que corren en el dispositivo. No se parcheó ni se redistribuyó nada. El objetivo era
entender **qué fuentes de datos existen** y después elegir las que una herramienta de código abierto puede usar mediante interfaces públicas y
documentadas.

## Xcode Instruments

| Área | Qué encontramos |
|------|-----------------|
| Lenguajes e interfaz | Swift y Objective-C, AppKit con SwiftUI, además de frameworks privados de gráficos y de árboles de llamadas |
| Plugins | Paquetes `.xrplugin` y paquetes de instrumentos `.instrdst` (Networking, HTTP Tracing, SwiftUI, Hangs, Hitches, System Trace…) |
| Transporte | **DTX**, un protocolo RPC privado, sobre sockets, puertos Mach o memoria compartida, con compresión |
| Agente en el dispositivo | `DTServiceHub` expone canales como `sysmontap` (estadísticas de procesos y del sistema), `coreprofilesessiontap` (traza del kernel), `objectalloc`, `vmtracking` y `activitytracetap` (os_log y signposts) |
| Fuentes de datos | Trazado del kernel (`ktrace`, `kperf`), el sistema de logging unificado y los signposts, simbolicación |
| Red | Estadísticas de conexión (`netstats`) y trazado HTTP de CFNetwork emitido a través del log unificado, no un proxy |
| Línea de comandos | `xctrace record` y `xctrace export` |

## Xcode y el simulador

`simctl` controla **CoreSimulator** mediante un servicio XPC. Puede listar y arrancar dispositivos, ejecutar comandos dentro de un
simulador (`simctl spawn … log stream`), capturar la pantalla (`simctl io screenshot`, `recordVideo`), listar apps y sus
contenedores, sobrescribir la barra de estado, fijar la ubicación, enviar notificaciones push y añadir certificados raíz. Una app de
simulador es un proceso normal de macOS, así que el host puede leer directamente su uso de CPU y de memoria.

## Android Studio

| Área | Qué encontramos |
|------|-----------------|
| Plataforma | IntelliJ Platform sobre JetBrains Runtime, Kotlin y Java, Swing con Compose for Desktop para la interfaz nueva del profiler |
| Acceso al dispositivo | ddmlib y adblib (ADB), y la API gRPC del emulador, descubierta a partir de los archivos `avd/running/pid_*.ini` |
| Profilers | Una *tubería de transporte*: grpc-java y protobuf entre Studio y un daemon en el dispositivo |
| Daemon del dispositivo | `transport` (antes `perfd`), enviado a `/data/local/tmp/perfd/`. Ejecuta `dumpsys meminfo --checkin`, `dumpsys SurfaceFlinger --latency`, `perfetto`, `simpleperf`, `am dumpheap` y carga un agente JVMTI en las apps depurables |
| Trazas | Perfetto (`trace_processor` como servicio gRPC), informes de simpleperf, trazas de métodos de ART |
| Memoria | Volcados de heap `.hprof` analizados con Shark, asignaciones nativas mediante heapprofd en Perfetto |
| Red | Antes, reescritura de bytecode de OkHttp y `HttpURLConnection` con ASM (`profilers-transform.jar`); hoy, el inspector de red de App Inspection, que se engancha a las pilas HTTP dentro de las apps depurables |

## Qué usa EmuLabZip

Solo usamos **interfaces públicas y documentadas**. El protocolo DTX de Apple y los agentes en el dispositivo de Google son privados, cambian
entre versiones y no nos corresponde redistribuirlos.

| Dato | Android | Simulador de iOS |
|------|---------|------------------|
| Dispositivos | Protocolo host de ADB (`host:devices-l`), hablado directamente desde Rust | `xcrun simctl list devices -j` |
| Datos del dispositivo | `getprop`, `wm`, `dumpsys battery`, `/proc` | JSON de `simctl`, lista de runtimes, `defaults` dentro del simulador |
| Apps | `pm list packages`, `ps` | `simctl listapps`, `launchctl list` dentro del simulador |
| Logs | `logcat -v threadtime` | `log stream --style ndjson` dentro del simulador |
| Capturas de pantalla | `screencap -p` | `simctl io screenshot` |
| CPU y memoria (Fase 2) | `/proc/<pid>/stat`, `dumpsys meminfo` | Estadísticas de procesos del host |
| Fotogramas (Fase 2) | `dumpsys gfxinfo framestats` | Signposts y `xctrace` (Fase 4) |
| Trazas (Fase 4) | Perfetto, simpleperf | `xctrace record` y `export` |
| Red (Fase 3) | Proxy HTTP con una CA de usuario, `emulator -tcpdump` | Proxy del host, `simctl keychain add-root-cert` |

Esto mantiene EmuLabZip pequeño, seguro para ejecutarlo contra cualquier imagen de emulador y estable frente a las actualizaciones de
Xcode y Android Studio. También significa que no se instala nada en el dispositivo: cada consulta es algo que tú mismo podrías escribir en
`adb shell` o `xcrun simctl`.
