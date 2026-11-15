---
source: 0c82c94d90e4
description: La herramienta Network es un proxy de depuración integrado que descifra HTTPS y lista cada petición con sus headers, cuerpo y tiempos, sin cambiar tu app.
---
# Red

La herramienta Network es un proxy de depuración integrado en EmuLabZip. Se coloca entre tu app e internet, descifra HTTPS
con una autoridad certificadora que vive solo en tu ordenador y lista cada petición con sus headers, cuerpo y tiempos. No
tienes que cambiar tu app.

<p align="center">
  <img src="images/network.png" alt="La herramienta Network listando peticiones, con una respuesta JSON abierta" />
</p>

## Inicio rápido

1. Abre **Network** (<kbd>⌘6</kbd>) y elige tu dispositivo.
2. Pulsa **Certificate** una vez por dispositivo, para que confíe en el certificado de EmuLabZip (consulta [más abajo](#the-certificate)).
3. Pulsa **Connect**. El proxy se inicia si no está en ejecución, y el dispositivo le envía su tráfico HTTP y HTTPS.
4. Usa tu app. Las peticiones aparecen a medida que terminan. Haz clic en una para ver sus headers, cuerpo y tiempos.
5. Pulsa **Disconnect** (o simplemente cierra) cuando termines. EmuLabZip deja el dispositivo como lo encontró.

Desde una terminal:

```bash
emulabzip ca install                 # once per device
emulabzip net --har capture.har      # Ctrl+C to stop; the device is restored
```

## Cómo llega un dispositivo al proxy

El proxy solo escucha en `127.0.0.1` de tu ordenador, nunca en la red.

| | Android | Simulador de iOS |
|---|---------|------------------|
| Ruta | `adb reverse tcp:<port> tcp:<port>` hace que `127.0.0.1:<port>` en el dispositivo llegue a tu ordenador, y luego `settings put global http_proxy 127.0.0.1:<port>` | El simulador comparte la red del Mac, así que el proxy HTTP y HTTPS **propio del Mac** apunta a EmuLabZip |
| Alcance | El único dispositivo que conectas | **Todas las apps del Mac** mientras está activo |
| Se deshace con | Disconnect, cerrar, o que `emulabzip net` se detenga. Solo se elimina el forward que creó EmuLabZip; el `tcp:8081` de Metro se queda | Disconnect, cerrar, o el siguiente inicio si EmuLabZip fue terminado a la fuerza |
| Funciona con | Emuladores, dispositivos USB, depuración inalámbrica | Simuladores en macOS |

En el Mac, EmuLabZip guarda los ajustes de proxy de cada servicio de red antes de cambiarlos y los restaura
exactamente, incluido un proxy que estaba desactivado pero conservaba una dirección. Si macOS pide una contraseña de
administrador la primera vez, es `networksetup` cambiando ajustes del sistema.

## El certificado

Para leer HTTPS, EmuLabZip responde con un certificado para el sitio, firmado por su propia autoridad certificadora. El
dispositivo tiene que confiar en esa autoridad. Se crea en el primer uso, se llama *EmuLabZip CA* y se guarda en
`~/Library/Application Support/EmuLabZip` (`%APPDATA%\EmuLabZip` en Windows, `~/.local/share/EmuLabZip` en Linux),
con la clave privada legible solo por ti. Define `EMULABZIP_HOME` para usar otra carpeta.

| Dispositivo | Qué hace **Certificate** | Qué apps confían en él |
|-------------|--------------------------|------------------------|
| Simulador de iOS | `simctl keychain add-root-cert` | Todas. Reinicia las apps que ya estaban en ejecución |
| Emulador de Android cuya imagen permite `adb root` (Google APIs, AOSP) | Lo añade al almacén del **sistema** | **Todas las apps**, hasta que el dispositivo se reinicie |
| Imagen de Android de Google Play, o un dispositivo físico | Copia `emulabzip-ca.crt` a Downloads y abre Settings para que lo instales como certificado de **usuario** | Chrome y las apps que lo permiten. Tus propios builds de depuración pueden permitirlo con el fragmento de abajo |

En una imagen con root, `adbd` se deja en ejecución como root, igual que después de `adb root`. Volver a ejecutar
**Certificate** es seguro: reemplaza el archivo en lugar de apilar otra copia.

### Que tu build de depuración confíe en él

Desde Android 7, las apps ignoran los certificados de usuario salvo que los pidan. Añade esto solo a tu build de
**depuración** y refiérelo desde el manifest de ese build (`android:networkSecurityConfig="@xml/network_security_config"`):

```xml
<?xml version="1.0" encoding="utf-8"?>
<network-security-config>
    <debug-overrides>
        <trust-anchors>
            <certificates src="system" />
            <certificates src="user" />
        </trust-anchors>
    </debug-overrides>
</network-security-config>
```

Los builds de release no se ven afectados, porque `debug-overrides` solo se aplica a las apps depurables.

### Sin instalar nada

Con el proxy en ejecución, un navegador del dispositivo puede obtener el certificado del propio proxy:
`http://127.0.0.1:9090/ca.crt` (DER, para Android e iOS) o `/ca.pem`.

## Qué ves

Cada fila es una petición. Las columnas son el estado, el método, el host, la ruta, el tipo, el tamaño y el tiempo. Filtra
por texto (URL, método o estado), por clase de estado o por método. Usa <kbd>↑</kbd> <kbd>↓</kbd> para moverte por la lista.

El panel de detalles tiene cuatro pestañas:

| Pestaña | Muestra |
|---------|---------|
| Headers | Los headers de la petición y de la respuesta |
| Request | El cuerpo que se envió |
| Response | El cuerpo que volvió |
| Timing | El protocolo, el tiempo hasta el primer byte y el tiempo hasta el final |

Los cuerpos se decodifican para leerlos (gzip, deflate, Brotli, zstd): ves el tamaño en la red y el tamaño decodificado.
El JSON se formatea, las imágenes se muestran y los datos binarios se muestran como volcado hexadecimal. El proxy reenvía
los bytes exactamente como eran; solo la copia que lees está decodificada.

Las **Connections** son lo que el proxy no pudo decodificar: tráfico que no es HTTP y apps que rechazan el certificado. Se
listan para que sepas que existen, con el motivo. Los WebSockets se listan con una pestaña **Messages** que muestra cada
mensaje (consulta [Mensajes de WebSocket](websockets.md)).

La columna **Proto** muestra lo que el proxy habló con el servidor: `h1`, `h2` o `h3`.

## Más que observar

- **Rules**: responde tú mismo a una petición, ralentízala, bloquéala o detenla y edítala. Consulta
  [Mocks y breakpoints](mocks.md).
- **Import**: abre una captura `.pcap` o `.pcapng` como peticiones. Consulta [Importar una captura](pcap.md).
- **Send again**: **Resend** envía una petición otra vez tal como era; **Edit and resend** te deja cambiar primero el método,
  la dirección, los headers y el cuerpo. La copia es una línea nueva en la lista marcada con `RESEND`. Las reglas y los
  breakpoints no se aplican a ella, porque tú elegiste enviarla. Una petición simulada, un WebSocket y una petición leída
  de una captura no se pueden enviar de nuevo, ni tampoco una cuyo cuerpo era demasiado grande para conservarlo.
- **GraphQL y gRPC**: las operaciones se nombran en la lista y los cuerpos se leen como significa su protocolo. Consulta
  [GraphQL](graphql.md) y [gRPC y protobuf](grpc.md).
- **Network conditions**: una conexión lenta, con latencia, con pérdidas o inexistente. Consulta [Condiciones de red](network-conditions.md).
- **Replay**: responde desde un archivo HAR o una sesión grabada, para que la app funcione sin internet. Consulta
  [Repetir una grabación](replay.md).
- **HTTP/3**: consulta más abajo.

![El editor para enviar de nuevo una petición GraphQL con otra variable](images/network-resend.png)

### Apps que fijan su certificado

Algunas apps (muchos servicios de Google, apps bancarias) aceptan solo el certificado de su propio servidor. EmuLabZip no
puede descifrarlas sin romperlas, así que después de que un host rechace el certificado una vez, las conexiones
posteriores a él **pasan sin cambios**. La app sigue funcionando, la lista muestra una conexión fallida con el motivo y un
banner nombra los hosts. **Decrypt again** los reintenta, por ejemplo después de instalar el certificado.

## Exportar

El botón de descarga guarda las peticiones como un archivo [HAR](http://www.softwareishard.com/blog/har-12-spec/), que pueden leer
Chrome DevTools, Firefox y la mayoría de las herramientas de API. Los cuerpos se guardan decodificados. Las conexiones se
dejan fuera. En la línea de comandos: `emulabzip net --har capture.har`.

## HTTP/3

Un dispositivo detrás de un proxy HTTP nunca habla QUIC con él, así que HTTP/3 solo afecta al segundo tramo del proxy, hacia
el servidor. Los servidores anuncian HTTP/3 en un header `Alt-Svc: h3=":443"`. El proxy lo recuerda por host, y con
**HTTP/3 → Use HTTP/3 to servers** activado (está desactivado por defecto), las peticiones posteriores a ese host van por
QUIC. Si QUIC falla (una red que bloquea UDP, un servidor que no responde), la petición vuelve a HTTP/2 o 1.1 y el host se
queda en TCP durante 10 minutos. Los flujos muestran `h3` en la columna Proto cuando funcionó. Una respuesta HTTP/3 se lee
completa antes de pasarla, así que una descarga muy grande no se transmite en streaming.

**Hide HTTP/3 from apps** (activado por defecto) elimina las entradas `h3` del header `Alt-Svc` que recibe la app. Sin esto,
una app puede enterarse de que el servidor habla HTTP/3 e intentar QUIC directamente, esquivando el proxy, y su tráfico
desaparecería de la lista. El flujo sigue registrando el header original. En la línea de comandos: `emulabzip net --http3`
y `--show-h3` para mostrar las ofertas a las apps.

## Límites

- Solo se almacenan los primeros **10 MB** de cada cuerpo. El resto sigue llegando a su destino.
- La lista conserva las **10.000** peticiones más recientes.
- Una app que habla QUIC directamente, esquivando el proxy, no se puede decodificar. Con el ajuste por defecto no se informa a las apps de HTTP/3, así que se quedan en HTTP/2 a través del proxy.
- El proxy es para **tus propias** apps y dispositivos. No lo apuntes a tráfico que no tengas permiso para inspeccionar.

## Línea de comandos

```bash
emulabzip net                       # the only running device; prints one line per request
emulabzip net --json | jq 'select(.status >= 400)'
emulabzip net --no-connect --port 9191   # only run the proxy
emulabzip net --rules rules.json    # mock, delay or block requests
emulabzip net --http3               # talk HTTP/3 to servers that offer it
emulabzip net --replay run.ezsession   # answer from a recording; --passthrough lets the rest through
emulabzip net import capture.pcap   # read a packet capture
emulabzip ca install                # make the device trust the certificate
emulabzip ca path                   # where the certificate file is
```

Consulta [Línea de comandos](cli.md) para ver todas las opciones.
