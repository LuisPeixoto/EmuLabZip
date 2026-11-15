---
source: fe2e62872ca2
description: La herramienta Crashes lista fallos, fallos nativos y ANR de un dispositivo, con su stack, pistas y avisos de nuevos fallos, en Android y en iOS Simulator.
---
# Crashes

La herramienta Crashes lista los fallos, los fallos nativos y los ANR que un dispositivo ha registrado, y abre cada uno con
su stack, los datos que anotó el dispositivo y qué probar a continuación. Los nuevos aparecen solos, con una notificación
y una insignia en la barra lateral, mientras EmuLabZip está abierto.

| Tipo | Android | iOS Simulator |
|------|---------|---------------|
| **Crash** | Una excepción no capturada, con su cadena de `Caused by:` | Un fallo del runtime de Swift, un `abort()` o una `NSException` no capturada |
| **Native crash** | Una señal fatal, del tombstone | Un acceso a memoria inválido (`EXC_BAD_ACCESS`) y similares |
| **ANR** | Application Not Responding, con el stack de cada hilo | no aplica |

<p align="center">
  <img src="images/crashes.png" alt="Un ANR con el hilo principal bloqueado, pistas y detalles" />
</p>

## Inicio rápido

1. Abre **Crashes** (<kbd>⌘8</kbd>). El fallo más reciente se abre solo.
2. Elige otro a la izquierda, o filtra por tipo, o por la app elegida en la barra de herramientas.
3. **Report** muestra las pistas, los detalles y los hilos. **Raw** muestra el informe tal como lo escribió el
   dispositivo. **Save…** lo escribe en un archivo.
4. Un build de release embrolla sus nombres. Añade su `mapping.txt`, source map o dSYM cuando el fallo lo pida, y el fallo
   se vuelve a leer, con **Readable** y **Original** para alternar entre ellos. Consulta [Stacks legibles](symbolication.md).
5. **Report a bug** en la barra de herramientas empaqueta el fallo, una captura de pantalla, el log y las peticiones
   fallidas en un solo archivo. Consulta [Informes de error](bug-reports.md).

Desde una terminal:

```bash
emulabzip crashes --app com.example.app          # newest first, with an id for each
emulabzip crash dropbox:data_app_crash:20261023205928
emulabzip crash <id> --all-threads               # every thread, not only the one that crashed
emulabzip crash <id> --raw                       # as the device wrote it
emulabzip crash <id> --mapping mapping.txt       # readable, with a file you name
```

## De dónde vienen los informes

**Android.** El sistema guarda una copia de cada fallo, ANR y tombstone en DropBox, y la shell puede leerla con
`dumpsys dropbox --print <tag>`. Eso significa que funciona en cualquier dispositivo y emulador, sin root y sin instalar
nada. EmuLabZip lee `data_app_crash`, `system_app_crash`, `data_app_native_crash`, `SYSTEM_TOMBSTONE`,
`data_app_anr` y `system_app_anr`. Los ids se ven como `dropbox:data_app_crash:20261023205928`. El sistema limita la
frecuencia de los fallos repetidos de una app, así que un fallo en bucle se lista con menos frecuencia de la que ocurre.

**iOS Simulator.** macOS escribe un informe `.ips` en `~/Library/Logs/DiagnosticReports` cuando un proceso falla, incluidas
las apps simuladas. Un informe pertenece a un simulador cuando la ruta de la app pasa por
`CoreSimulator/Devices/<udid>`. EmuLabZip lee los 200 archivos más recientes allí y en `Retired`. Los ids se ven como
`ips:MyApp-2026-10-23-210246.ips`. macOS escribe el informe unos segundos después del fallo.

## Leer un informe

El hilo que falló está abierto; los demás están plegados. En un ANR, el hilo principal bloqueado va primero. Cada frame
tiene un cuadrado de color que indica de quién es el código, los mismos cuatro colores que el flame graph:

| Color | Significado |
|--------|---------|
| Azul | El código propio de la app |
| Verde | La plataforma: el framework y el runtime de Android, UIKit, Foundation, el runtime de Swift |
| Naranja | Bibliotecas nativas |
| Gris | El kernel |

**Only the app's code** oculta todo lo demás, que es la forma de leer un stack nativo formado sobre todo por interioridades
del runtime.

Un fallo de Java muestra una sección por cada excepción de la cadena, y la última, la causa raíz, es la que está marcada
como fallida. El título del fallo es esa causa raíz.

## Pistas

EmuLabZip dice lo que puede deducir del informe:

- Nombres de clase que parecen ofuscados: añade el `mapping.txt` del build y se leen por ti. Consulta
  [Stacks legibles](symbolication.md).
- Posiciones de JavaScript de una app de React Native: añade el source map del bundle.
- Frames nativos de la app sin símbolos: `ndk-stack -sym <folder> -dump tombstone.txt`, o `llvm-symbolizer`. El
  BuildId tiene que coincidir con el build que falló.
- Frames de iOS de la app sin símbolos: añade el dSYM del build y se leen con `atos`, o ejecuta el comando `atos`
  que se muestra, con la arquitectura y la dirección de carga rellenadas.
- Un ANR cuyo hilo principal estaba durmiendo o esperando: el hilo principal estaba bloqueado.
- Un ANR en un dispositivo con poca CPU: puede que la app no tenga la culpa.
- Un fallo nativo cuya señal vino de `kill()`: fue enviada, no provocada por un error.

## Pruébalo

`tools/crash-app` (Android) y `tools/crash-app-ios` (iOS Simulator) son apps diminutas que fallan a propósito, para que
puedas ver cada tipo sin escribir una. Cada una tiene un `install.sh` y un README.
