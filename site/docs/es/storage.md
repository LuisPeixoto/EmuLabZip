---
source: 236e4411c357
description: La herramienta Storage explora los archivos de la app seleccionada y abre bases de datos, preferencias, plists e imágenes en el lugar, también desde la CLI.
---
# Storage

La herramienta Storage explora los archivos de la app seleccionada en la barra de herramientas y abre en el lugar los
formatos habituales.

## Dónde puedes mirar

| Raíz | Android | iOS Simulator |
|------|---------|---------------|
| Datos de la app | `/data/data/<package>`, leído con `run-as`. **Solo apps depurables.** | El contenedor de datos (`Documents`, `Library`, `tmp`) |
| Almacenamiento compartido | `/sdcard/Android/data/<package>`, mediante el protocolo sync de ADB | — |
| Bundle de la app | — | El `.app` instalado |
| App groups | — | Cada contenedor de app group que usa la app |

Los builds de depuración de tu app son depurables de forma predeterminada. Los builds de release y la mayoría de las apps
preinstaladas no lo son, y Android no deja que ninguna herramienta lea sus datos privados. EmuLabZip lo dice en lugar de
mostrar una carpeta vacía.

Para probar las herramientas sin una app propia, instala la app de ejemplo: `tools/sample-app/install.sh`.

## Visores

| Archivo | Visor |
|------|--------|
| Bases de datos SQLite | Tablas y vistas con recuento de filas, tipos de columna y claves primarias, 100 filas por página y un cuadro SQL de solo lectura |
| SharedPreferences (`shared_prefs/*.xml`) | Clave, tipo y valor; los sets muestran sus elementos |
| Preferences DataStore (`datastore/*.preferences_pb`) | Clave, tipo y valor, decodificados desde el protobuf |
| Proto DataStore (`datastore/*.pb`) | El protobuf decodificado sin su esquema: números de campo y valores |
| MMKV (un archivo en `mmkv/` junto a su `.crc`) | Clave y valor. MMKV no guarda tipos, así que cada valor se muestra como el tipo al que se parece (string, número, bytes, set) y avisa de que es una suposición; el hex sin procesar está a un clic. Un archivo cifrado se reconoce y se explica |
| Realm (`*.realm`) | Clases con su recuento de filas, tipos de columna y filas, leídas una página a la vez con un lector del formato de archivo de Realm (versiones 20 a 24). Se muestran listas, enlaces y marcas de tiempo; las columnas de sets, diccionarios, mixed, decimal y UUID aún no se leen y lo dicen |
| AsyncStorage (`RKStorage`, o `manifest.json` en iOS) | Claves y valores, con los valores JSON con formato legible |
| Core Data (un almacén SQLite) | Entidades con sus atributos y filas (sin el prefijo `Z`), junto a las tablas sin procesar |
| Listas de propiedades (binarias o XML) | Con formato legible como JSON |
| JSON | Con formato legible |
| Imágenes (PNG, JPEG, GIF, WebP) | Vista previa sobre un tablero de ajedrez |
| Texto | Tal cual, hasta 2 MB |
| Cualquier otra cosa | Los primeros 4 KB como volcado hexadecimal |

Las bases de datos se copian a una carpeta temporal privada junto con sus archivos `-wal` y `-shm`, así que se ven los
cambios que la app aún no ha confirmado con un checkpoint. La copia se abre con `query_only` activado: las consultas
pueden leer pero nunca escribir, y la base de datos real de la app nunca se toca. Las copias temporales se borran cuando
EmuLabZip se cierra.

**Save…** copia el archivo original al lugar que elijas.

![Un archivo Realm leído desde el dispositivo: sus clases y las filas de Person](images/storage-realm.png)

## Cambiar y guardar

Storage es de solo lectura hasta que activas **Edit**: consulta [Cambiar los datos de una app](storage-editing.md).
**Save state…**, **Open state…** y la comparación de esquemas están en [Estado de la app](app-state.md).

## Desde la línea de comandos

```bash
emulabzip files com.example.app                   # the first readable root
emulabzip files com.example.app databases --root data
emulabzip pull com.example.app shared_prefs/settings.xml
emulabzip db com.example.app databases/app.db                     # tables
emulabzip db com.example.app databases/app.db --table users       # rows
emulabzip db com.example.app databases/app.db -q "SELECT count(*) FROM users"
emulabzip edit cell com.example.app databases/app.db -t users --rowid 1 --text name=Ana --yes   # see Changing an app's data
```
