---
source: 6a33aedfa838
description: EmuLabZip lee un .hprof de Android por sí mismo y lista las fugas sospechosas, los dominadores y las clases por tamaño, sin otra herramienta.
---
# Encontrar fugas en volcados de heap

EmuLabZip lee un `.hprof` de Android por sí mismo y lista lo que mantiene la memoria viva, sin necesitar otra herramienta.

```bash
emulabzip heapdump com.example.app --analyze         # take a dump and analyze it
emulabzip heap analyze before-login.hprof            # a dump you already have
emulabzip heap analyze dump.hprof --json --top 20
```

La app tiene que ser **depurable** (o el dispositivo, una imagen `eng`/`userdebug`) para poder volcarla.

## Qué informa

**Suspected leaks**, los casos que son fugas sin duda, con la ruta que mantiene vivo a cada uno:

- una `Activity` que fue **destruida** (`mDestroyed`), o que fue **finalizada** (`mFinished`, informada solo como *posible*,
  porque puede que el sistema aún no la haya destruido) y sigue en memoria;
- un `Fragment` (`androidx.fragment.app` o `android.app`) sin `mFragmentManager`, lo que significa que fue eliminado.

Para cada uno: su clase y dirección, **cuánta memoria mantiene viva** y la ruta más corta desde una raíz del GC, por
ejemplo:

```text
static field: class com.example.LeakyActivity.kept → com.example.LeakyActivity
```

Lee la ruta de derecha a izquierda: la última línea es el objeto con la fuga, y la entrada anterior es lo que lo
retiene. Las referencias weak, soft y phantom (`Reference.referent`) no se siguen, porque no mantienen vivo un objeto.

**Objects that keep the most alive**, por retained size. El retained size es lo que se liberaría si el objeto se
recolectara; se calcula a partir del árbol de dominadores, así que los objetos compartidos no se cuentan dos veces.

**Classes by size**: número de instancias y shallow size.

Por defecto solo se mira el heap propio de la app; `--all-heaps` añade el zygote y la imagen del sistema.

## Pruébalo

`tools/leak-app` es una pequeña app depurable que provoca a propósito la fuga de una Activity (consulta su README):

```bash
tools/leak-app/install.sh
adb shell am start -n dev.emulabzip.leaky/.MainActivity --es mode leak
emulabzip heapdump dev.emulabzip.leaky --analyze
```

## Simulador de iOS

Un `.memgraph` se lee con la herramienta `leaks` del Mac, así que necesita un Mac. El informe lista los **retain cycles y
objetos con fuga** por tipo, con sus tamaños (no hay retained sizes). `tools/crash-app-ios --leak` crea un retain cycle
para probarlo.

## Dónde encontrarlo

- **Escritorio**: Memory → **Heap dump…** y luego **Find leaks**, u **Open dump…** para un archivo en disco. Tres pestañas:
  Suspected leaks, Dominators y Classes.
- **CLI**: `emulabzip heap analyze`, `emulabzip heapdump --analyze`.
- **MCP**: `analyze_heap` (Android).

Analizar un volcado de 40 MB tarda unos segundos. Un volcado de heap contiene los datos de la app (consulta [Security](../SECURITY.md)), así que
mantenlo tan privado como la app.
