---
source: dfe16053c631
description: El visor de logs sigue logcat en Android y el log unificado en iOS en una sola tabla, con filtros por nivel, tag y búsqueda, y exportación.
---
# Logs

El visor de logs sigue **logcat** en Android y el **log unificado** en iOS en una sola tabla con las mismas columnas:
hora, id de proceso, nivel, tag y mensaje.

## Filtros

| Control | Qué hace |
|---------|--------------|
| **V D I W E F** | Nivel mínimo. Al hacer clic en **W** se muestran advertencias, errores y líneas fatales. |
| **Tag** | Líneas cuyo tag contiene el texto (sin distinguir mayúsculas). |
| **Search** | Líneas cuyo mensaje o tag contiene el texto. Pulsa <kbd>⌘F</kbd> para saltar aquí. |
| **.\*** | Trata la búsqueda como una expresión regular, por ejemplo `skipped \d+ frames`. Una expresión no válida pone el campo en rojo. |
| **App** | Solo las líneas escritas por la app elegida en la barra de herramientas. |

En iOS, los mensajes `Default` se muestran como **I** y los mensajes `Fault` como **F**. En Android, `A` (assert) se
muestra como **F**.

## Lectura

- La lista sigue las líneas nuevas mientras estás al final. Desplázate hacia arriba para dejar de seguir; **Jump to
  latest** te devuelve al final.
- **Pause** congela la vista sin detener el flujo. Las líneas que llegan mientras tanto aparecen cuando continúas.
- Haz clic en una línea para ver el mensaje completo, su id de hilo y (en iOS) su subsistema. Haz doble clic en una línea
  para copiarla.

## Exportación

El botón de descarga exporta **las líneas que ves en ese momento**, después de filtrar, como texto plano con el formato
`threadtime` de logcat o como líneas JSON (un objeto por línea). La CLI puede hacer lo mismo con `emulabzip logs --json`.

## Límites

El visor conserva las 50,000 líneas más recientes de forma predeterminada; cámbialo en Settings. Cuando las líneas llegan
más rápido de lo que la interfaz puede mostrarlas, la barra de estado cuenta las líneas descartadas en lugar de ocultarlo.
