---
source: ad59e3a2fe37
description: Cómo EmuLabZip lee el tráfico WebSocket como mensajes en la pestaña Messages de Network, qué protocolo entiende y qué límites tiene cada flujo.
---
# Mensajes WebSocket

La herramienta [Network](network.md) lee el tráfico WebSocket como mensajes, no como bytes. Abre una petición WebSocket y la pestaña
**Messages** lista lo que fue en cada sentido, con el tiempo desde que se abrió la conexión.

- Las **flechas** muestran la dirección: hacia arriba va de la app al servidor, hacia abajo vuelve.
- Los mensajes de texto se muestran como texto, y el JSON se formatea al abrir uno. Los mensajes binarios muestran un volcado hexadecimal.
- El filtro busca dentro de los mensajes de texto.
- Los frames de control (ping, pong, close) también aparecen en la lista.

## Cómo se lee

El proxy sigue copiando cada byte entre la app y el servidor sin cambios y solo escucha. Sigue el RFC 6455:
frames con longitudes de 7, 16 y 64 bits, máscaras, mensajes fragmentados unidos de nuevo en uno solo, frames de control entre
ellos, y `permessage-deflate` cuando ambos lados lo acordaron (incluido `no_context_takeover`). Si lo que fluye no es
WebSocket después de todo, la lectura se detiene sin ruido y la conexión continúa.

## Límites

Un flujo guarda hasta 1.000 mensajes y 64 KB de cada uno; la pestaña indica cuántos mensajes posteriores no se guardaron y cuándo un
mensaje se cortó. Los contadores siguen subiendo. La [exportación HAR](network.md#export) incluye los mensajes en
`_webSocketMessages`, como hace Chrome.
