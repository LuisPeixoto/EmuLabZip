---
source: bde0d784ece0
description: Empaqueta captura, dispositivo, log, peticiones fallidas y fallos legibles en un solo archivo HTML y abre un issue con el resumen ya rellenado.
---
# Informes de error

Un informe de error necesita siempre lo mismo: cómo se veía la pantalla, qué dispositivo y app, qué decía el log,
qué peticiones fallaron y cómo se cayó la app, de forma legible. EmuLabZip lo empaqueta en **un solo archivo HTML** y, si quieres,
abre un issue nuevo con un resumen ya rellenado.

<p align="center">
  <img src="images/bug-report-dialog.png" alt="El diálogo Report a bug" />
</p>

No se envía nada a ninguna parte. Tú ves el archivo y decides adónde va.

## Inicio rápido

En la app de escritorio, pulsa **Report a bug** en la barra de herramientas. Escribe qué pasó, elige qué incluir y pulsa
**Save HTML…**. Si indicas un tracker, el issue rellenado se abre en tu navegador y el archivo se muestra en su carpeta,
listo para adjuntarlo.

Desde un terminal:

```bash
emulabzip report --app com.example.shop -o report.html
emulabzip report --app com.example.shop --issue github:owner/repo --open
emulabzip report --session run.ezsession -o report.html       # from a recording
```

## Qué hay en el archivo

<p align="center">
  <img src="images/bug-report-html.png" alt="El informe abierto en un navegador: captura de pantalla, dispositivo y el fallo legible" />
</p>

| Parte | Qué contiene |
|------|----------------|
| Captura de pantalla | La pantalla tal como está ahora (o la última de una sesión), dentro del archivo |
| Grabación de pantalla | De una sesión que tiene video: el fragmento que contiene el fallo, o el último. Hasta 20 MB forma parte del archivo; un clip más grande se escribe junto a él como `emulabzip-report.mp4`, al que el archivo hace referencia |
| Tira de línea de tiempo | De una sesión: el último minuto antes del fallo (o el final de la ejecución), una fila por tipo de cosa: toques, peticiones, problemas del log, frames lentos, recolecciones de basura, el fallo. Dibujada a partir de números, sin script |
| Dispositivo | El nombre, SO, modelo, ABI, memoria, pantalla, idioma y zona horaria |
| Fallos | Los de los últimos 30 minutos, leídos con los mappings, source maps y dSYMs guardados para la app: consulta [Stacks legibles](symbolication.md) |
| Peticiones fallidas | Peticiones que no recibieron respuesta o con un estado de 400 o superior, con el error y el comienzo de la respuesta. Vienen del proxy de [Network](network.md), así que inícialo y conecta antes el dispositivo |
| Log | Las advertencias y errores (primero los errores cuando hay muchos), y las últimas líneas |

<p align="center">
  <img src="images/bug-report-video.png" alt="Un informe hecho a partir de una sesión: el clip alrededor del fallo y el último minuto en una tira" />
</p>

El archivo no tiene script y no carga nada de un servidor, así que se abre igual en cualquier navegador, sin conexión, años después.
Todo lo que contiene está escapado, así que el texto de un log o de una respuesta nunca puede convertirse en marcado. Las listas están limitadas, y una
captura muy grande se deja fuera con una nota, de modo que un informe siga siendo algo que una persona pueda leer.

Deja fuera una parte cuando no deba salir de tu máquina: `--without screenshot`, `logs`, `requests`, `crashes` o `video`
(repítelo para más), o las casillas del diálogo.

## Abrir un issue

`--issue` acepta un tracker:

| Tracker | Escríbelo así |
|---------|-------------|
| GitHub | `github:owner/repo` |
| GitLab | `gitlab:group/project`, o `gitlab:git.example.com/group/project` para un servidor propio |
| Jira | `jira:yoursite.atlassian.net/PROJECTID/ISSUETYPEID`, siendo ambos ids números |

El enlace abre la página de nuevo issue del tracker con el título y un resumen rellenados: la descripción, la app y el
dispositivo, el fallo y sus primeros frames, las peticiones fallidas y los primeros errores del log. El resumen se corta para ajustarse a
la longitud que aceptan los trackers, y termina con una línea que indica que el informe completo va adjunto. Tú adjuntas el archivo HTML.
No se pide ni se guarda ningún token, porque EmuLabZip nunca habla con el tracker: lo hace tu navegador, con tu propio inicio de sesión.

## Desde una grabación

Una [sesión](sessions.md) ya contiene los logs, peticiones, fallos y capturas de una ejecución, así que se puede hacer un informe
a partir de una más tarde, o de una ejecución de CI: ábrela en **Sessions** y pulsa **Bug report**, o

```bash
emulabzip report --session run.ezsession -o report.html
```

## Para asistentes

`create_bug_report` escribe el archivo en `reports/` en la carpeta de datos de EmuLabZip y devuelve su ruta, un resumen en Markdown
y, con `issue`, el enlace. Un informe con un clip demasiado grande para el archivo recibe una carpeta propia, con el clip dentro.
Consulta [MCP](mcp.md).

## Qué revisar antes de compartirlo

Un informe contiene lo que había en la pantalla y en el log de tu app, y las direcciones de las peticiones que fallaron. Un clip
de la pantalla puede mostrar cualquier cosa que hubiera en ella. Léelo y míralo antes de que vaya a un issue que otros puedan ver, y
usa `--without` para lo que deba quedar fuera. Consulta
[Seguridad](../SECURITY.md).
