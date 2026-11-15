---
source: 8c8e10338584
description: Network muestra las llamadas gRPC por el método al que llaman, con el estado del servidor, y lee los mensajes protobuf como JSON, con o sin archivos .proto.
---
# gRPC y protobuf

![Una llamada gRPC en Network, con su estado y el mensaje protobuf leído como JSON](images/network-grpc.png)

Las llamadas gRPC se muestran en **Network** por el método al que llaman, con el estado con que respondió el servidor.

- La columna **Operation** muestra `Service/Method`, tomado de la ruta.
- gRPC indica cómo terminó una llamada en los **trailers**, headers enviados después del cuerpo (`grpc-status`, `grpc-message`). Se
  registran y se muestran en la pestaña **Headers**, y el estado aparece en la lista como `OK`, `UNAVAILABLE`, `NOT_FOUND`…
- Las llamadas pasan por el proxy en streaming a medida que ocurren, así que las llamadas de streaming funcionan, y una llamada larga muestra sus mensajes a
  medida que llegan. Un breakpoint de petición no puede retener una llamada gRPC, porque necesita el mensaje completo. Las reglas aún pueden retrasarla,
  bloquearla o responderla.
- Solo se descifra gRPC sobre TLS (HTTP/2 negociado por ALPN), como todo HTTPS. HTTP/2 en texto claro (`h2c`) no está
  soportado.

## Leer los mensajes

Un cuerpo gRPC es una serie de mensajes, cada uno en protobuf. Las pestañas **Request** y **Response** los listan como JSON, de una de
dos maneras:

- **Sin un esquema.** Protobuf en el cable tiene números de campo, no nombres. EmuLabZip los lee igualmente: `{"1": "hello",
  "2": 150, "3": {"1": 5}}`. El texto se muestra como texto, un mensaje anidado como un objeto, un campo repetido como una lista y
  los demás bytes como hexadecimal. Un mensaje que no es protobuf lo indica.
- **Con tus archivos `.proto`.** En **Network → Network → gRPC and protobuf**, elige la carpeta que los contiene (aquella a la que son relativas
  las rutas de `import`). Se compilan en memoria, sin necesitar `protoc`. Entonces el método de la llamada indica
  de qué tipos de mensaje son su petición y su respuesta, y los mensajes se muestran como el JSON de protobuf, con los nombres de campo
  reales. Un mensaje que no encaja con su tipo vuelve a la lectura sin esquema.

Los mensajes comprimidos con `grpc-encoding: gzip` se descomprimen.
