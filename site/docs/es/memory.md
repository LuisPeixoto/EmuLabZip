---
source: e2d7a7419077
description: La herramienta Memory muestra adónde va la memoria de la app, compara dos momentos para ver qué creció y genera volcados de heap en Android e iOS.
---
# Memoria

La herramienta Memory muestra adónde va la memoria de la app seleccionada y te deja comparar dos momentos para ver qué creció.

## Snapshots

Haz clic en **Take snapshot** para registrar un desglose:

| | Android | Simulador de iOS |
|---|---|---|
| Fuente | `dumpsys meminfo <package>` | `footprint --json <pid>` en el Mac |
| Total | PSS (proportional set size) | Footprint, el número que iOS usa para decidir cuándo terminar una app |
| Categorías | Java heap, native heap, código, stack, gráficos, private other, sistema | Regiones de VM como `MALLOC_SMALL`, `__DATA`, `CoreAnimation` |
| Detalles | RSS total, swap y recuentos de objetos (vistas, activities, binders, WebViews…) | Memoria limpia y recuperable, número de regiones |

Toma un snapshot, usa la app (abre y cierra una pantalla varias veces) y toma otro. El segundo snapshot se compara
automáticamente con el anterior; elige cualquier otra línea base en el menú **Compared with**. Las categorías se listan por
el tamaño de su cambio, con una flecha y un signo para que la dirección nunca dependa solo del color.

Una categoría que sigue creciendo con acciones repetidas es un buen lugar donde buscar una fuga.

## Volcados de heap

| | Android | Simulador de iOS |
|---|---|---|
| Botón | **Heap dump…** | **Memory graph…** |
| Archivo | `.hprof`, escrito con `am dumpheap` | `.memgraph`, escrito con `leaks --outputGraph` |
| Ábrelo con | Android Studio, Eclipse MAT o [Perfetto](https://ui.perfetto.dev) | Xcode (File › Open), `leaks`, `heap`, `vmmap` |
| Requisito | Una app **depurable** (o una imagen `eng`/`userdebug`) | Ninguno |

En Android, el volcado se escribe en `/data/local/tmp`, se copia a tu ordenador y se elimina del dispositivo.

EmuLabZip también puede leer el volcado por sí mismo: **Find leaks** (u **Open dump…** para un archivo en disco) lista las
Activities destruidas y los Fragments eliminados que siguen vivos, la ruta que mantiene a cada uno y los objetos que
mantienen vivos más memoria. Consulta [Finding leaks](leaks.md).

## Desde la línea de comandos

```bash
emulabzip mem com.example.app
emulabzip mem com.example.app --json
emulabzip heapdump com.example.app -o before-login.hprof
emulabzip heapdump com.example.app --analyze
emulabzip heap analyze before-login.hprof
```
