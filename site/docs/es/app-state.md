---
source: 2240302ecefc
description: Guarda, restaura y compara el estado de una app (bases de datos, preferencias, archivos) en un archivo .ezstate para repetir errores y probar migraciones.
---
# Estado de la app: guardar, restaurar y comparar

<p align="center">
  <img src="images/state-compare.png" alt="Un estado guardado abierto en Storage, comparado con otro: se añadieron una columna, un índice y una tabla" />
</p>

El estado de una app es lo que conserva: bases de datos, preferencias, archivos. **Save state** lo pone todo en un solo archivo `.ezstate`;
**Restore** lo devuelve; **compare** dice qué cambió en su estructura entre dos guardados. Es lo que necesitas para
repetir un error con los mismos datos cada vez, para probar una migración y para empezar una prueba desde un punto conocido.

```bash
emulabzip state save com.example.shop -o v1.ezstate
# …use the app, or install the new version…
emulabzip state save com.example.shop -o v2.ezstate
emulabzip state diff v1.ezstate v2.ezstate
emulabzip state restore v1.ezstate --yes
```

## Qué se guarda

| | Android | iOS Simulator |
|---|---------|---------------|
| Qué | La carpeta privada de la app (`/data/data/<package>`) salvo `cache`, `code_cache` y `lib`; con `--external`, también su carpeta en el almacenamiento compartido | El contenedor de datos (`Documents`, `Library`, `tmp`) |
| Cómo | `run-as <package> tar` (la app debe ser **debuggable**, como para [Storage](storage.md)) | Un tar del contenedor, escrito por EmuLabZip |
| La app | Se detiene antes, para que ningún archivo quede a medio escribir | Se termina antes |

Un archivo `.ezstate` es un zip: `state.json` (la app, su versión, el dispositivo, la hora), el tar y `schemas.json`, una
descripción de cada base de datos SQLite (tablas, columnas, índices) y de cada archivo de preferencias, DataStore y lista de propiedades
(claves y tipos). Esa descripción es lo que hace rápida la comparación: no hay que desempaquetar nada.

## Restaurar

`state restore` reemplaza los datos de la app con lo que hay en el archivo. Este comando:

1. rechaza un archivo hecho en otra plataforma, y avisa cuando la app es de otra versión que aquella de la que viene el estado;
2. necesita `--yes` (la app de escritorio pregunta antes);
3. **guarda los datos que reemplaza** en un archivo en `states/auto/` de la carpeta de datos de EmuLabZip, e imprime su nombre. Restaurar
   ese archivo deshace la restauración. Si no se puede hacer la copia de seguridad, no se reemplaza nada;
4. detiene la app, vacía la carpeta de la app (conservando `lib` en Android), desempaqueta el tar y avisa si algo falló.

Cada entrada de un tar se comprueba antes de escribir nada: sin rutas absolutas, sin `..`, sin enlaces duros, sin dispositivos y
sin enlaces simbólicos que apunten fuera de la carpeta. Un archivo `.ezstate` de otra persona no puede escribir fuera de la app.

Android ejecuta el desempaquetado como la app (`run-as`), así que los archivos conservan a la app como propietaria.

## La diferencia de esquema

Una línea por cambio: `+` añadido, `-` eliminado, `~` modificado, luego el archivo, qué es y su nombre, y el detalle.

```
$ emulabzip state diff v1.ezstate v2.ezstate
+ databases/shop.db: column cart.note  TEXT
+ databases/shop.db: index idx_cart_sku  CREATE INDEX idx_cart_sku ON cart (sku)
+ databases/shop.db: table orders  3 columns
```

Lista tablas, columnas (con su tipo, `NOT NULL` y clave primaria), índices, y las claves y tipos de los archivos de preferencias,
DataStore y listas de propiedades, añadidos, eliminados o modificados. El contenido de las filas no se compara: esto sirve para comprobar una
migración, no para comparar datos. `state diff v1.ezstate` sin segundo archivo compara con la app tal como está ahora.
`--json` lo entrega como datos.

## En la app de escritorio

Storage tiene **Save state…** y **Open state…** en su barra de herramientas. Un estado abierto muestra lo que contiene y ofrece
**Compare with the app now**, **Compare with another file…** y **Restore…**. Restore pregunta antes, e indica dónde se guardan los datos
antiguos.

## Para asistentes

Por [MCP](mcp.md) se ofrecen `save_state` y `diff_state`, y `list_states` muestra los nombres guardados. Restaurar
**no** se ofrece: un asistente puede ver qué cambió, no reemplazar los datos de una app.

## Límites

- Android necesita una app debuggable. La carpeta privada de un build de release no puede leerla ninguna herramienta.
- Los datos que solo están en memoria, el llavero (keychain) y lo que otras apps guardan para esta (cuentas, content providers) no forman parte de la
  carpeta, así que no forman parte del estado.
- iOS: los contenedores de app group no se guardan.
- Un estado no incluye la app en sí: instala la misma versión antes de restaurar.
