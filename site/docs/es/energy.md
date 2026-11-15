---
source: a19d308974b9
description: "Mide cuánta energía gasta una app durante un periodo de uso: estimación en mAh y wake locks en Android, y tiempo de CPU y estado térmico en iOS."
---
# Energía

Cuánta energía usa una app durante un periodo de tiempo, mientras la usas.

```bash
emulabzip energy com.example.app --seconds 30
emulabzip energy com.example.app --json
```

Empieza, usa la app, y lee el resultado cuando se acaba el tiempo o cuando pulsas <kbd>Ctrl+C</kbd>.

## Android

El dispositivo se desconecta a ojos del servicio de batería (`dumpsys battery unplug`) y las estadísticas se reinician
(`dumpsys batterystats --reset`), para que se cuenten en el dispositivo. Al final se lee `dumpsys batterystats --charged
<package>` para el uid de la app, y la batería se restaura (`dumpsys battery reset`) **siempre**, también después de un
error o de <kbd>Ctrl+C</kbd>.

El informe tiene:

- el **uso estimado** del sistema en mAh, y las partes de las que viene (CPU, radio, pantalla, sensores…);
- el **tiempo de procesador** que usó la app;
- **wake locks** (cuánto tiempo se mantuvo cada uno y cuántas veces), **alarms** y **jobs**;
- los bytes de **red** recibidos y enviados.

Un emulador no tiene batería. Android calcula estos números a partir de un modelo de consumo, así que compara una
ejecución con otra, no con un teléfono. Una app que usó muy poco en ese tiempo, o que no estaba en ejecución, no recibe
estimación, y el informe lo dice.

## Simulador de iOS

El simulador no tiene un modelo de consumo, así que no hay cifra en mAh. El informe da el **tiempo de CPU** que usó la app
(lo que primero agota una batería real) y el **estado térmico** del Mac (`pmset -g therm`): cuando el Mac reduce la velocidad de su
procesador, dos ejecuciones no son comparables.

## Dónde encontrarlo

- **Escritorio**: Performance → **Energy** (Start, luego Stop y leer).
- **CLI**: `emulabzip energy`.
- **MCP**: `measure_energy` (como máximo 120 segundos).
