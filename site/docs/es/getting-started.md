---
source: 76f23448cd4b
description: "Guía de inicio de EmuLabZip: instala la app, prepara adb o Xcode, arranca un dispositivo y empieza a leer los logs de tu app en un par de minutos."
---
# Primeros pasos

Esta guía te lleva desde una instalación nueva hasta leer los logs de tu app en un par de minutos.

## 1. Instala EmuLabZip

Descarga el instalador para tu sistema desde la [última versión](https://github.com/LuisPeixoto/EmuLabZip/releases),
o [compila desde el código fuente](../README.md#build-from-source).

## 2. Ten listas las herramientas de la plataforma

| Para inspeccionar | Necesitas |
|------------|----------|
| Emuladores y dispositivos Android | Las **platform tools** del Android SDK (`adb`). Android Studio las instala. |
| Simuladores de iOS | Un Mac con **Xcode** y sus herramientas de línea de comandos (`xcode-select --install`). |

EmuLabZip encuentra `adb` en tu `PATH`, en `$ANDROID_HOME` / `$ANDROID_SDK_ROOT` o en la ubicación predeterminada del SDK
(`~/Library/Android/sdk`, `%LOCALAPPDATA%\Android\Sdk`, `~/Android/Sdk`). Inicia el servidor ADB si no está en
ejecución. Consulta [Android](android.md) y [iOS Simulator](ios-simulator.md) para más detalles.

## 3. Arranca un dispositivo

```bash
# Android
emulator -list-avds
emulator -avd Pixel_9

# iOS
xcrun simctl boot "iPhone 17"
open -a Simulator
```

Abre EmuLabZip. Los dispositivos en ejecución aparecen en el **device picker** (selector de dispositivo), en la esquina
superior izquierda, en unos segundos, y el primero que esté listo se selecciona automáticamente.

## 4. Echa un vistazo

- **Overview** muestra qué es el dispositivo: sistema y build, hardware, pantalla, batería, almacenamiento e idioma, con
  una vista previa en vivo de la pantalla. Haz clic en cualquier valor para copiarlo.
- **Logs** sigue el log del dispositivo desde el momento en que lo seleccionas, así que el historial ya está ahí cuando
  lo abres. Consulta [Logs](logs.md).
- **Performance** muestra en vivo gráficas de CPU, memoria, frecuencia de fotogramas y batería. Consulta
  [Performance](performance.md).
- **Memory** toma snapshots, los compara y guarda volcados de heap. Consulta [Memory](memory.md).
- **Storage** explora los archivos de la app y abre bases de datos, preferencias y listas de propiedades. Consulta
  [Storage](storage.md).
- **Network** captura el tráfico HTTP y HTTPS del dispositivo. Consulta [Network](network.md).
- **Traces** graba lo que hicieron el sistema y una app, y muestra dónde se fue el tiempo. Consulta [Traces](traces.md).
- **Crashes** lista fallos, fallos nativos y ANR con sus stacks, y te avisa cuando ocurre uno nuevo. Consulta
  [Crashes](crashes.md).
- **Controls** cambia la ubicación, la apariencia, la barra de estado, la batería y los permisos del dispositivo.
  Consulta [Controls](controls.md).
- **Plugins** ejecuta tus propios colectores. Consulta [Plugins](plugins.md). Para que un asistente de IA use tus
  dispositivos, consulta el [servidor MCP](mcp.md).
- **Screenshot** en la barra de herramientas captura la pantalla y te deja guardarla como PNG.

## 5. Concéntrate en tu app

Elige tu app en el **app picker** (selector de app), junto al dispositivo. Después, en Logs, activa el filtro de app para
ver solo las líneas que escribió su proceso. EmuLabZip sigue el id del proceso, así que el filtro sigue funcionando
cuando la app se reinicia.

## Atajos de teclado

| Atajo | Acción |
|----------|--------|
| <kbd>⌘1</kbd> … <kbd>⌘6</kbd> | Overview, Logs, Performance, Memory, Storage, Network |
| <kbd>⌘F</kbd> | Buscar en el log |
| <kbd>⌘K</kbd> | Borrar el log |
| <kbd>⌘R</kbd> | Buscar dispositivos ahora |
| <kbd>⌘,</kbd> | Settings |

En Windows y Linux usa <kbd>Ctrl</kbd> en lugar de <kbd>⌘</kbd>.
