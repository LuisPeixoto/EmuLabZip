---
source: f024f5686c62
description: Convierte stacks de builds de release en nombres legibles con mapping.txt de R8, source maps de React Native y dSYM de iOS, guardados por app y versión.
---
# Stacks legibles

Un build de release oculta sus nombres. R8 y ProGuard renombran las clases a `a.b.c`, React Native entrega un único bundle
de JavaScript donde cada posición es `1:608617`, y una app de iOS sin símbolos tiene direcciones sin más. EmuLabZip los
devuelve a lo que escribiste, cuando tiene el archivo que el build generó para ese fin.

| Qué falló | El archivo | Qué obtienes |
|--------------|----------|--------------|
| Java o Kotlin, minificado con R8 o ProGuard | `mapping.txt` | `Cart.average` en `MainActivity.kt:39`, con las llamadas inlined devueltas como frames propios |
| JavaScript de React Native (Hermes o JavaScriptCore) | el source map del bundle | `App.tsx 17:14` en lugar de `1:608617` |
| Una app de iOS | su `.dSYM` | función, archivo y línea para cada dirección (en un Mac, con `atos`) |

<p align="center">
  <img src="images/symbolication.png" alt="El mismo fallo de Android, legible con su mapping.txt y tal como lo escribió el dispositivo" />
</p>

## Inicio rápido

Guarda una sola vez el archivo de la app y de la versión a la que pertenece. Desde entonces, cada fallo de ese build se
lee con él.

```bash
emulabzip mappings add com.example.shop "1.2.0 (42)" app/build/outputs/mapping/release/mapping.txt
emulabzip crash dropbox:data_app_crash:20261023205928        # readable
emulabzip crash dropbox:data_app_crash:20261023205928 --raw  # as the device wrote it
```

En la app de escritorio, abre **Crashes**. Un fallo que se podría leer mejor dice qué falta y ofrece **Add mapping or
source map…** y **Add dSYM…**: elige el archivo y el fallo se vuelve a leer. **Readable** y **Original** alternan entre
los dos.

## La versión tiene que coincidir

Un mapping solo es correcto para el build que lo generó, y leer otro build con él es peor que no leerlo. Por eso los
archivos se guardan por app y versión, y un fallo solo se lee con los de su propia versión. La versión es la que muestra
el fallo:

- **Android**: `1.2.0 (42)`, el nombre de la versión y el código de la versión, de la línea `Package:` del informe.
- **iOS**: `1.2.0 (42)`, la versión corta y el número de build.

Cuando ningún archivo encaja, el fallo lo dice y nombra la versión que necesita.

## Dónde están los archivos

Puedes guardarlos donde quieras y nombrarlos en la línea de comandos, un fallo cada vez:

```bash
emulabzip crash <id> --mapping app/build/outputs/mapping/release/mapping.txt
```

o guardarlos de forma permanente con `mappings add`. Se copian a la carpeta de datos de EmuLabZip
(`~/Library/Application Support/EmuLabZip/mappings/<app>/<version>/` en macOS), así que una compilación limpia no se los
lleva. `emulabzip mappings list` muestra lo que se guarda, y `emulabzip mappings remove <app> <version>` olvida una versión.
Qué es un archivo se deduce del propio archivo, así que no hay que indicar el tipo; `--kind mapping|sourcemap|dsym`
lo indica cuando no se puede saber.

## Android: mapping.txt

R8 lo escribe en `app/build/outputs/mapping/<variant>/mapping.txt` cuando `isMinifyEnabled` está activado. Guárdalo para
cada release que publiques: es la única vía de regreso desde los nombres de un fallo.

- Los frames se devuelven con clase, método, archivo y línea.
- Cuando R8 hizo inline de un método en su llamador, el único frame embrollado se convierte en toda la cadena, con la
  llamada interna primero, como habría sido sin R8.
- Los nombres de clase del título de la excepción también se restauran.
- Mantén `-keepattributes SourceFile,LineNumberTable` en tus reglas, o no habrá líneas que devolver.

## React Native: source maps

El JavaScript de un fallo de React Native llega en el informe como un texto de excepción, no como frames. EmuLabZip lo lee
en un hilo **JavaScript** con los hilos de Java y nativos después. Las posiciones se buscan en el source map del bundle:

- Los builds de release de **Hermes** escriben un mapa compuesto, `android/app/build/generated/sourcemaps/react/release/index.android.bundle.map`.
  Usa ese; los mapas `packager` y `compiler` que están a su lado son las dos mitades de las que se compone.
- Los bundles de **JavaScriptCore** y **Metro** usan el mapa que escribe `react-native bundle --sourcemap-output`.

## iOS: dSYM

Xcode escribe un `.dSYM` junto a cada archivo. EmuLabZip comprueba que su UUID es el de la imagen que falló, y luego le
pide a `atos` todas las direcciones de esa imagen a la vez. Esto necesita un Mac, porque `atos` forma parte de las
herramientas de Xcode; en Linux y Windows un fallo con direcciones sin más lo dice, y un fallo que ya tiene nombres se
muestra tal cual. El Simulator a menudo simboliza él mismo el informe de fallo cuando encuentra el dSYM, y entonces no
queda nada por hacer.

## Un stack de otra procedencia

Un stack que copiaste de un log, de un informe de error o de una ejecución de CI se puede leer de la misma manera:

```bash
emulabzip symbolicate stack.txt --mapping mapping.txt
adb logcat -d | emulabzip symbolicate - --mapping mapping.txt
```

Los frames de Java se leen con un `mapping.txt` y los de JavaScript con un source map. Un `.dSYM` necesita las direcciones
y la imagen de un informe de fallo, que un stack pegado no tiene.

## Para asistentes

`get_crash` lee el fallo con lo que se guarda para su app y versión, y dice qué archivos usó y qué falta
(`symbolication`). `original: true` mantiene los nombres tal como los escribió el dispositivo. Consulta [MCP](mcp.md).
