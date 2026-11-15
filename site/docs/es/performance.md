---
source: 98d4164fe240
description: La herramienta Performance muestra la CPU, la memoria, los fotogramas y la batería de la app y del dispositivo en gráficos en vivo de los últimos dos minutos.
---
# Rendimiento

La herramienta Performance muestrea la app seleccionada y el dispositivo mientras la herramienta está abierta, y dibuja
los últimos dos minutos como gráficos en vivo. Elige la app en la barra de herramientas; sin una, solo se muestran los
valores de todo el dispositivo.

## Qué se mide

| Valor | Android | Simulador de iOS |
|-------|---------|------------------|
| CPU de la app | Tiempo de usuario + sistema de `/proc/<pid>/stat`, como parte de todos los núcleos | Tiempo de CPU del proceso en el host (`ps`), como parte de todos los núcleos del Mac |
| CPU del dispositivo | `/proc/stat`, tiempo ocupado en todos los núcleos | — (el simulador comparte la CPU del Mac) |
| Memoria de la app | Resident set size (`VmRSS`) | Resident set size del proceso en el host |
| Memoria del dispositivo | `MemTotal − MemAvailable` de `/proc/meminfo` | — |
| Frame rate | Fotogramas renderizados por segundo, de `dumpsys gfxinfo` | Phase 4, con trazas de Instruments |
| Fotogramas con jank | Proporción de fotogramas con jank desde la muestra anterior | Phase 4 |
| Batería | Nivel y temperatura de `dumpsys battery` | — |

Cada muestra de Android es **una** llamada a `adb shell` que ejecuta todos los comandos a la vez, así que muestrear cada
500 ms sigue siendo barato. El id de proceso de la app se busca en cada muestra: cuando la app se reinicia, los gráficos
continúan con el nuevo proceso.

## Cómo leer los gráficos

- Los porcentajes de CPU son de **todo el dispositivo**, como los muestra Android Studio. En un emulador de 4 núcleos, un
  núcleo ocupado es el 25 %.
- El frame rate solo se mueve mientras la app dibuja. Una pantalla estática no renderiza fotogramas, y eso es bueno.
- Los fotogramas con jank son los que no cumplieron su plazo, según los cuenta el `gfxinfo` de la plataforma.
- Pasa el cursor sobre un gráfico para leer los valores exactos. Los gráficos con una sola serie no tienen leyenda; el título
  nombra el valor.

## Controles

| Control | Efecto |
|---------|--------|
| **0.5 s · 1 s · 2 s** | Tiempo entre muestras. |
| **Pause** | Detiene el muestreo; los gráficos conservan lo que tienen. |
| **Clear** (papelera) | Vacía los gráficos. |

El muestreo se detiene cuando sales de la herramienta, así que nunca se ejecuta en segundo plano.

Debajo de los gráficos hay dos tarjetas que miden algo bajo petición: **Start time** ([guía](startup.md)) y **Energy**
([guía](energy.md)). Ambas cambian el dispositivo mientras se ejecutan (la app se detiene y se abre; la batería se
desconecta a ojos del servicio de batería) y lo dejan como estaba.

## Desde la línea de comandos

```bash
emulabzip perf --app com.example.app
emulabzip perf --app com.example.app --interval 500 --json | jq .appCpu
```
