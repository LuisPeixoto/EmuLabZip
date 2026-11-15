---
source: 75e61f983ac4
description: "Las reglas del proxy cambian lo que recibe una app sin tocar el servidor: simulan respuestas, ralentizan, bloquean o detienen una petición para editarla."
---
# Mocks y breakpoints

Las reglas te permiten cambiar lo que recibe una app sin tocar el servidor: responder tú a una petición, hacerla lenta,
hacerla fallar o detenerla y editarla a mano. Viven en el proxy, así que funcionan en todos los dispositivos a los que
está conectada la herramienta [Network](network.md).

## Reglas

Una regla tiene un **matcher** (qué peticiones) y una **acción** (qué hacer). Gana la primera regla activada que coincide.

| Campo del matcher | Coincide con |
|-------------------|--------------|
| `methods` | Cualquiera de estos métodos. Vacío significa cualquiera. |
| `host` | Un patrón: `api.example.com`, `*.example.com`. `*` es cualquier cadena de caracteres, `?` cualquier carácter. |
| `path` | Un patrón para la ruta sin la query: `/v1/users/*`. |
| `urlContains` | Texto que tiene que contener la dirección completa (host, ruta y query). |

Todos los campos definidos tienen que coincidir.

| Acción | Qué ocurre |
|--------|------------|
| **Answer** (`mock`) | El proxy responde con tu estado, headers y cuerpo, tras un retraso opcional. Nunca se consulta al servidor. |
| **Delay** | La petición espera y luego continúa. Úsalo para probar una red lenta. |
| **Block** | La app recibe un error de gateway, como si la red hubiera fallado. Nada llega al servidor. |
| **Hold** (`breakpoint`) | La petición, la respuesta, o ambas, se detienen y te esperan. |

Cuatro acciones más envían una petición a un archivo o a otro servidor, comprueban la respuesta contra un archivo OpenAPI o
ejecutan un script de WebAssembly, y una regla puede dejar que se apliquen también las que están debajo: consulta
[Reglas de red](network-rules.md) y [Scripts de reglas](rule-scripts.md).

Los flujos que tocó una regla muestran una insignia **MOCK** o **RULE** en la lista y en los detalles.

Una regla es para una cosa que tú elegiste. Para responder *todo* desde una grabación, consulta [Repetir una grabación](replay.md). Una regla
sigue ganando a la grabación.

## En la app

Abre **Network** y luego **Rules**. Elige un preset (una respuesta JSON, un 500, una red lenta, un bloqueo, un breakpoint) o
parte de una petición: selecciónala y pulsa **Mock** para obtener una regla que responde como lo hizo el servidor, lista
para editar. Las reglas se aplican a la siguiente petición y se guardan en `net-rules.json` en la carpeta de datos de EmuLabZip.

### Breakpoints

Una petición o respuesta retenida aparece en un cajón en la parte inferior de la vista Network. Puedes editar los headers
(un `name: value` por línea), el cuerpo (cuando es texto) y, en una respuesta, el estado. **Continue** la envía con tus
ediciones, **Abort** la hace fallar, **Continue all** deja pasar todo. Una respuesta se muestra decodificada y sigue sin
comprimir. Si nadie responde, un mensaje retenido continúa sin cambios tras 2 minutos, así que una app nunca se queda
colgada para siempre.

## En la línea de comandos

```bash
emulabzip net --rules rules.json
```

```json
[
  {
    "name": "stub the profile",
    "matcher": { "host": "api.example.com", "path": "/v1/profile" },
    "action": { "type": "mock", "status": 200,
                "headers": [["content-type", "application/json"]],
                "body": "{\"name\": \"Ada\"}" }
  },
  { "name": "slow uploads", "matcher": { "methods": ["POST"] }, "action": { "type": "delay", "ms": 3000 } },
  { "name": "no analytics", "matcher": { "host": "*.analytics.example.com" }, "action": { "type": "block" } }
]
```

Un mock puede llevar un cuerpo binario: define `"base64": true` y da el cuerpo en base64. Los breakpoints necesitan que
alguien los continúe, así que la línea de comandos omite esas reglas y lo indica.

## Límites

- El cuerpo de un mock puede ser de hasta 5 MB, y un retraso de hasta 60 segundos.
- Las reglas ven la petición descifrada, así que HTTPS necesita el [certificado](network.md#the-certificate) instalado. Los hosts
  que pasan sin cambios (apps con certificate pinning) no se pueden modificar.
- Los upgrades de WebSocket se comparan como cualquier petición, pero solo se puede simular o bloquear el handshake.
