---
source: 18e7488c6820
description: "Importa un archivo .pcap o .pcapng en EmuLabZip y míralo como tráfico en vivo: peticiones HTTP, conexiones TLS y TCP, consultas DNS y cómo capturar."
---
# Importar una captura de paquetes

EmuLabZip puede abrir un archivo `.pcap` o `.pcapng` y mostrar lo que contiene como muestra el tráfico en vivo. Es útil cuando
no puedes usar el proxy: una app que ignora la configuración de proxy, tráfico de un servicio del sistema o una captura que alguien te
envió.

```bash
emulabzip net import capture.pcap            # lists the requests
emulabzip net import capture.pcap --dns      # and the DNS lookups
emulabzip net import capture.pcap --har out.har
```

En la app, pulsa **Import** en la vista Network. Las peticiones se unen a la lista con una insignia `PCAP`, y un banner muestra
lo que contenía el archivo, con las consultas DNS a un clic.

## Qué se obtiene

| En la captura | En EmuLabZip |
|----------------|--------------|
| HTTP/1.x simple sobre TCP | Peticiones y respuestas, con cabeceras y cuerpos (los fragmentados y comprimidos se decodifican) y tiempos |
| Una conexión TLS | Una **conexión** con el nombre del servidor de su primer mensaje (SNI), con los bytes en cada sentido |
| Cualquier otra conexión TCP | Una conexión a su dirección |
| DNS sobre el puerto UDP 53 | Una lista de consultas con sus respuestas y su estado |

TLS sigue cifrado, así que su contenido no está disponible en una captura; usa el proxy para eso. Los flujos TCP se reconstruyen
por número de secuencia: los segmentos desordenados y retransmitidos se gestionan, y una pieza que falta se informa
en lugar de adivinarse ("some packets are missing"). Los paquetes IP fragmentados se omiten. Se leen archivos de hasta 512 MB.

Se entienden capturas Ethernet, Linux "cooked" (interfaz `any`), IP sin procesar y loopback, en cualquier orden de bytes y con
marcas de tiempo en nanosegundos.

## Capturar

En un emulador Android, inícialo con `-tcpdump`:

```bash
emulator -avd Pixel_9 -tcpdump capture.pcap
```

Todo lo que transporta la red emulada se escribe en el archivo, y queda completo cuando el emulador termina. En una
imagen o dispositivo con root puedes usar `tcpdump` desde el shell: `adb shell tcpdump -i any -w /sdcard/capture.pcap`, y luego
`adb pull`. En un Mac, `tcpdump -i lo0 -w capture.pcap` en la interfaz loopback captura el tráfico de un simulador.
