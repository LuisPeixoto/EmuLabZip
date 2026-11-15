---
source: a4b2edad11c5
description: "Audita la seguridad y privacidad de una app: secretos y datos personales en logs, preferencias y bases de datos, tráfico HTTP, manifiestos y componentes expuestos."
---
# Auditoría de seguridad y privacidad

<p align="center">
  <img src="images/audit-security.png" alt="La vista Audit con los hallazgos de una app: un bearer token en el log, un token de sesión en las preferencias, un receiver exportado" />
</p>

`emulabzip audit security <app>` y la vista **Audit** buscan lo que una app no debería dejar tirado: secretos y
datos personales en el log, en las preferencias y en las bases de datos, HTTP simple y secretos en las peticiones, indicadores como `debuggable`
y `allowBackup`, componentes a los que otras apps pueden acceder, excepciones de App Transport Security y huecos en un manifiesto de privacidad de iOS.
Cada hallazgo dice qué es, dónde y cómo corregirlo.

```bash
emulabzip audit security com.example.shop
emulabzip audit security com.example.shop --har run.har       # also look at recorded requests
emulabzip audit security com.example.shop --only manifest,storage --json
emulabzip audit security com.example.shop --fail-on error     # exit 1 when something at least this serious is found
```

**Los secretos nunca se repiten.** La evidencia se reduce a los dos primeros y los dos últimos caracteres y la longitud
(`ey…Xk (131 chars)`), en el terminal, en JSON, en el informe y por MCP.

## Qué revisa

| Área | Reglas |
|------|-------|
| **Manifest** (Android, desde `dumpsys package`) | `debuggable`, `allow-backup`, `old-target-sdk` (por debajo de API 34), `exposed-component` (un receiver, servicio o activity con un intent filter, aparte del launcher), `content-provider` (listado; `dumpsys` no dice si está exportado) |
| **Manifest** (iOS, `Info.plist`) | `ats-arbitrary-loads`, `ats-arbitrary-loads-scoped` (contenido web, multimedia), `ats-insecure-domain` (HTTP simple permitido para un dominio), `ats-old-tls` |
| **Privacidad** (iOS) | `no-privacy-manifest`, `tracking-not-declared`, `undeclared-tracking-domain`: los dominios de publicidad y analytics que se vio usar a la app, frente a `PrivacyInfo.xcprivacy` |
| **Tráfico** (peticiones grabadas) | `cleartext-traffic` (una vez por host; `localhost`, `10.0.2.2` y `*.local` no se informan), `secret-in-url` (un token o clave en la query string) y, solo sobre HTTP simple, `<kind>-in-cleartext-body` |
| **Log** | `<kind>-in-log` |
| **Almacenamiento** | `<kind>-in-storage` (SharedPreferences, DataStore, MMKV, listas de propiedades), `secret-in-storage` (una clave con nombre de secreto y un valor), `sensitive-column` (una columna SQLite llamada `password`, `token`, `api_key`…) |

`<kind>` es `jwt`, `bearer-token`, `api-key` (formas de claves de Google, AWS, Stripe, GitHub y Slack), `secret`, `email` o
`card-number` (un número que pasa la comprobación de Luhn, así que los números de pedido no se confunden con tarjetas). Los marcadores de posición como
`password=****` y `${TOKEN}` se dejan en paz, y también las fechas y los datos binarios bajo un nombre que suena a secreto.

## De dónde vienen las peticiones

La auditoría lee las peticiones del proxy. En la app de escritorio, inicia primero el proxy en la vista **Network**: la auditoría
incluirá entonces lo que haya grabado. En la línea de comandos, pasa un archivo HAR (`--har`, de `emulabzip net --har`). Un cuerpo
solo se juzga cuando la petición fue por HTTP simple, porque por HTTPS lo que la app envía al servidor es asunto suyo.

## El log

El log del dispositivo lo comparten todas las apps. EmuLabZip lee las líneas del proceso propio de la app (y las que la mencionan), así que
la app tiene que estar en ejecución. Si no lo está, el área se informa como **not read** en lugar de escanear las líneas del sistema,
que serían secretos de otras personas y números que solo parecen tarjetas.

## En `ci`

```bash
emulabzip ci -o run.ezsession --app com.example.shop --network --fail-on-security warning -- ./gradlew connectedCheck
```

Después del comando, se ejecuta la auditoría (con las peticiones que grabó el proxy) y la ejecución falla cuando un hallazgo es al menos tan
grave como se indicó. Los hallazgos están en el resumen en Markdown. Consulta [el modo CI](ci.md).

## Límites

- `dumpsys package` no imprime el atributo `exported`. Un componente con un intent filter es accesible para otras
  apps, y eso es lo que se informa; uno sin filtro pero exportado no se ve.
- El almacenamiento de Android y los archivos de la app solo pueden leerse en apps **debuggable**, como en [Storage](storage.md).
- Los patrones encuentran lo que parece un secreto. Uno corto o poco común se pierde, y un valor que parece uno se
  informa: lee el hallazgo antes de actuar.
- iOS: el log solo se lee mientras la app se ejecuta; las declaraciones de privacidad se comparan con los dominios vistos en las peticiones
  grabadas, así que graba algunas primero.
- Es una primera pasada, no una prueba de penetración. No mira dentro del código de la app ni de su servidor.

## Para asistentes

Por [MCP](mcp.md), `audit_security` devuelve los mismos hallazgos. Solo lee; no cambia nada en el dispositivo.
