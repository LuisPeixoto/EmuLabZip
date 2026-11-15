---
source: 7a600d6f8eb4
description: "Crea tu propio colector como plugin: una carpeta con plugin.json y un programa que imprime JSON, y EmuLabZip lo dibuja como gráficos y tablas."
---
# Plugins

Un plugin es tu propio colector. Es una carpeta con un `plugin.json` y un programa. EmuLabZip inicia el programa para un
dispositivo, el programa imprime un objeto JSON por línea en su salida estándar, y EmuLabZip los dibuja: métricas como gráficos
o tarjetas, tablas, texto y líneas de log.

No hay código de plugin dentro de la app. Un plugin no puede cambiar la interfaz ni romperla, y solo puede hacer lo que podría hacer un
programa que hubieras iniciado tú mismo.

<p align="center">
  <img src="images/plugins.png" alt="Un plugin en streaming que grafica la temperatura de la batería" />
</p>

## Instalar, activar, ejecutar

```bash
emulabzip plugin new my-plugin                       # a template in the plugins folder
emulabzip plugin install examples/plugins/top-processes   # copy a plugin folder in
emulabzip plugin list
emulabzip plugin enable my-plugin
emulabzip plugin run my-plugin --app com.example.app
```

En la app, abre **Plugins**. Los plugins viven en la carpeta `plugins` dentro de la carpeta de datos de EmuLabZip (**Open folder**).

**Un plugin es un programa que se ejecuta como tú.** Por eso nunca se ejecuta hasta que lo activas, y al activarlo se muestra el comando exacto
y desde dónde se ejecuta. La aprobación está ligada al contenido de `plugin.json`: si cambias el archivo, el plugin se
desactiva de nuevo. La app y el servidor MCP solo ejecutan plugins activados. En la línea de comandos, `emulabzip plugin run <name>`
es tu propia petición de ejecutarlo. Lee un plugin antes de activarlo.

## `plugin.json`

```json
{
  "protocol": 1,
  "name": "battery-temperature",
  "version": "1.0.0",
  "description": "Plots the battery temperature of an Android device.",
  "platforms": ["android"],
  "command": ["./run.sh"],
  "mode": "stream",
  "timeoutS": 60
}
```

| Campo | Significado |
|-------|---------|
| `protocol` | Debe ser `1`. |
| `name` | Letras, dígitos, `-` y `_`. Debe ser el nombre de la carpeta. |
| `version`, `description` | Se muestran en la lista. |
| `platforms` | `android`, `ios`, o ambos. Vacío significa ambos. |
| `command` | El programa y sus argumentos, nunca ejecutados a través de un shell. El programa se busca primero en la carpeta del plugin y luego en el PATH. |
| `mode` | `once`: el programa imprime sus resultados y termina. `stream`: sigue ejecutándose e imprime sobre la marcha, hasta que se detiene. |
| `timeoutS` | Para `once`: segundos antes de que se detenga el programa (60 por defecto, máximo 600). |

## Qué recibe el programa

El directorio de trabajo es la carpeta del plugin, y el entorno tiene:

| Variable | Valor |
|----------|-------|
| `EMULABZIP_DEVICE` | El serial de adb o el UDID del simulador |
| `EMULABZIP_PLATFORM` | `android` o `ios` |
| `EMULABZIP_APP` | La app elegida (nombre de paquete o bundle id), vacío cuando no hay ninguna |
| `EMULABZIP_ADB` | La ruta de `adb` (la copia del SDK, o `adb`) |
| `EMULABZIP_PLUGIN_DIR` | La carpeta del plugin |
| `EMULABZIP_PROTOCOL` | `1` |

## Qué imprime el programa

Un objeto JSON por línea, con un `type`:

```json
{"type":"metric","name":"battery temperature","value":25.0,"unit":"°C"}
{"type":"table","title":"Top processes","columns":["PID","MB","Name"],"rows":[[552,217.9,"system_server"]]}
{"type":"text","title":"Picked app","body":"com.example uses 120 MB"}
{"type":"log","level":"warn","message":"adb was slow"}
{"type":"error","message":"The device did not answer"}
```

- `metric`: un número. En modo `stream` cada valor es un punto de un gráfico (se conservan los últimos 120); en modo `once` es una
  tarjeta.
- `table`: las celdas son texto, números, booleanos o null. Una fila puede ser más corta que las columnas, nunca más larga, y las celdas
  no pueden contener listas ni objetos.
- `text`, `log`: se muestran tal cual.
- `error`: se muestra como un error. A un programa que termina con un código de error y no dice nada se le genera uno a partir de su código de salida
  y del final de su salida de error.

Las líneas que no son JSON, y los eventos de un tipo que esta versión no conoce, se omiten, de modo que un plugin puede imprimir su progreso
para las personas y usar eventos más nuevos sin riesgo. Haz flush después de cada línea cuando el plugin haga streaming.

## Límites

Una línea puede tener hasta 256 KB (una más larga se omite), y una ejecución puede imprimir hasta 8 MB en total, tras lo cual el plugin se
detiene. Un plugin `once` que supera su timeout se detiene. Detener una ejecución, cerrar la app o desactivar el plugin
termina el programa.

## Ejemplos

`examples/plugins` tiene dos: `battery-temperature` (un script `sh` que emite una métrica en streaming) y `top-processes` (un script de Python
que imprime una tabla). `emulabzip plugin new` escribe una plantilla de Python que imprime una métrica y algo de texto.
