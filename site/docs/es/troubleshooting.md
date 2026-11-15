---
source: a63f6e8c9dee
description: "Soluciones a los problemas habituales de EmuLabZip: dispositivos que no aparecen, logs vacíos, Network, Traces, Crashes, Controls, Flows, plugins y MCP."
---
# Solución de problemas

## Mi emulador Android no aparece

1. Ejecuta `adb devices`. Si el emulador no aparece allí, EmuLabZip tampoco puede verlo. Reinícialo, o ejecuta
   `adb kill-server && adb start-server`.
2. El pie del device picker muestra **adb unavailable** cuando no se encuentra `adb`. Instala las platform tools del
   Android SDK, o apunta `ANDROID_HOME` o `EMULABZIP_ADB` a tu SDK.
3. Un dispositivo listado como **unauthorized** está esperando que aceptes el aviso de depuración por USB.
4. Un puerto personalizado del servidor ADB también debe establecerse con `ANDROID_ADB_SERVER_PORT` o `ADB_SERVER_SOCKET`
   para EmuLabZip.

## Mi simulador no aparece

- Solo se listan los simuladores **de iOS arrancados**. Compruébalo con `xcrun simctl list devices booted`.
- El pie muestra **simctl unavailable** cuando faltan las herramientas de línea de comandos de Xcode. Ejecuta
  `xcode-select --install`, o `sudo xcode-select -s /Applications/Xcode.app` si hay varias versiones de Xcode instaladas.

## El log está vacío

- En Android, comprueba que `adb logcat` imprime líneas.
- Mira el nivel mínimo y los filtros; **Reset filters** aparece cuando hay líneas pero ninguna coincide.
- El filtro de app necesita que la app esté en ejecución. Inicia la app; el filtro recoge el nuevo id de proceso
  automáticamente.

## El log está demasiado cargado

Consulta [El log unificado está muy cargado](ios-simulator.md#the-unified-log-is-busy). En Android, sube el nivel mínimo o
filtra por tag.

## Las capturas de pantalla fallan

- En Android, `adb exec-out screencap -p > shot.png` debe funcionar. Algunas pantallas seguras (video con DRM, apps
  bancarias) salen negras por diseño.
- En iOS, `xcrun simctl io booted screenshot shot.png` debe funcionar.

## Storage dice "only debuggable apps can be browsed"

Android solo deja que `run-as` lea los datos privados de apps depurables. Instala un build de depuración de tu app, o
prueba las herramientas con la app de ejemplo (`tools/sample-app/install.sh`). El almacenamiento compartido funciona con
todas las apps.

## Los volcados de heap fallan en Android

`am dumpheap` necesita una app depurable, o una imagen de emulador del tipo `userdebug`/`eng` (imágenes sin Google Play).
El error muestra el motivo que dio Android, por ejemplo `Process not debuggable`.

## Performance no muestra la frecuencia de fotogramas

Los frames solo se cuentan mientras la app dibuja. Interactúa con la app; una pantalla estática no renderiza nada. En el
iOS Simulator, las métricas de frames llegan con las trazas de Instruments en la Fase 4.

## Network: no aparece nada

- Comprueba que el dispositivo dice *sending traffic here*. En Android, `adb shell settings get global http_proxy` debería
  imprimir `127.0.0.1:<port>`.
- Algunas apps ignoran el proxy del sistema (las que abren sockets por su cuenta, o usan HTTP/3). No se pueden capturar.
- Las apps que usan mucho la caché puede que no consulten la red en absoluto. Borra sus datos o usa una URL nueva.

## Network: las peticiones HTTPS fallan o aparecen como "Connections"

El dispositivo no confía en el certificado, o la app fija el de su servidor. Pulsa **Certificate** y sigue lo que dice. En
las imágenes de Android de Google Play solo Chrome y las apps que admiten certificados de usuario confiarán en él;
consulta [el certificado](network.md#the-certificate). Un banner nombra los hosts que se están dejando pasar; usa **Decrypt
again** después de arreglarlo.

## Network: el dispositivo o el Mac sigue apuntando al proxy

Pulsa **Disconnect**, o ejecuta `adb shell settings put global http_proxy :0` en Android. Para el Mac, abre EmuLabZip una
vez: restaura los ajustes de proxy que guardó si una ejecución anterior se interrumpió. Si no, desactiva los proxies en
*System Settings › Network › Details › Proxies*.

## Network: macOS pide una contraseña

Cambiar el proxy del Mac es un ajuste del sistema. EmuLabZip lo intenta como tú primero y pide derechos de administrador
solo si macOS lo rechaza.

## Network: una regla no hace nada

Las reglas ven las peticiones descifradas, así que HTTPS necesita el certificado instalado, y un host que lo rechazó se
deja pasar y no se puede cambiar. Gana la primera regla activada que coincide; mueve la que esperas por encima de las
demás. Un patrón de host coincide solo con el host (`api.example.com`, no `https://api.example.com/`).

## Network: HTTP/3 no muestra `h3`

La función está desactivada de forma predeterminada (**HTTP/3 → Use HTTP/3 to servers**). Un host solo se prueba después
de que una de sus respuestas dijera que admite HTTP/3, así que la primera petición siempre es TCP. Una red que bloquea UDP
hace fallar QUIC; el host se queda entonces en TCP durante 10 minutos.

## Layout: no hay nada que leer en iOS

Simulator.app tiene que estar abierto en ese dispositivo, y el programa que ejecuta EmuLabZip tiene que tener permiso en
System Settings, Privacy & Security, Accessibility (la app, o tu terminal para `emulabzip layout`). Despierta el
dispositivo si muestra una pantalla negra.

## Flutter: "Could not find where the Dart VM service listens"

Solo los builds de debug y profile tienen uno, y la app escribe su dirección en el log una vez, cuando arranca. Inicia la
app de nuevo, o da con `--uri` la dirección que sale en la salida de `flutter run`. Un build de release no tiene VM
service.

## Flutter: el árbol no tiene nombres de archivo

La app no se compiló con `--track-widget-creation`. `flutter run` lo añade; para `flutter build` añade
`--extra-front-end-options=--track-widget-creation`.

## React Native: "Metro is not running" o "does not list a runtime"

Inicia Metro (`npx react-native start`) y abre la app en un dispositivo que lo alcance (`adb reverse tcp:8081 tcp:8081` en
Android). Metro lista un runtime solo mientras la app está conectada a él, y deja entrar a un solo depurador a la vez:
cierra React Native DevTools y el inspector de Chrome, y vuelve a intentarlo. Los builds de release no tienen Metro ni el
hook de React DevTools.

## State: "RN state is empty" o "no DevTools hook"

Redux, React Query y los contextos se encuentran mediante el hook de DevTools de React, que solo tienen los builds de
desarrollo. En un build de release registra los stores en `globalThis.__EMULABZIP_STORES__` (consulta
[Estado de React Native](react-native-state.md)). Un store usado sin un `Provider` de react-redux también tiene que
registrarse.

## State: falla restaurar o guardar en Android

La app tiene que ser depurable (como para [Storage](storage.md)). `run-as` y `tar` se ejecutan en el dispositivo: una
imagen de emulador con un `run-as` bloqueado no puede guardar un estado. Una restauración que dice que se rechazó el tar
encontró una entrada fuera de la carpeta de la app; el archivo no es de EmuLabZip, o está dañado.

## Edit: "the table cannot be changed"

Una fila se encuentra por `rowid` o por su clave primaria. Una vista, o una tabla sin ninguna de las dos, no tiene nada
que distinga sus filas: cámbiala con SQL en la app. Una edición que se lee de vuelta de forma distinta restaura el archivo
anterior y lo dice.

## Devices: un emulador no arranca

`emulabzip avd boot` necesita el `emulator` del SDK (se encuentra a partir de `ANDROID_HOME`, `ANDROID_SDK_ROOT` y las
carpetas habituales). Ejecuta `emulator -avd <name>` tú mismo para ver su propio mensaje. Un emulador en ejecución nunca
se limpia ni se borra: apágalo primero.

## Audit: "the area could not be read"

El log necesita que la app esté **en ejecución**, para distinguir sus líneas de las del sistema: inicia la app y úsala, y
luego audita de nuevo. Storage en Android necesita una app depurable, como en [Storage](storage.md). Las peticiones
vienen del proxy (inícialo en la vista Network, o pasa `--har`).

## Language: "Android changes the language of one app at a time"

Android no tiene forma de cambiar el idioma de un dispositivo desde la shell, solo el de una app (`cmd locale set-app-locales`,
Android 13 y posteriores). Pasa `--app`, o elige la app en la barra de herramientas. El iOS Simulator cambia el idioma de
todas las apps, y cada una lo toma cuando se inicia de nuevo.

## Text checks: se informa de una etiqueta que no está cortada

`text-cut-off` es una heurística: una sola línea cuyo recuadro quedó limitado al espacio que le dejó su contenedor (o la
vista siguiente de su fila), y cuyo texto llega hasta el final del recuadro. Compruébalo al 130% y al 200%; si cabe,
ignóralo.

## Visual: "the screenshots are not the same size"

Vienen de dispositivos, orientaciones o tamaños de pantalla distintos. Compara cosas iguales: el mismo dispositivo,
orientación y tamaño de pantalla (`control reset-display` devuelve uno de Android a su estado).

## Network: un build de Gradle no puede descargar su distribución

No es un asunto de EmuLabZip, pero surge con las apps de demostración: algunas redes dejan que `curl` llegue a los
servidores de Gradle mientras Java agota el tiempo de espera. Descarga la distribución con `curl` y colócala donde Gradle la
espera (`~/.gradle/wrapper/dists/<name>/<hash>/`), o compila con un Gradle que ya tengas.

## Layout: Android dice que uiautomator no produjo nada

Algunas ventanas lo prohíben: pantallas seguras y unos pocos reproductores. Ve a otra pantalla e inténtalo de nuevo. Si el
dispositivo está ocupado (una animación de arranque, un diálogo de ANR), espera un momento.

## Sessions: una grabación queda como una carpeta `.partial`

La grabación se interrumpió antes de poder empaquetarse. No se pierde nada: `emulabzip session show run.ezsession.partial`
la abre. Borra la carpeta, o muévela, antes de grabar de nuevo con el mismo nombre.

## Timeline: faltan los toques

Los toques se leen del dispositivo de entrada del kernel con `getevent`, así que se ve un dedo sobre la ventana del
emulador o un evento enviado por la consola del emulador. `adb shell input tap` se inyecta más arriba, dentro de Android,
y no se ve. iOS no entrega los toques del simulador a nada fuera de él, así que allí no hay ninguno. Graba con `--taps`.

## Timeline: las horas del log son "estimated"

La grabación se hizo antes de EmuLabZip 0.10, o el dispositivo no pudo decir su hora, así que las horas locales del log
se colocaron comparando la primera línea con el inicio de la grabación. Pueden desviarse la diferencia entre los dos
relojes. Graba de nuevo para obtener horas exactas.

## Video: "screenrecord made no video"

Algunas pantallas no se pueden grabar (ventanas seguras) y unas pocas imágenes no admiten `screenrecord` en absoluto. El
mensaje lleva lo que imprimió la herramienta. En el iOS Simulator, `recordVideo` necesita que el simulador esté arrancado;
si no empieza en 20 segundos, el mensaje dice por qué.

## Video: el informe tiene un `.mp4` aparte

Un clip de hasta 20 MB forma parte del archivo HTML. Uno más grande se escribe a su lado como `emulabzip-report.mp4`, y el
archivo se refiere a él con ese nombre: mueve o adjunta los dos juntos.

## Replay: "not in the recording"

La app pidió algo que la grabación no contiene: otra dirección, otra consulta, un método que no se usó, o una petición
que falló cuando se grabó. El proxy responde 504 y lo lista en **Not recorded** (o al final de
`emulabzip net --replay`). Graba de nuevo con esa petición, o añade `--passthrough` para dejar que llegue al servidor. La
coincidencia ignora el orden de los parámetros de la consulta y el puerto, pero no los valores: una app que añade una
marca de tiempo o un id aleatorio a cada petición fallará siempre.

## Replay: fallan los WebSockets y algunas conexiones

Mientras se reproduce una grabación, lo que no se puede responder desde un archivo HAR se rechaza, para que no pueda
llegar a internet: WebSockets, llamadas gRPC, apps que no aceptan el certificado de EmuLabZip y conexiones sin TLS que el
proxy no puede leer. Usa `--passthrough` si deben funcionar.

## Traces: "Perfetto did not record a trace"

Perfetto necesita Android 9 o posterior. Si el mensaje nombra un permiso, reinicia el emulador, o ejecuta
`adb shell setprop persist.traced.enable 1` y graba de nuevo. Una configuración personalizada con un error de sintaxis la
informa el propio Perfetto; compruébala con `adb shell perfetto --txt -c - -o /data/misc/perfetto-traces/test <config.txt`.

## Traces: falla un perfil de CPU

simpleperf solo muestrea apps que son *debuggable* o *profileable*, y la app debe estar en ejecución. Para todo el sistema
necesitas una imagen de emulador que permita `adb root` (no Google Play). "No samples were recorded" significa que la app
no hizo nada mientras grababa: úsala y graba de nuevo.

## Traces: Instruments dice que el Developer Mode está desactivado

Ejecuta `sudo DevToolsSecurity -enable` en una terminal, escribe tu contraseña y graba de nuevo. `DevToolsSecurity -status`
muestra el estado actual.

## Traces: falla la descarga del trace processor

Viene de `github.com/google/perfetto/releases`. Detrás de un proxy, establece `HTTPS_PROXY`. Un archivo que no coincide con
el SHA-256 fijado se descarta y nunca se ejecuta. Aun así puedes abrir cualquier traza de Perfetto en la Perfetto UI.

## Crashes: no se lista nada

En Android, el sistema guarda un fallo en DropBox y limita las repeticiones, así que una app en bucle se lista con menos
frecuencia de la que falla. En el iOS Simulator, macOS escribe el informe `.ips` unos segundos después del fallo, en
`~/Library/Logs/DiagnosticReports`; los informes de apps que no están en la carpeta de un simulador no se listan. Pulsa
**Refresh**, o ejecuta `emulabzip crashes`.

## Crashes: los nombres siguen embrollados

Un mapping solo se usa para la versión de la app para la que se hizo, y el fallo dice cuál es ("Add the
mapping.txt of com.example 1.2.0 (42)"). Añade el archivo con exactamente esa versión: `emulabzip mappings add com.example
"1.2.0 (42)" mapping.txt`. Un archivo de otro build daría nombres erróneos, así que no se usa. Los frames también se quedan
como están cuando R8 no tenía números de línea que conservar: añade `-keepattributes SourceFile,LineNumberTable` a las
reglas del próximo build.

## Crashes: React Native muestra `1:608617` en lugar de un archivo

Un build de release ejecuta un bundle, y sus posiciones necesitan el source map de ese bundle. Para Hermes usa el mapa
compuesto, `android/app/build/generated/sourcemaps/react/release/index.android.bundle.map`, no los de `packager` ni
`compiler`. También tiene que ser el mapa del build que falló.

## Crashes: iOS muestra direcciones sin más

Añade el `.dSYM` del build que falló (`emulabzip mappings add com.example.app "1.2.0 (42)" MyApp.app.dSYM`). Su UUID tiene
que coincidir con el de la app, así que un dSYM de otro build, incluso de la misma versión, no se usa. Leer direcciones
necesita un Mac, porque usa `atos`.

## Bug report: no hay peticiones fallidas

Las peticiones vienen del proxy de Network. Inícialo y conecta el dispositivo antes de que ocurra el error; un informe
hecho a partir de una [sesión](sessions.md) grabada con `--network` también las tiene.

## Controls: la ubicación no hace nada en un dispositivo

Solo los emuladores aceptan una ubicación nueva, mediante la consola del emulador. Necesita el token de
`~/.emulator_console_auth_token`, que el emulador crea la primera vez que se ejecuta desde tu cuenta. Los dispositivos
físicos y los simuladores de otros tipos no son compatibles con la ubicación en Android.

## Controls: una notificación no aparece en Android

Las notificaciones se publican desde la shell con `cmd notification post` (probado en Android 14). Activa las
notificaciones de "Android Shell" si el dispositivo las oculta. El argumento de app se ignora en Android.

## Web pages: mi WebView no aparece en la lista

Un WebView solo aparece en la lista cuando su app llama a `WebView.setWebContentsDebuggingEnabled(true)` (un build de
depuración suele hacerlo) y la página está abierta. Chrome aparece mientras está abierto. La lista se lee cuando abres
**Frameworks** y cuando pulsas **Refresh**. En iOS usa el menú Develop de Safari; consulta
[Páginas web en apps](webviews.md).

## Flows: mis toques no se graban

Solo se lee la pantalla táctil: toca la propia ventana del emulador. Los toques enviados desde EmuLabZip (**Interact**,
**Play**) o con `adb shell input` nunca llegan a ella, y el texto escrito no se graba. Un toque recibe el nombre de la
vista que tocó solo cuando la pantalla se leyó a tiempo, lo que ocurre dos o tres segundos después del toque anterior; el
punto se conserva de todos modos. Consulta [Entrada y flujos](automation.md).

## Deep links: la comprobación dice que el dominio no está verificado

En Android, pídele al dispositivo que lo intente de nuevo con **Verify again** o `emulabzip link verify <app>`, y comprueba
pasados unos segundos. Una huella que no coincide suele significar que el archivo lista la clave de release y el build del
dispositivo está firmado con la clave de debug. Un archivo de dominio que redirige (a `www`, o de http a https) lo
rechazan ambos sistemas. Consulta [Deep links](deep-links.md).

## Network: una regla de script no hace nada

Un script se ejecuta solo después de que se aprobaran sus bytes exactos (**Approve this script** en la regla, o
`emulabzip script approve`). Cambia el archivo y hay que aprobarlo de nuevo. Si un script se ejecuta pero la petición no
cambia, los detalles del flujo dicen qué escribió el script o por qué falló. Consulta [Scripts de reglas](rule-scripts.md).

## Plugins: un plugin no se puede ejecutar

Primero tiene que estar activado, y editar su `plugin.json` lo desactiva de nuevo. Un plugin que termina con un error
muestra el final de su salida de error. Comprueba que el programa de `command` existe (se busca en la carpeta del plugin y
luego en el PATH) y que es ejecutable (`chmod +x run.sh`).

## MCP: el asistente no ve ninguna herramienta

Ejecuta `emulabzip mcp` a mano y envíale una petición `initialize`, como en [Servidor MCP](mcp.md#trying-it). Si el cliente
no encuentra `emulabzip`, dale la ruta completa. La salida estándar lleva solo el protocolo; mira el error estándar
(`EMULABZIP_LOG=debug`) para saber qué salió mal.

## Obtener diagnósticos

Inicia la app o la CLI con `EMULABZIP_LOG=debug` e incluye la salida cuando
[abras un issue](https://github.com/LuisPeixoto/EmuLabZip/issues/new?template=bug_report.yml).
