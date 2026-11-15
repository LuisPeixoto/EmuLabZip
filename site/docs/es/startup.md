---
source: fb1852db95e8
description: Mide cuánto tarda en abrirse una app, siempre de la misma forma y repetido varias veces para que la mediana signifique algo, en Android y en iOS.
---
# Tiempo de arranque de la app

Cuánto tarda en abrirse una app, medido siempre de la misma forma y repetido unas cuantas veces para que la mediana
signifique algo.

```bash
emulabzip startup com.example.app                 # cold, 5 runs
emulabzip startup com.example.app --kind hot --runs 10
emulabzip startup com.example.app --json
```

| Tipo | Android | Simulador de iOS |
|------|---------|------------------|
| **cold** | El proceso se detiene primero (`am force-stop`), así que el sistema lo inicia desde cero | La app se termina primero |
| **warm** | La pantalla se cierra con el botón atrás y el proceso se conserva | No disponible: el simulador no tiene botón atrás, así que no hay un arranque warm separado |
| **hot** | La app se envía a segundo plano con el botón de inicio | La app Settings se pone delante |

## Qué se mide

- **Android** usa `am start -W`, que informa `TotalTime` (el tiempo hasta que se dibuja el primer fotograma), `WaitTime` y el
  `LaunchState`. La activity de lanzamiento se encuentra con `cmd package resolve-activity`.
- **El simulador de iOS** lee el log unificado. SpringBoard registra cuándo se le pide abrir una app y otra vez cuando la
  escena de la app está activa. El tiempo entre ambos es lo que espera una persona, animación incluida. Compara las
  ejecuciones entre sí, no con un teléfono.

El resultado lista cada ejecución y la mediana, la más rápida, la más lenta y el percentil 90. La mediana es el número que
hay que seguir; una ejecución lenta suele ser ruido.

## Dónde encontrarlo

- **Escritorio**: Performance → **Start time** (tipo, número de ejecuciones, una barra por ejecución).
- **CLI**: `emulabzip startup`.
- **MCP**: `measure_startup`.
- **CI**: con una tabla `[startup]` en [`emulabzip.toml`](budgets.md), `emulabzip ci --app …` mide antes del comando
  y somete la mediana a `start_time_ms`.

La app se detiene y se abre en el dispositivo mientras esto se ejecuta, así que no lo ejecutes en un dispositivo donde eso te cueste trabajo.
