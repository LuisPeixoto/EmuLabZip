---
source: 3fbc535a9ed8
description: Cómo EmuLabZip inspecciona el iOS Simulator con xcrun simctl, qué datos recoge, cómo manejar el log unificado y las limitaciones actuales.
---
# iOS Simulator

El iOS Simulator es compatible en macOS con Xcode instalado. En Windows y Linux el backend de iOS se declara no
disponible y el resto de EmuLabZip funciona con normalidad.

## Cómo se conecta

EmuLabZip usa la herramienta documentada `xcrun simctl`. Los argumentos se pasan directamente al proceso, nunca a través de
una shell. Instruments usa frameworks privados para hablar con los simuladores; EmuLabZip deliberadamente no lo hace, así
que sigue funcionando entre versiones de Xcode. Consulta [Cómo funcionan las herramientas oficiales](research/how-the-official-tools-work.md).

## Qué se recoge

| Función | Fuente |
|---------|--------|
| Lista de dispositivos | `simctl list devices -j`, solo simuladores de iOS arrancados |
| Overview | El JSON del simulador, `simctl list runtimes -j`, `defaults read -g AppleLocale` / `AppleLanguages` dentro del simulador, `sw_vers` y `xcodebuild -version` en el host |
| Apps | `simctl listapps` (convertido con `plutil`), ids de proceso de `launchctl list` dentro del simulador |
| Logs | `simctl spawn <udid> log stream --style ndjson --level debug` |
| Capturas de pantalla | `simctl io <udid> screenshot` |

## El log unificado está muy cargado

Un simulador escribe con facilidad varios miles de líneas por segundo en el nivel debug. EmuLabZip agrupa las líneas cada
100 ms y mantiene un búfer acotado (50,000 líneas de forma predeterminada), así que la interfaz sigue respondiendo. Para
ver menos:

- Elige tu app y activa el filtro de app en Logs.
- Sube el nivel mínimo a **I** o **W**.
- Desactiva *Verbose and debug lines* en Settings para pedirle a `log stream` solo info y superiores.

Las líneas de log de iOS llevan el **nombre del proceso** como etiqueta y el **subsistema y la categoría** (por ejemplo
`com.apple.network:connection`), que puedes ver al hacer clic en una línea.

## Limitaciones

- `log stream` no tiene historial, así que el log empieza cuando se selecciona el dispositivo.
- Los dispositivos iOS físicos aún no son compatibles (consulta la [hoja de ruta](roadmap.md)).
