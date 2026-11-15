---
source: 8715ca46d87a
description: "Conecta un asistente de IA a tus emuladores y simuladores con emulabzip mcp: herramientas, prompts, recursos y los límites de seguridad."
---
# Servidor MCP

`emulabzip mcp` permite que un asistente de IA mire tus emuladores y simuladores, y trabaje con ellos: leer logs, fallos y
peticiones, manejar la pantalla, grabar una sesión, comprobar un build. Habla el
[Model Context Protocol](https://modelcontextprotocol.io) por entrada y salida estándar, y usa el mismo código que la app
y la línea de comandos, así que un dispositivo se comporta igual en todas partes.

Ofrece **118 herramientas** (130 con los cambios activados), **8 prompts** y **13 recursos**. La lista completa, escrita a partir del
propio servidor para que no se desfase, es la [referencia de MCP](mcp-tools.md).

## Conecta un asistente

La app de escritorio muestra el texto exacto para tu equipo en **Settings, AI assistants**, con un botón Copy. A mano:

**Claude Code**

```bash
claude mcp add emulabzip -- emulabzip mcp
```

**Claude Desktop**, en `claude_desktop_config.json` (Settings, Developer, Edit Config):

```json
{
  "mcpServers": {
    "emulabzip": { "command": "emulabzip", "args": ["mcp"] }
  }
}
```

**Cursor**, en `~/.cursor/mcp.json` (o `.cursor/mcp.json` en un proyecto): el mismo bloque `mcpServers` que en Claude Desktop.

**VS Code**, en `.vscode/mcp.json`:

```json
{
  "servers": {
    "emulabzip": { "type": "stdio", "command": "emulabzip", "args": ["mcp"] }
  }
}
```

Cualquier otro cliente que inicie un servidor stdio funciona igual. Da a `command` la ruta completa de `emulabzip` si no está en
el PATH del cliente.

## Qué puede hacer un asistente

Cada herramienta acepta un `device` opcional (un id o un nombre). Cuando solo hay un dispositivo en marcha se puede omitir.

| Pide | Usa |
|------|-----|
| "¿Qué hay en la pantalla? ¿Es accesible?" | `screenshot`, `get_layout`, `audit_accessibility`, `check_text`, `simulate_color_blindness` |
| "Abre la app e inicia sesión" | `launch_app`, `tap`, `swipe`, `input_text`, `press_key`, `open_url`, `play_flow`, `flow_create` |
| "¿Por qué falló?" | `list_crashes`, `get_crash`, `symbolicate_stack`, `read_logs`, `watch_start` |
| "¿Por qué va lenta?" | `perf_snapshot`, `record_trace`, `trace_summary`, `measure_startup`, `memory_diff`, `analyze_heap` |
| "¿Qué envía al servidor?" | `net_start`, `net_connect_device`, `net_flows`, `net_flow`, `net_export_har`, `openapi_check` |
| "Haz que falle el backend" | `net_rules_set` (mock, delay, block, map), `net_conditions`, `net_replay`, `net_breakpoints` |
| "Sigue los logs mientras uso la app" | `watch_start` con `logs`, `perf`, `touches`, `webview_console`, `webview_network`, `react_console` o `react_actions`, y luego `watch_read` |
| "Graba esto y compáralo con el de la semana pasada" | `session_start`, `session_stop`, `session_summary`, `session_timeline`, `session_compare` |
| "¿Qué hay en la base de datos?" | `storage_roots`, `read_database` (SQL de solo lectura), `read_preferences`, `list_files`, `read_file` |
| "¿Está listo este build?" | `audit_security`, `analyze_package`, `package_budget`, `i18n_sweep`, `visual_compare`, `check_link` |
| "Escribe un informe de esto" | `create_bug_report` (omite el video, las peticiones o los logs con `without`) |

### Watches en vivo

Los flujos de datos (logs, rendimiento, toques, WebView y React Native) son *watches*: `watch_start` devuelve un id, `watch_read` devuelve
lo que llegó desde la última lectura (y lo que se perdió si el asistente fue lento), y `watch_stop` lo termina. Un watch guarda los
últimos 5,000 elementos, termina solo tras diez minutos sin una lectura, y como máximo ocho se ejecutan a la vez.

### Estado de red, de sesión y del dispositivo que se mantiene

El servidor mantiene estado mientras se ejecuta: el proxy, las peticiones capturadas, las reglas, una grabación y los watches. Todo lo que
cambió en un dispositivo se restablece cuando el asistente se desconecta: se borra el ajuste de proxy del dispositivo y se termina
una grabación abierta. Si una sesión acaba mal, `net_status` muestra lo que sigue activo y `net_disconnect_device` lo restablece.

Las reglas de red son las que usa la app de escritorio (un archivo compartido), así que una regla que defina un asistente aparece en la app.

## Prompts

Los prompts son planes desde los que un asistente puede empezar. Elige uno en el cliente (en Claude Code, escribe `/` y el nombre del prompt).

| Prompt | Para |
|--------|------|
| `explain_crash` | Encontrar el último fallo, leerlo con su mapeo, decir qué falló y cómo arreglarlo |
| `why_is_it_slow` | Medir, trazar la parte lenta y nombrar la causa |
| `summarize_session` | Contar la historia de una sesión grabada |
| `reproduce_bug` | Seguir una descripción en el dispositivo, grabarla y escribir el informe |
| `debug_request` | Capturar el tráfico y decir por qué falló una petición |
| `release_check` | Comprobaciones de seguridad, accesibilidad, tamaño, texto e idioma en una sola pasada |
| `check_deep_link` | Verificar un enlace y ver adónde llega |
| `mock_backend` | Responder a un host con una respuesta fija, un retraso o un error |

## Recursos

Un asistente también puede *leer* estos sin llamar a una herramienta: `emulabzip://devices`, `sessions`, `flows`, `states`,
`links/history`, `rules`, `network/flows` y `guide` (cómo usar el servidor, por tarea), además de `emulabzip://sessions/{id}`,
`emulabzip://flows/{id}` (y `/maestro`, `/script`) y `emulabzip://network/flows/{id}`. Una dirección nunca puede nombrar una
ruta: los ids se comprueban como los comprueban las herramientas.

## Qué puede y qué no puede hacer un asistente

Las herramientas que miran están siempre activas. Las herramientas que **actúan** sobre un dispositivo (tocar, grabar, conectar el proxy…) lo cambian de verdad:
para eso está el servidor. Están limitadas por las mismas comprobaciones que la app (consulta [Controls](controls.md#safety)): un enlace
a `file:` se rechaza, los valores se validan y nada llega a un shell sin entrecomillar.

**Destruir o reescribir datos está desactivado por defecto.** Borrar o eliminar un dispositivo, restaurar un estado, cargar o eliminar un
snapshot del emulador, escribir un archivo en el almacenamiento de una app, ejecutar un script en una página web y eliminar sesiones, flujos, mapeos
y líneas base no se ofrecen en absoluto. Inicia el servidor con `--allow-writes` y aparecen (marcadas como destructivas), y
**cada llamada te pregunta primero**: el cliente muestra lo que va a pasar ("Wipe Pixel_Test? All its apps and data are deleted.") y
nada se ejecuta hasta que aceptes. Restaurar un estado guarda antes los datos tal como están ahora.

```bash
claude mcp add emulabzip -- emulabzip mcp --allow-writes
```

- Un cliente que no puede preguntarte (sin *elicitation* de MCP) es rechazado, con el motivo. Para una ejecución desatendida en un
  dispositivo desechable, `--allow-writes --no-confirm` omite la pregunta; no lo uses en ningún sitio donde tengas datos que te importen.
- **Aprobar scripts y plugins, y ejecutar `ci`, nunca se ofrecen**, con o sin `--allow-writes`: ejecutan código,
  así que quedan en tus manos. `run_plugin` rechaza un plugin que no has activado, y una regla de red que ejecuta un script se
  acepta solo para un script que aprobaste en la app.
- Los archivos se escriben solo en las carpetas propias de EmuLabZip (`exports`, `reports`, `sessions`, `states`, `flows`, `mappings`); un asistente
  nunca da una ruta donde escribir.
- No puede descargar nada. `trace_summary` y `trace_query` necesitan el procesador de trazas de Perfetto, que solo se
  descarga después de que aceptes en la app o ejecutes `emulabzip trace summary` una vez.
- Solo lee lo que expone el dispositivo: los archivos de las apps que ya puedes explorar, y solo cuando son depurables en
  Android. Las herramientas de Storage son de solo lectura (el SQL que escribe se rechaza).
- Solo habla por entrada y salida estándar. No hay un puerto de red para el propio protocolo.

Trata a un asistente como a un colega con tu terminal: no lo apuntes a un dispositivo que tenga datos que no puedes permitirte
cambiar.

## Probarlo

```bash
# Ask the server what it offers, the way an assistant does
printf '%s\n' \
  '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"try","version":"0"}}}' \
  '{"jsonrpc":"2.0","method":"notifications/initialized"}' \
  '{"jsonrpc":"2.0","id":2,"method":"tools/list"}' | emulabzip mcp
```

Los logs van a la salida de error estándar (`EMULABZIP_LOG=debug`); la salida estándar lleva solo el protocolo.
