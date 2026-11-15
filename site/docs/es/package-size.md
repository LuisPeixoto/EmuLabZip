---
source: 11355a8d3815
description: Desmonta un APK, AAB o IPA para ver dónde están sus bytes, compara dos builds y haz que CI falle con un presupuesto de tamaño en emulabzip.toml.
---
# Tamaño del build

<p align="center">
  <img src="images/size-diff.png" alt="La vista Build size comparando dos builds: qué creció, por tipo y por archivo" />
</p>

`emulabzip size` desmonta un APK, un AAB o un IPA y dice dónde están sus bytes, `emulabzip size diff` dice qué
cambió entre dos builds, y un **presupuesto de tamaño** en `emulabzip.toml` hace fallar CI cuando un build crece demasiado. Lee el
archivo en este ordenador: sin dispositivo, sin herramientas del SDK, sin Xcode.

```bash
emulabzip size show app-release.apk
emulabzip size show app-release.aab --json
emulabzip size diff app-1.4.apk app-1.5.apk
emulabzip size diff app-1.4.apk app-1.5.apk --fail-on-growth 5      # exit 1 over 5% growth
emulabzip size show app-release.apk --budget --baseline app-1.4.apk # hold it to [size] in emulabzip.toml
```

## Qué obtienes

- El tamaño de descarga (comprimido, tal como se guarda en el archivo) y el tamaño instalado, por **tipo**: `dex` (código compilado),
  `native` (`lib/<abi>/*.so`, y en iOS el ejecutable y `Frameworks/`), `resources`, `assets`, `manifest`, `signing`
  y `other`.
- El código nativo **por ABI** (`arm64-v8a`, `x86_64`…), de modo que un build que incluye cuatro de ellas se ve con claridad. En un IPA,
  `Frameworks` y el ejecutable.
- Cada **archivo dex** con sus clases y métodos, leídos de su cabecera, y el total frente a los 65.536 métodos que cabe en un
  solo archivo dex.
- Los **módulos** de un AAB (`base` y los módulos de funciones).
- Los archivos más grandes.
- Con `diff`: el total y el porcentaje, el cambio por tipo y por carpeta, y los archivos más grandes **añadidos, eliminados y
  modificados**.

Que un archivo sea un APK, un AAB o un IPA se decide por lo que contiene, no por su nombre.

## El presupuesto

```toml
[size]
max_total_mb = 25          # the download size, compressed
max_dex_mb = 8
max_native_mb = 12
max_resources_mb = 6
max_methods = 60000        # all dex files together
max_growth_percent = 5     # against a baseline build (--baseline, or ci --size-baseline)
```

Los tamaños son megabytes (1.048.576 bytes) de tamaño **comprimido**. Todas las claves son opcionales, una clave desconocida es un error, y un
límite de crecimiento sin build de línea base se informa en lugar de darse por pasado. En CI:

```bash
emulabzip ci -o run.ezsession --size app-release.apk --size-baseline last-release.apk -- ./gradlew connectedCheck
```

El build se lee antes de que se ejecute nada, así que un archivo que falta no cuesta una ejecución de pruebas. Consulta [Presupuestos](budgets.md).

## En el escritorio

La vista **Build size** abre un build con un diálogo de archivo (no hace falta dispositivo), muestra las barras por tipo, el código nativo,
los archivos dex y los archivos más grandes, y **Compare with…** un segundo build.

## Límites

- No abre el manifest compilado ni el bytecode dex: solo tamaños, carpetas y la cabecera dex. Para un desglose
  a nivel de clase usa el APK Analyzer de Android Studio.
- Todavía no lee la app instalada desde un dispositivo: extrae primero el archivo (`adb shell pm path`, `adb pull`).
- Por MCP (`analyze_package`, `diff_packages`) un archivo solo se acepta con extensión `.apk`, `.aab` o `.ipa`.
