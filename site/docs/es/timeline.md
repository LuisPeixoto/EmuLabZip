---
source: ffe4b55ec1e8
description: Pon logs, peticiones, toques, frames lentos y fallos de una sesión en un solo eje de tiempo y míralo con el video de pantalla grabado.
---
# Línea de tiempo y video de pantalla

<p align="center">
  <img src="images/timeline.png" alt="La pestaña Timeline: carriles de toques, peticiones, log, frames lentos, GC y fallos, y el video en el momento del fallo" />
</p>

Una [sesión](sessions.md) es un montón de flujos separados: líneas de log, muestras, peticiones, fallos. Cada uno marca el tiempo a su
manera, así que ver *qué pasó justo antes del fallo* significaba abrir cuatro pestañas y hacer aritmética. La **línea de tiempo**
lo pone todo en un solo eje, y el **video de pantalla** te deja verlo.

## Qué hay en la línea de tiempo

| Carril | Qué | De dónde viene |
|------|------|---------------------|
| **Touches** | Taps, pulsaciones largas y deslizamientos, con el lugar de la pantalla | `getevent` en la pantalla táctil (Android; ver más abajo) |
| **Requests** | Cada petición, durante lo que tardó, en rojo cuando falló | El proxy (`--network`) |
| **Log** | Advertencias y errores | El log |
| **Slow frames** | Frames que la app tardó demasiado en dibujar. Un tirón de treinta frames es un evento: una *ráfaga* | `dumpsys gfxinfo framestats` (Android) |
| **GC** | Recolecciones de basura, con cuánto tardaron y cuánto pausaron la app | Las propias líneas de log de ART (Android) |
| **Crashes** | Cada fallo, en el momento en que ocurrió | Los registros de fallos del dispositivo |
| **Screenshots**, **Video** | Dónde se tomó cada captura, y qué cubre cada fragmento de video | La grabación |

## Grabar lo que necesita la línea de tiempo

```bash
emulabzip session record -o run.ezsession --app com.example.app --network --video --taps
```

- `--taps` graba los toques. Se leen del dispositivo de entrada del kernel, así que son los toques que llegan allí: un
  dedo en la ventana del emulador, o un evento enviado por la consola del emulador (que la app toma por un clic solo
  cuando el evento lleva `ABS_MT_PRESSURE`). `adb shell input tap` se inyecta más arriba, dentro de Android, y **no** se
  ve. iOS no da los toques del simulador a nada fuera de él, así que esto es solo para Android. El kernel informa de una
  posición solo cuando cambia, de modo que el primer toque de una grabación puede no tener ninguna: se conserva y se muestra como "Tap"
  sin coordenadas.
- `--video` graba la pantalla en fragmentos de 30 segundos (`--video 10` para 10). Consulta [Video de pantalla](#screen-video).
- Los frames lentos y las recolecciones de basura no necesitan ninguna opción: se graban cuando hay una app, en Android.

En la app de escritorio, la vista **Sessions** tiene las casillas **Video** y **Touches** junto a **Network**.

## Un solo reloj

Cada flujo marca el tiempo a su manera: el log imprime la hora local del dispositivo sin año, las muestras cuentan desde que
empezaron, las peticiones usan el reloj de este ordenador, y los toques y frames cuentan desde que arrancó el dispositivo. Cuando una grabación
empieza, EmuLabZip pide al dispositivo su reloj, su zona horaria y su tiempo de actividad, y los guarda en la sesión. La
línea de tiempo los usa para poner cada flujo en el reloj de este ordenador, en milisegundos desde el inicio de la grabación.

Una grabación hecha antes de EmuLabZip 0.10 no lo medía. Las horas de su log se calculan a partir de la primera línea (se
escribió poco después del inicio, lo que indica cuál debía de ser la zona), y la línea de tiempo dice que son estimadas.

## Leerla

En la app, abre una sesión y su pestaña **Timeline**. Pasa el cursor sobre un evento para leerlo, haz clic para ver más y para llevar el
video allí, arrastra para moverte por la ejecución, y pulsa ⌘ (Ctrl) y desplaza para hacer zoom. **Open** salta a la petición, el log o
el fallo que hay detrás de un evento.

En la línea de comandos:

```bash
emulabzip session timeline run.ezsession                    # everything, oldest first
emulabzip session timeline run.ezsession --lane requests --lane crashes
emulabzip session timeline run.ezsession --json             # for scripts
```

```text
+00:04.174  taps      Tap (527, 1464)  (154 ms)
+00:04.206  frames    6 slow frames, worst 118 ms  (216 ms) !
+00:04.749  requests  GET time.akamai.com/ 200  (83 ms)
+00:50.000  crashes   java.lang.IllegalStateException: boom !!
```

Un carril guarda como máximo 3.000 eventos. Cuando hay más, se conservan los errores y el resto se reparte de forma uniforme; el recuento
al final de la salida (y en la app, bajo el nombre del carril) dice cuántos había.

Un asistente puede leerla con la herramienta `session_timeline` del [servidor MCP](mcp.md): por carril, limitada, de más antiguo a más reciente.

## Video de pantalla

La pantalla se graba con `screenrecord` en Android y `simctl io recordVideo` en el iOS Simulator, en fragmentos cortos.
Hay tres razones: Android detiene `screenrecord` después de tres minutos pidas lo que pidas, un archivo de película solo puede leerse
cuando la herramienta lo ha cerrado, y un fragmento corto es un clip que puedes usar tal cual, sin un editor de video en la máquina.

- El siguiente fragmento empieza mientras el anterior todavía se copia fuera del dispositivo, así que el hueco entre dos es una fracción de
  segundo en Android y un tercio de segundo aproximadamente en el Simulator.
- Los fragmentos de Android se escalan para que el lado largo mida 1280 píxeles, a 2 megabits por segundo: unos 7 MB por 30 segundos. Los
  del Simulator tienen su propio tamaño, en H.264.
- Al detener la grabación se conserva lo que el fragmento en curso haya grabado.
- El video se guarda en la sesión sin compresión, porque ya está comprimido.

`emulabzip session video run.ezsession -o pieces/` escribe los fragmentos como archivos `.mp4`. En la app, la pestaña Timeline
los reproduce: el video y la línea de tiempo se siguen mutuamente.

Es fácil olvidarse del video cuando compartes un archivo. Una pantalla puede mostrar cualquier cosa: mensajes, un correo, una contraseña. Mira
el video antes de adjuntar una grabación a un issue público.

## En un informe de error

Un informe hecho a partir de una sesión con video incluye el fragmento que contiene el fallo (el último fragmento, cuando no hubo ninguno),
y una **tira** del último minuto antes del fallo: una fila por tipo de cosa, un punto por cada una. La tira se dibuja
a partir de números y no tiene script. Un clip de hasta 20 MB forma parte del archivo HTML; uno más grande se escribe junto a él como
`emulabzip-report.mp4`, al que el archivo hace referencia, así que mantén los dos juntos. Déjalo fuera con `--without video`.
Consulta [Informes de error](bug-reports.md).
