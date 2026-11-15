---
source: a0fef516194b
description: Inspecciona los eventos que una app envía a Segment, Amplitude, Mixpanel y Firebase Analytics, ya decodificados, en una lista propia y en tiempo real.
---
# Inspector de Analytics

<p align="center">
  <img src="images/analytics.png" alt="La vista Analytics con la lista de eventos que una app envió a Segment y Mixpanel, con uno abierto" />
</p>

Mira los eventos que una app envía a **Segment**, **Amplitude**, **Mixpanel** y **Firebase Analytics**, decodificados, a medida que
ocurren, en una lista propia.

```bash
emulabzip analytics                          # runs the proxy, points the device at it, prints each event
emulabzip analytics --service segment,mixpanel --json
emulabzip analytics --firebase-log           # Firebase's debug-mode lines from the device log
```

Las llamadas de analytics son peticiones normales, así que también siguen en la lista de Network. El inspector lee su host, dirección,
cabeceras y cuerpo, por lo que funciona con todo lo que el proxy graba, y con una sesión o un HAR abierto más tarde.

## Qué se decodifica

| Servicio | Host | Qué |
|---------|------|------|
| Segment | `*.segment.io`, `*.segment.com` | `/v1/track`, `/v1/identify`, `/v1/page`, `/v1/screen`, `/v1/group`, `/v1/alias` y `/v1/batch`: JSON, una llamada o un lote |
| Amplitude | `*.amplitude.com` | La API HTTP (`/2/httpapi`, `/batch`: JSON con `events`) y el cuerpo de formulario más antiguo (`e=`), con propiedades de evento y de usuario |
| Mixpanel | `*.mixpanel.com` | `/track` y `/import` (`data=` en base64, en el cuerpo o en la query, o JSON simple) y los cambios de perfil de `/engage` |
| Firebase Analytics | `app-measurement.com` | El protobuf por lotes de `/a`: nombres de evento, parámetros y horas, después de deshacer el `Content-Encoding` del cuerpo |

Cada evento es un servicio, un tipo (`track`, `screen`, `page`, `identify`, `profile`…), un nombre, sus propiedades, el usuario (o
el id anónimo o de dispositivo) y su hora cuando la tiene.

Firebase también puede leerse **sin el proxy**. Con el modo debug activado para la app
(`adb shell setprop debug.firebase.analytics.app <package>`), Firebase escribe cada evento en el log bajo la etiqueta `FA`, y
`emulabzip analytics --firebase-log` los imprime.

## En el escritorio

La vista **Analytics** lista los eventos de las peticiones que el proxy ha grabado, los más recientes primero, con un filtro por servicio
y una búsqueda en nombres y propiedades. Abre un evento para ver sus propiedades. Inicia primero el proxy en la vista **Network**.

## Límites

- Los SDK que **fijan su certificado** pasan sin cambios y no pueden leerse. El log de debug de Firebase funciona con ellos.
- El formato de lotes de Firebase es el del protocolo de medición de Google tal como se entiende hoy, y el decodificador se probó
  con payloads hechos con ese formato, **no con tráfico capturado de una app Firebase real**. Cuando Google lo cambie, puede que falten
  eventos: indícalo en un issue, con una captura si puedes.
- Los eventos se decodifican bajo demanda, no se almacenan: un evento de una petición que el proxy ya no conserva se pierde.
- Por [MCP](mcp.md) no hay herramienta de analytics: el servidor no ejecuta ningún proxy.
