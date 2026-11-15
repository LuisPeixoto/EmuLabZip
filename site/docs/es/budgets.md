---
source: 4044d9ed6c76
description: Define en emulabzip.toml lo que puede costar una ejecución y deja que emulabzip ci falle si cuesta más, con presupuestos y comparación con una línea base.
---
# Presupuestos y líneas base

Escribe lo que una ejecución puede costar, y `emulabzip ci` hace fallar la ejecución cuando cuesta más.

## `emulabzip.toml`

Ponlo en la carpeta del proyecto (o pasa `--config`). Cada límite es opcional; una ejecución solo se somete a los que
están escritos.

```toml
[budget]
start_time_ms = 800        # the median start time; needs [startup]
memory_peak_mb = 300
cpu_avg_percent = 40
janky_frames_percent = 5
requests = 200
failed_requests = 0
log_errors = 10
crashes = 0

[startup]                  # optional: measure the start time before the command runs
runs = 5
kind = "cold"              # cold, warm or hot

[baseline]
tolerance_percent = 10     # how much worse than the baseline counts as a regression

[size]                     # a build's size: see Build size (package-size.md)
max_total_mb = 25
max_dex_mb = 8
max_native_mb = 12
max_methods = 60000
max_growth_percent = 5

[visual]                   # screenshots: see Visual regression (visual-regression.md)
threshold_percent = 0.1
tolerance = 8
```

`[size]` se comprueba sobre un archivo de build con `emulabzip size show --budget` y `emulabzip ci --size`; `[visual]` define
cómo compara `emulabzip visual` y qué partes de la pantalla ignora. Consulta [Tamaño del build](package-size.md) y
[Regresión visual](visual-regression.md).

Una clave que no existe es un **error**, así que una errata no deja un límite desactivado. Un límite que no se pudo
medir cuenta como incumplido: `start_time_ms` sin una tabla `[startup]` y sin `--app` falla, en lugar de pasar en silencio.

## Presupuestos en `ci`

```bash
emulabzip ci -o run.ezsession --app com.example.app -- ./gradlew connectedCheck
```

Después de la ejecución, los límites se comprueban y se listan en el informe Markdown bajo **Budgets**. Si uno se
incumple, el código de salida es 1, salvo que el propio comando haya fallado, en cuyo caso prevalece su código.

## Líneas base

Compara una ejecución con una sesión anterior, normalmente la última buena:

```bash
emulabzip ci -o run.ezsession --app com.example.app --baseline last-good.ezsession --fail-on-regression -- ./test.sh
emulabzip session compare last-good.ezsession run.ezsession
```

Un número es **peor** cuando creció más que la tolerancia (10 % por defecto, `--tolerance` en `session compare`). El
informe lista cada número como mejor, peor o igual. Sin `--fail-on-regression`, una regresión solo se informa.

En GitHub Actions, guarda la última sesión buena como caché o como artefacto de la rama por defecto, y descárgala antes de
la ejecución:

```yaml
- uses: actions/cache@v4
  with:
    path: baseline.ezsession
    key: baseline-${{ github.base_ref || github.ref_name }}
    restore-keys: baseline-
- run: >
    emulabzip ci -o run.ezsession --app com.example.app
    --baseline baseline.ezsession --fail-on-regression -- ./gradlew connectedCheck
- if: github.ref_name == 'main'
  run: cp run.ezsession baseline.ezsession
```

El tiempo de arranque se guarda en la sesión (`startup.json`), así que también se puede comparar. Las sesiones creadas antes
de esto siguen funcionando: el archivo es opcional y la versión del formato sigue siendo 1.
