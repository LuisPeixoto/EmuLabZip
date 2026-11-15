---
source: 57ea062b424d
description: Network nombra cada petición GraphQL por lo que pide y muestra su query y sus variables, para que una lista de POST /graphql sea legible.
---
# GraphQL

![Una petición GraphQL en Network, mostrada como su query y sus variables](images/network-graphql.png)

En **Network**, una petición GraphQL recibe el nombre de lo que pide, así que una lista de `POST /graphql` es legible.

- La **Operation** se muestra antes de la ruta: el `operationName` que envió el cliente o, si no, el nombre escrito después de
  `query`, `mutation` o `subscription`. Una operación sin nombre se muestra como `anonymous query` (o `mutation`).
  Un batch (una lista de peticiones en un solo cuerpo) muestra los primeros nombres y cuántos más hay.
- Las peticiones GraphQL se reconocen por su cuerpo: un `POST` JSON con una `query`, un cuerpo `application/graphql` o un `GET`
  con `?query=`.
- El filtro también busca en los nombres de las operaciones.
- Pestaña **Request**: la query tal como se escribió, luego sus variables como JSON y una insignia **GraphQL**. En un batch, cada
  operación tiene su propio encabezado.
- En la línea de comandos, `emulabzip net` imprime la operación después de cada petición.

La respuesta de una llamada GraphQL es JSON como cualquier otra. Un servidor GraphQL suele responder `200` incluso cuando la operación
falló, con el fallo en una lista `errors`: abre la pestaña **Response** para verlo.
