---
source: 88df05457f3f
description: "Cuatro acciones de regla más allá de responder: servir un archivo local, redirigir a otro servidor, comprobar un contrato OpenAPI y encadenar reglas."
---
# Reglas de red: archivos, otros servidores y contratos

[Mocks y breakpoints](mocks.md) responden, retrasan, bloquean o retienen una petición. Cuatro acciones más actúan sobre la
forma en que se responde una petición, y las reglas se pueden encadenar.

| Acción | Qué ocurre |
|--------|------------|
| **File** (`mapLocal`) | Responde con un archivo de este ordenador, o con el archivo de una carpeta que nombra la ruta. No se consulta al servidor. |
| **Server** (`mapRemote`) | Envía la petición a otro servidor, como si la app lo hubiera pedido. |
| **Contract** (`validate`) | Comprueba la respuesta contra un archivo OpenAPI y muestra lo que no coincide. No cambia nada. |
| **Script** (`script`) | Ejecuta un [script](rule-scripts.md) de WebAssembly sobre la petición y la respuesta. |

En la app están en **Network → Rules**, con selectores de archivos. En la línea de comandos van en el mismo archivo de `--rules`.

## Map local

```json
{ "name": "bundled assets", "matcher": { "host": "cdn.example.com", "path": "/assets/*" },
  "action": { "type": "mapLocal", "path": "/Users/me/app/dist" } }
```

Con una carpeta, la parte de la ruta anterior al patrón de la propia regla se omite: con `/assets/*`, una petición de
`/assets/app.js` obtiene `app.js` de la carpeta, y una petición de una carpeta obtiene su `index.html`. Con un archivo, el archivo es la
respuesta a cada petición que coincide con la regla. El tipo de contenido sale de la extensión (o de `contentType`), y `status`
(200 por defecto) es el estado de la respuesta. **Nunca se sirve nada fuera de la carpeta**: se rechazan `..`, un `%2e%2e`
codificado, una unidad o un enlace simbólico que lleve fuera. Un archivo puede ser de hasta 50 MB.

El flujo muestra **MOCK** y el archivo que se sirvió.

## Map remote

```json
{ "name": "use my laptop", "matcher": { "host": "api.example.com", "path": "/api/*" },
  "action": { "type": "mapRemote", "scheme": "http", "host": "localhost", "port": 3000, "pathPrefix": "/v2/" } }
```

Lo que la regla omite se queda como lo pidió la app, así que un host y un puerto solos envían la misma ruta a un servidor
local. `pathPrefix` reemplaza la parte de la ruta anterior al primer `*` del patrón: `/api/users` se convierte en `/v2/users`. Un
nuevo esquema o host toma el puerto habitual del esquema salvo que se dé uno. Al nuevo servidor se le indica su propio host,
salvo con `"preserveHost": true`. La vista Network sigue mostrando lo que pidió la app e indica adónde se envió la petición.
Map remote no se aplica a las llamadas gRPC.

## Contract

```json
{ "name": "the API contract", "matcher": { "host": "api.example.com" },
  "action": { "type": "validate", "spec": "/Users/me/app/openapi.yaml" }, "thenContinue": true }
```

Cada respuesta se comprueba, después de enviarla, contra una descripción **OpenAPI 3.0 o 3.1** en JSON o YAML (el archivo se
lee al guardar la regla, así que un archivo inutilizable se rechaza de inmediato). La operación sale del método y de la
ruta (`/users/{id}`, con las rutas base de los servidores), el esquema sale del estado (`200`, luego `2XX`, luego `default`) y del tipo
de contenido, y se valida el cuerpo. El flujo obtiene una pestaña **Contract** y una insignia en la lista:

- la respuesta **coincide** (`ok`);
- **no coincide**: cada problema viene con un puntero JSON al cuerpo (`/items/0/name`) y un mensaje, y un estado
  o tipo de contenido que la especificación no describe también es un problema;
- la especificación **no describe** la petición (`unknown`), o **no tiene esquema** contra el que comprobar esta respuesta, o el cuerpo no es
  JSON (`unchecked`).

![Una petición con dos incumplimientos de su archivo OpenAPI, en la pestaña Contract](images/network-contract.png)

Sin conexión, `emulabzip openapi check --spec api.yaml --har run.har` comprueba las respuestas de un archivo HAR y termina con un error
cuando una no coincide, que es lo que quiere un paso de CI. Solo se comprueban las respuestas (no las peticiones), solo los cuerpos JSON y
solo los `$ref` dentro del archivo. Un archivo Swagger 2.0 se rechaza con un mensaje para convertirlo.

## Encadenar reglas

Decide la primera regla que coincide, salvo que diga `"thenContinue": true`. Una regla que **cambia** la petición (un retraso, un map
a otro servidor, una comprobación de contrato, un script) puede pasar la petición a las reglas de debajo; una regla que **responde**, bloquea o
retiene la petición (una respuesta, un archivo, un bloqueo, un breakpoint) siempre termina la búsqueda. Así que un retraso seguido de un
archivo responde despacio desde el archivo, y una comprobación de contrato seguida de un mock comprueba el mock. El flujo lista todas las reglas que se aplicaron.
