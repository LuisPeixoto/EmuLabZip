---
source: 06c0e7164971
description: "Cambia las filas SQLite, las SharedPreferences y los plists de una app en el dispositivo con cuidado: confirmación, copia de seguridad, verificación y deshacer."
---
# Cambiar los datos de una app

<p align="center">
  <img src="images/storage-edit.png" alt="Storage en modo de edición: se está cambiando un valor de la tabla cart" />
</p>

[Storage](storage.md) es de solo lectura hasta que activas **Edit**. Editar cambia los archivos reales de la app en el
dispositivo, así que está pensado para ser cuidadoso: cada cambio pregunta primero, detiene la app, conserva el archivo
que reemplaza, vuelve a leer el archivo nuevo para comprobarlo y se puede deshacer.

## Qué se puede cambiar

| Archivo | Escritorio | Línea de comandos |
|------|---------|--------------|
| Filas SQLite | Doble clic en una celda; **New row**; el cubo de basura de una fila | `edit cell`, `edit insert`, `edit delete` |
| SharedPreferences | Doble clic en un valor; **New key**; el cubo de basura | `edit pref-set`, `edit pref-remove` |
| Listas de propiedades (iOS) | — | `edit plist-set`, `edit plist-remove` |

DataStore, MMKV, AsyncStorage, Realm y Core Data son de solo lectura: sus formatos no se vuelven a escribir.

```bash
emulabzip edit cell com.example.shop databases/shop.db -t customers --rowid 1 --text name=Ana --set vip=1 --yes
emulabzip edit insert com.example.shop databases/shop.db -t customers --set name=Bo --yes
emulabzip edit delete com.example.shop databases/shop.db -t cart --pk id=3 --yes
emulabzip edit pref-set com.example.shop shared_prefs/settings.xml theme dark --yes
emulabzip edit pref-set com.example.shop shared_prefs/settings.xml launches 4 --as int --yes
emulabzip edit plist-set com.example.shop Library/Preferences/com.example.shop.plist flags/beta true --as boolean --yes
emulabzip edit undo <data folder>/states/edits/<folder> --yes      # the folder the edit printed
```

Un valor es un número entero o decimal cuando lo parece y texto en caso contrario; `--text column=value` fuerza texto y
`--null column` establece NULL. Una preferencia de tipo set son sus elementos separados por comas. Todos los comandos
necesitan `--yes`.

## Qué hace una edición

1. Comprueba el cambio **antes** de tocar el dispositivo: la tabla y la columna existen, una fila coincide con la clave,
   el valor tiene el tipo que se pidió.
2. Detiene la app, para que no escriba encima del cambio al salir.
3. Conserva los bytes que va a reemplazar en una carpeta de copia de seguridad (`states/edits/` en la carpeta de datos de
   EmuLabZip).
4. Escribe: en SQLite, se descargan la base de datos y su registro de escritura anticipada, el cambio se hace en una copia
   dentro de una transacción que debe cambiar **exactamente una fila**, con las claves foráneas activadas, el registro se
   incorpora al archivo (`wal_checkpoint(TRUNCATE)`), el archivo se sube, y se eliminan los `-wal` y `-shm` del
   dispositivo para que el registro viejo no se reproduzca sobre el archivo nuevo.
5. Vuelve a leer el archivo. Si no es lo que se escribió, se restaura el anterior y la edición falla.

Una fila se encuentra por `rowid`, o por su clave primaria en las tablas creadas `WITHOUT ROWID`. Una tabla sin una clave
utilizable se muestra con el motivo y no se puede editar. Las vistas no se pueden editar.

En Android el archivo se escribe mediante `run-as`, así que conserva a la app como propietaria; en iOS es una escritura
de archivo del host que rechaza una ruta cuyos enlaces simbólicos salen del contenedor.

## Deshacer

La app de escritorio tiene **Undo last change**. En la línea de comandos cada edición imprime la carpeta que se le da a
`edit undo`. Deshacer detiene la app y devuelve cada archivo de esa edición a su sitio, o lo elimina si no existía.

Para algo más grande, [guarda antes el estado de la app](app-state.md).

## Para asistentes

La edición **no** se ofrece por [MCP](mcp.md).
