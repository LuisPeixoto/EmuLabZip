---
source: d9e8d27bf968
description: "Una sesión graba lo que pasó en un dispositivo mientras se usaba una app: logs, rendimiento, red, fallos, capturas y video en un solo archivo .ezsession."
---
# Sesiones

Una **sesión** es una grabación de lo que ocurrió en un dispositivo mientras se usaba una app, guardada como un solo archivo que puedes abrir
más tarde, adjuntar a un informe de error o recoger desde CI.

| Dentro | Qué |
|--------|------|
| `session.json` | El dispositivo, la app, cuándo empezó y terminó |
| `logs.jsonl` | Cada línea de log (hasta 500.000) |
| `perf.jsonl` | Muestras de CPU, memoria, tasa de frames y batería, una por segundo |
| `network.har` | Las peticiones, cuando se grabó la red, con mensajes WebSocket |
| `crashes/` | Cada fallo, fallo nativo y ANR ocurrido *durante* la ejecución, con su informe |
| `screens/` | Capturas de pantalla tomadas cada pocos segundos, cuando se piden, con `index.jsonl` indicando cuándo |
| `input.jsonl` | Toques (taps, pulsaciones largas, deslizamientos), cuando se piden, en Android |
| `frames.jsonl` | Frames que la app dibujó demasiado despacio, en Android |
| `video/` | La grabación de pantalla en fragmentos, cuando se pide, con `index.jsonl` indicando qué cubre cada uno |

`session.json` también guarda el **reloj** del dispositivo, de modo que los flujos pueden ponerse en un solo eje: consulta
[Línea de tiempo y video de pantalla](timeline.md).

Un archivo `.ezsession` es un zip de JSON simple, un archivo HAR, PNG y MP4, así que otras herramientas también pueden leerlo. El formato está en la
versión 2; un archivo de versión 1 de EmuLabZip 0.9 o anterior todavía se abre.

## Grabar

En la app, abre **Sessions**, elige la app en la barra de herramientas y pulsa **Record**. Opcionalmente marca **Network** (el dispositivo
tiene que estar conectado antes en la vista [Network](network.md)), **Video** y **Touches**, y elige cada cuánto tomar una
captura de pantalla. Pulsa
**Stop and save**; la sesión se abre de inmediato. Las sesiones grabadas aquí aparecen en la lista de la izquierda.

En la línea de comandos:

```bash
emulabzip session record -o run.ezsession --app com.example.app --screens 5 --network
# ... use the app, then Ctrl+C. Or add --duration 60 to stop by itself.
emulabzip session record -o run.ezsession --app com.example.app --video --taps   # the screen, and touches (Android)
```

Con `--network`, el dispositivo apunta al proxy durante la ejecución y se restaura después, incluso si algo falla.

Si una grabación se interrumpe (un fallo, un portátil cerrado), lo que se escribió sigue allí en una carpeta llamada
`run.ezsession.partial`. Ábrela como un archivo; su informe dice "cut short".

## Leer

```bash
emulabzip session show run.ezsession            # for people
emulabzip session show run.ezsession --json     # for scripts
emulabzip session show run.ezsession --markdown # for a pull request comment
```

El informe tiene la duración, los errores del log y los más repetidos (los números se ignoran, así que "retry 3 of 5" y "retry 4
of 5" cuentan como uno), fallos, peticiones con las más lentas, y picos de CPU y memoria.

En la app, la sesión abierta tiene pestañas: **Overview**, **Logs** (filtra por nivel y texto), **Requests** (cabeceras y cuerpo
de cada una), **Timeline** (toques, peticiones, problemas del log, frames lentos, recolecciones de basura y fallos en un solo eje, con el video
que lo acompaña, y debajo los gráficos de CPU y memoria) y **Screens**. **Replay network** responde las peticiones de una app desde
la grabación: consulta [Repetir una grabación](replay.md).

Una sesión es también la forma más fácil de informar de un error más tarde: **Bug report** en la sesión abierta, o `emulabzip report
--session run.ezsession`, la empaqueta en un solo archivo HTML. Consulta [Informes de error](bug-reports.md).

```bash
emulabzip session timeline run.ezsession        # everything on one axis
emulabzip session video run.ezsession -o clips/ # the pieces of the screen recording
```

Un asistente puede leer sesiones con las herramientas `list_sessions`, `session_summary` y `session_timeline` del
[servidor MCP](mcp.md).

## Conviene saber

- Las sesiones pueden contener lo que tu app registra y envía: tokens, direcciones, datos personales. Una grabación de pantalla puede mostrar
  cualquier cosa que hubiera en la pantalla. Trátalas como logs, y revísalas antes de adjuntar una a un issue público.
- Una sesión grabada por una versión más reciente de EmuLabZip (un formato más nuevo) se rechaza con un mensaje que pide actualizar.
- Los archivos de sesión solo se escriben en lugares que eliges o en la carpeta `sessions` propia de EmuLabZip.

Consulta [el modo CI](ci.md) para grabar alrededor de una ejecución de pruebas.
