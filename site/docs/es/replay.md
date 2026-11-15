---
source: 77184675abce
description: "Responde las peticiones de la app desde una grabación para ejecutarla sin internet: cómo se emparejan, qué no cubre, uso en CI y en la app."
---
# Repetir una grabación

<p align="center">
  <img src="images/replay.png" alt="Network con una grabación repetida: tres peticiones respondidas, una que no está en la grabación" />
</p>

Un **replay** responde las peticiones de la app desde una grabación, de modo que los servidores nunca reciben consulta. La app se ejecuta sin
internet, contra lo que un servidor dijo una vez. Sirve para errores cuyo backend ha cambiado desde entonces, y para ejecuciones en CI que deben
dar el mismo resultado siempre.

```bash
# Record once, with the network.
emulabzip session record -o run.ezsession --app com.example.app --network

# Run the app against it. Press Ctrl+C to stop.
emulabzip net --replay run.ezsession
```

Una [sesión](sessions.md) con su red, o cualquier archivo HAR (una exportación de un navegador, o `emulabzip net --har`), es una grabación.

## Qué hace

El proxy consulta la grabación antes de preguntar a un servidor. Una petición que está en ella recibe la respuesta grabada: el estado,
las cabeceras y el cuerpo, tal como eran. Una petición que **no** está en ella falla con un `504` que dice *not in recording*,
y aparece en la lista de peticiones marcada como fallida. Nunca se envía al servidor. Ese es el objetivo: una prueba que depende
de una dirección de la que nadie te habló falla, en lugar de usar la red sin avisar.

- Las peticiones se emparejan por método, host, ruta y query. El orden de los parámetros de la query
  no importa; el puerto tampoco.
- Una petición grabada **más de una vez** recibe las respuestas en el orden en que se grabaron, y después la última otra vez. Un contador que decía 1, luego 2, dice 1, luego 2, luego 2.
- Cuando las peticiones a la misma dirección difieren en su cuerpo (una consulta GraphQL, por ejemplo), se prefiere la que tiene el mismo cuerpo.
- HTTPS funciona, porque el proxy termina el TLS él mismo (el dispositivo tiene que confiar en el [certificado](network.md#the-certificate)
  como en cualquier captura).
- Una regla de [Mocks](mocks.md) sigue ganando a la grabación, porque la escribiste a propósito.

## Qué no hace

Mientras se repite una grabación, lo que no puede responderse desde un archivo HAR se rechaza en lugar de dejarlo pasar a
internet:

- **WebSockets**. Sus mensajes están en el archivo, pero no se repiten.
- **Llamadas gRPC**. Un archivo HAR no tiene trailers, así que la llamada no podría terminar.
- **Conexiones que el proxy no puede decodificar**: otros protocolos, y apps que no aceptan el certificado de EmuLabZip.
- **Reenviar una petición** desde la vista Network.

Las peticiones que fallaron al grabarse no están en la grabación.

## Dejar pasar algunas

```bash
emulabzip net --replay run.ezsession --passthrough
```

Con `--passthrough`, una petición que no está en la grabación va al servidor real. Úsalo cuando solo quieras congelar
una parte de lo que hace la app. `--timing` hace que cada respuesta espere tanto como tardó el servidor, para una app que se comporta
de forma distinta cuando la red es lenta.

## En CI

```bash
emulabzip ci -o run.ezsession --network --replay baseline.ezsession --fail-on-miss -- ./gradlew connectedCheck
```

La ejecución se graba a sí misma como de costumbre, con sus peticiones respondidas desde `baseline.ezsession`. `--fail-on-miss` la hace fallar cuando la
app pidió algo que no está en la grabación, y el final de la ejecución lista qué. `session record` también acepta
`--replay`. Ambos necesitan `--network`.

## En la app

En **Network**, **Replay → Choose a file…** carga un archivo HAR o una sesión. Una barra muestra qué se está repitiendo, cuántas
peticiones se respondieron y cuántas no (y cuáles, en **Not recorded**). En **Sessions**, **Replay network**
carga la sesión abierta. Inicia el proxy y conecta el dispositivo como de costumbre; **Stop replay** vuelve a los servidores.

## Conviene saber

- La grabación se lee antes que cualquier otra cosa, así que un archivo equivocado o una sesión sin red se explica de inmediato.
- Las respuestas son los cuerpos grabados, decodificados. `Content-Encoding`, `Content-Length` y `Alt-Svc` no se repiten, así que
  a la app no se le dice que use una compresión que el cuerpo ya no tiene, ni que pruebe HTTP/3 saltándose el proxy.
- Una grabación contiene lo que envió el servidor, incluidos tokens y datos personales. Revísala antes de compartirla.
