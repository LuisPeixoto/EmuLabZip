---
source: 45af944a7748
description: Los scripts de reglas son pequeños programas WebAssembly aislados que miran una petición o respuesta y deciden qué le pasa, y solo se ejecutan tras aprobarlos.
---
# Scripts de reglas

Un **script** es un pequeño programa de WebAssembly que mira una petición o una respuesta y decide qué le ocurre: añadir un
header, ocultar un campo, responder por sí mismo, hacer fallar una de cada tres peticiones. Una regla con la acción **Script** lo
ejecuta para cada petición con la que coincide la regla.

Un script está aislado en un sandbox. No puede leer archivos, acceder a la red, leer el reloj ni tocar nada más de tu
ordenador: la única función que puede importar es `env.log`. Se detiene tras una cantidad fija de trabajo y de memoria. Y
solo se ejecuta después de que **hayas aprobado sus bytes exactos**.

## Aprobar

Un archivo de script no hace nada hasta que se aprueba, en el diálogo de la regla (**Approve this script**, que muestra su SHA-256 y
qué mira) o en la línea de comandos:

```bash
emulabzip script show add-header.wasm       # what it does, and whether it is approved
emulabzip script approve add-header.wasm
emulabzip script list
emulabzip script revoke 09e74ff6            # a file, or the start of its hash
```

La aprobación es para el SHA-256 del archivo: cambia un byte y hay que aprobarlo de nuevo. Aprueba solo scripts en los que confíes: un
script ve las peticiones y respuestas con las que coincide su regla, con sus headers y cuerpos. Un flujo cuyo script no se ejecutó indica
el motivo en sus detalles.

![El diálogo Rules con una regla de script que no se ha aprobado](images/rules-script.png)

## La interfaz

Un script es un módulo que exporta:

| Export | Tipo | |
|--------|------|-|
| `memory` | memory | La memoria lineal del script. |
| `alloc` | `(len: i32) -> i32` | Un puntero a `len` bytes en los que el host puede escribir. |
| `on_request` | `(ptr: i32, len: i32) -> i64` | Se llama con una petición. Opcional. |
| `on_response` | `(ptr: i32, len: i32) -> i64` | Se llama con una respuesta. Opcional. |

Hace falta al menos uno de `on_request` y `on_response`. El host escribe un documento JSON en `ptr` (`len` bytes) y la
función devuelve `(out_ptr << 32) | out_len`, donde está su respuesta, también en JSON.

**Entrada**

```json
{ "method": "GET", "url": "https://api.example.com/users?id=1",
  "headers": [["accept", "application/json"]],
  "body": "<base64>", "bodyOmitted": false,
  "status": 200 }
```

`status` solo está en una respuesta. El cuerpo de una respuesta se decodifica (gzip, brotli, zstd). Un cuerpo de más de 1 MB no se
entrega: `bodyOmitted` es `true` y `body` está vacío.

**Respuesta**

```json
{ "action": "continue", "status": 201, "headers": [["x-by", "me"]], "body": "<base64>", "text": "…", "message": "…" }
```

- `"continue"` (o sin `action`) deja que la petición siga o que la respuesta pase. `headers`, `body` (base64) o `text` y, en una
  respuesta, `status`, reemplazan lo que había; lo que se omite no se cambia. Una lista de headers **reemplaza** toda la lista, así que
  parte de la de la entrada.
- `"replace"` (en una petición) responde de inmediato con ese `status` (200 por defecto), `headers` y cuerpo, y nunca
  se consulta al servidor. En una respuesta equivale a `continue`.
- `"block"` hace fallar la petición con un error de gateway, indicando `message`.

Un script que entra en trap, se queda sin trabajo, responde algo que no es válido o pide algo que no se le da **no**
rompe la petición: pasa como si el script no estuviera, y el motivo está en los detalles del flujo.

## Límites

- **Trabajo:** 10 millones de unidades de fuel por llamada, más o menos el mismo número de instrucciones.
- **Memoria:** 64 MB. Un módulo puede ser de 2 MB y una respuesta de 8 MB. Como máximo 20 líneas de `env.log`, de 1 KB cada una.
- Una instancia nueva para cada llamada, así que nada de lo que guarde un script sobrevive hasta la siguiente petición; usa la URL cuando una decisión tenga que ser
  repetible.
- Ninguna importación aparte de `env.log(ptr: i32, len: i32)`, que escribe una línea que se muestra en el flujo.

## Escribir uno

Sirve cualquier lenguaje que compile a WebAssembly. `tools/rule-scripts` tiene tres ejemplos en Rust, cada uno compilado para
`wasm32-unknown-unknown`, con un pequeño crate auxiliar que hace el empaquetado:

| Script | Qué hace |
|--------|----------|
| `add-header.wasm` | Añade `x-emulabzip-script` a cada petición y respuesta. |
| `redact-json.wasm` | Reemplaza por `***` el valor de `email`, `token`, `password` y `authorization` en las respuestas JSON. |
| `flaky.wasm` | Responde 503 para alrededor de un tercio de las direcciones, elegidas a partir de la URL para que la misma falle siempre. |

```bash
cd tools/rule-scripts && ./build.sh          # needs: rustup target add wasm32-unknown-unknown
```

Los archivos de `dist/` ya están compilados. Añade el script a una regla, apruébalo y observa el flujo.
