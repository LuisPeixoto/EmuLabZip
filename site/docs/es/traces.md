---
source: 628549c96a94
description: La herramienta Traces graba lo que hicieron el sistema y una app durante unos segundos con Perfetto, simpleperf e Instruments, y te ayuda a ver por qué fue lento.
---
# Trazas

La herramienta Traces graba lo que hicieron el sistema y una app durante unos segundos, y luego te ayuda a descubrir por
qué fue lento. Reúne tres herramientas oficiales y guarda su salida en una sola biblioteca:

| Grabación | Dispositivo | Herramienta | Qué obtienes en EmuLabZip |
|-----------|-------------|-------------|---------------------------|
| **System trace** | Android 9 y posteriores | [Perfetto](https://perfetto.dev) | Un resumen (CPU, fotogramas, memoria, slices más ocupados), una consola SQL y un botón para abrir la línea de tiempo completa en la UI de Perfetto |
| **CPU profile** | Android 8 y posteriores | `simpleperf` | Un flame graph con funciones calientes y exportación de stacks plegados |
| **Instruments** | Simulador de iOS | `xcrun xctrace` | Un flame graph para grabaciones de Time Profiler y un botón para abrir el `.trace` en Instruments |

<p align="center">
  <img src="images/traces-summary.png" alt="El resumen de una traza de fotogramas de Perfetto" />
</p>

## Inicio rápido

1. Abre **Traces** (<kbd>⌘7</kbd>) y elige tu dispositivo. Elige una app en la barra de herramientas si quieres centrarte en una.
2. Elige qué grabar, define la duración (de 1 a 300 segundos) y pulsa **Record**. <kbd>Stop</kbd> la termina antes y
   conserva lo grabado hasta ese momento.
3. La grabación se abre sola. Cada grabación se conserva en la lista **Recordings** hasta que la eliminas.

Desde una terminal:

```bash
emulabzip trace record --preset frames --duration 15 -o frames.pftrace
emulabzip trace summary frames.pftrace
emulabzip trace query frames.pftrace "select name, count(*) n from slice group by name order by n desc limit 10"
emulabzip trace cpu --app com.example.app --duration 10 --folded stacks.txt
```

## Trazas del sistema (Perfetto)

Cuatro presets cubren las preguntas habituales. Son configuraciones de texto normales de Perfetto, así que **Custom**
acepta las tuyas.

| Preset | Graba | Úsalo para responder |
|--------|-------|----------------------|
| **CPU & scheduling** | Cambios y despertares del planificador, frecuencia de CPU y estados de reposo, la lista de procesos | ¿Qué hilos usaron la CPU y estaba disponible? |
| **Frames & jank** | La línea de tiempo de fotogramas de SurfaceFlinger, más los slices de vistas, gráficos, entrada y binder a su alrededor | ¿Qué fotogramas llegaron tarde y qué se estaba ejecutando cuando lo hicieron? |
| **Memory** | Memoria de procesos (`mem.rss`) y contadores de memoria del sistema cada 250 ms, más asignaciones nativas (`heapprofd`) cuando se elige una app | ¿Dónde crece la memoria? |
| **Custom** | Lo que diga tu configuración | Cualquier otra cosa |

La pestaña **Summary** muestra:

- la duración de la traza, lo ocupada que estuvo la CPU, el número de fotogramas y el tiempo de fotograma en los percentiles 50, 90 y 99;
- el tiempo de CPU por proceso y por hilo;
- para los fotogramas: los tipos de jank (como los nombra SurfaceFlinger) y, por capa, cuántos fotogramas llegaron tarde. Un
  fotograma cuenta como tardío cuando la app o el compositor no cumplió su plazo; *Prediction Error* y *Buffer Stuffing* por sí solos no;
- memoria: los contadores del sistema más bajos y más altos y la memoria residente máxima por proceso;
- los slices con más tiempo en total.

Una sección se omite cuando la traza no tiene datos para ella. Una traza de memoria no tiene planificación, y una traza de
CPU no tiene fotogramas. Usa la pestaña **SQL** para todo lo demás: ejecuta [PerfettoSQL](https://perfetto.dev/docs/analysis/perfetto-sql-syntax)
(<kbd>⌘</kbd>/<kbd>Ctrl</kbd> + <kbd>Enter</kbd>) con tablas como `slice`, `sched`, `thread`, `process` y
`counter`, y muestra hasta 5.000 filas.

### El trace processor

Leer una traza de Perfetto necesita el `trace_processor_shell` de Google. EmuLabZip no lo incluye. La primera vez que abres
una traza de Perfetto explica la descarga y pregunta antes:

- viene del release oficial de Perfetto en GitHub (versión fijada en el código fuente, de 12 a 81 MB según la
  plataforma);
- el archivo se comprueba contra un SHA-256 fijado en el código fuente, y se descarta si no coincide;
- se guarda en `tools/` dentro de la carpeta de datos de EmuLabZip, y se inicia solo en `127.0.0.1`, en un puerto libre,
  cuando abres una traza. Se detiene cuando abres otra traza o cierras.

### Abrir en la UI de Perfetto

**Open in Perfetto UI** sirve el archivo una vez desde `127.0.0.1` con los headers que necesita [ui.perfetto.dev](https://ui.perfetto.dev),
y luego abre tu navegador. La página carga la traza desde tu propio ordenador: el archivo nunca se sube. El servidor
solo responde por ese archivo y se detiene después de que se haya descargado o pasados dos minutos.

## Perfiles de CPU (simpleperf)

`simpleperf` muestrea la CPU con call stacks. EmuLabZip lo ejecuta en el dispositivo, convierte su informe en el dispositivo
y recoge el resultado, así que no hace falta el NDK en tu ordenador.

- **Una app**: la app tiene que estar en ejecución, y ser *debuggable* o *profileable*. Si no, simpleperf se niega y EmuLabZip
  lo dice. No espera a una app que no está en ejecución.
- **Whole system**: muestrea todos los procesos. Necesita `adb root`, así que usa una imagen de emulador sin Google Play.

El flame graph es un icicle: la llamada más externa arriba, con un ancho proporcional al tiempo de CPU. Haz clic en un
fotograma para acercarte, usa el breadcrumb para alejarte, escribe para resaltar una función y elige otro proceso o hilo
encima. Los colores indican de quién es el código de un fotograma:

| Color | Significado |
|-------|-------------|
| Azul | El código propio de la app: apps instaladas, sus bibliotecas y métodos de la app compilados con JIT |
| Verde | Código gestionado de la plataforma: el framework de Android, ART, `*.jar`, `*.odex`, `*.oat` |
| Naranja | Bibliotecas nativas (`.so`) |
| Gris | El kernel |

<p align="center">
  <img src="images/traces-flame.png" alt="Un perfil de CPU de todo el sistema como flame graph" />
</p>

Bajo el gráfico, **Hot functions** lista dónde se gastó el tiempo en una función misma y no en lo que llamó.
**Folded stacks** guarda el árbol de llamadas para `flamegraph.pl` y speedscope. El `perf.data` sin procesar se conserva junto a la
grabación; *Show in folder* lo revela.

## Instruments (simulador de iOS)

EmuLabZip ejecuta `xcrun xctrace record` para cualquier plantilla que informe `xctrace list templates`, asociado a la app
que elegiste (o a todos los procesos). El resultado es un documento `.trace`: **Open in Instruments** lo abre. Para **Time Profiler**
y **CPU Profiler**, EmuLabZip también exporta las muestras (`xctrace export`) y dibuja el mismo flame graph.

Requisitos:

- Xcode, con `xcrun xctrace` disponible.
- **Developer Mode** en el Mac. Sin él, `xctrace` espera una aprobación que nadie puede dar. EmuLabZip comprueba
  `DevToolsSecurity -status` primero y te dice qué hacer: ejecuta `sudo DevToolsSecurity -enable` en una terminal.
- Para una sola app, tiene que estar en ejecución en el simulador.

El lector de la exportación de Time Profiler sigue la forma que documenta `xctrace` (filas con un hilo, un peso y un stack,
con los elementos repetidos escritos una vez y referenciados por id). Está cubierto por pruebas sobre una exportación escrita
a mano; no había grabaciones de un Mac con Developer Mode disponibles mientras se escribió. Si una grabación se abre sin flame graph, el
`.trace` sigue intacto y se abre en Instruments. Abre un issue con la versión de Xcode.

## Dónde viven las grabaciones

Cada grabación es una carpeta en `traces/` dentro de la carpeta de datos de EmuLabZip (`EMULABZIP_HOME` la mueve):

```text
traces/20261023-184501-3f2a/
    meta.json          what was recorded, where and when
    trace.pftrace      or profile.pb (+ perf.data), or recording.trace (+ time-profile.xml)
```

`emulabzip trace list` las imprime. Eliminar una en la app borra su carpeta. Nada que se escriba en un dispositivo se
queda sin eliminar: las configuraciones, las grabaciones sin procesar y los informes se borran del dispositivo cuando
termina una grabación, haya funcionado, fallado o se haya detenido.

## Límites

- Como máximo 300 segundos por grabación, y 1 GB por traza de Perfetto.
- Perfetto necesita Android 9 (API 28) o posterior. En Android 9 y 10, EmuLabZip activa `persist.traced.enable`, ya que la
  imagen del sistema lo trae desactivado.
