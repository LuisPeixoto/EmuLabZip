---
source: 9f0d25bee3d4
description: Graba una sesión alrededor de cualquier comando, normalmente una ejecución de pruebas, y falla el pipeline por fallos, regresiones, presupuestos o auditorías.
---
# Modo CI

`emulabzip ci` graba una [sesión](sessions.md) alrededor de cualquier comando, normalmente una ejecución de pruebas, y después termina con el
código de salida del propio comando, de modo que tu pipeline se comporta como antes, con una grabación que mirar cuando algo sale mal.

```bash
emulabzip ci -o run.ezsession --app com.example.app -- ./gradlew connectedCheck
```

Empieza a grabar, ejecuta el comando (su salida pasa sin cambios), se detiene, escribe la sesión e imprime un
resultado de una línea.

| Opción | |
|--------|---|
| `-o, --output FILE` | Dónde escribir la sesión (obligatorio) |
| `-a, --app ID` | La app: se toman muestras para ella y los fallos se limitan a ella |
| `--network` | Graba el tráfico HTTP(S): el dispositivo apunta al proxy durante la ejecución |
| `--screens N` | Una captura de pantalla cada N segundos |
| `--fail-on-crash` | Hace fallar la ejecución (salida 1) cuando la app falló mientras se ejecutaba, aunque el comando pasara |
| `--summary FILE` | Escribe también el informe, en Markdown, en este archivo |
| `--config FILE` | Límites a los que ceñir la ejecución (por defecto: `./emulabzip.toml`, cuando existe). Consulta [Presupuestos](budgets.md) |
| `--baseline FILE` | Una sesión anterior con la que comparar |
| `--fail-on-regression` | Falla cuando un número empeoró respecto a la línea base en más que la tolerancia |
| `--video [N]` | Graba la pantalla, en fragmentos de N segundos (por defecto 30). Consulta [Línea de tiempo y video de pantalla](timeline.md) |
| `--taps` | Graba los toques (Android) |
| `--replay FILE` | Responde las peticiones de la app desde un archivo HAR o una sesión, para que la ejecución no dependa de ningún servidor. Necesita `--network`. Consulta [Repetir una grabación](replay.md) |
| `--passthrough` | Con `--replay`: deja que las peticiones que no están en la grabación lleguen al servidor real |
| `--fail-on-miss` | Con `--replay`: hace fallar la ejecución cuando la app pidió algo que la grabación no contiene |
| `--size BUILD` | Ciñe un APK, AAB o IPA a los límites `[size]` de `emulabzip.toml`. Se lee antes de la ejecución. Consulta [Tamaño del build](package-size.md) |
| `--size-baseline BUILD` | Con `--size`: un build anterior, para medir el crecimiento |
| `--fail-on-security LEVEL` | Audita la seguridad y privacidad de la app después del comando (necesita `--app`; se incluyen las peticiones del proxy) y falla ante un hallazgo al menos así de grave: `note`, `warning` o `error`. Consulta [Auditoría de seguridad](security-audit.md) |
| `--visual NAME` | Compara la pantalla después del comando con una línea base. Consulta [Regresión visual](visual-regression.md) |
| `--fail-on-visual` | Con `--visual`: hace fallar la ejecución cuando la pantalla difiere más que el límite |

El código de salida es el del comando. Si el comando pasó pero `--fail-on-crash` encontró un fallo, se incumple un [presupuesto](budgets.md) o se pidió que una regresión hiciera fallar la ejecución, es 1; si pulsas Ctrl+C, es 130.

Cuando la variable de entorno `GITHUB_STEP_SUMMARY` está definida (GitHub Actions), el informe en Markdown se añade a ella
y aparece en la página de la ejecución sin pasos adicionales.

## GitHub Actions

```yaml
name: UI tests
on: [pull_request]

jobs:
  android:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-java@v4
        with: { distribution: temurin, java-version: 17 }
      - name: Install EmuLabZip
        run: cargo install --git https://github.com/LuisPeixoto/EmuLabZip emulabzip-cli
      - uses: reactivecircus/android-emulator-runner@v2
        with:
          api-level: 34
          arch: x86_64
          script: >
            emulabzip ci -o run.ezsession --app com.example.app --fail-on-crash --screens 10
            -- ./gradlew connectedCheck
      - uses: actions/upload-artifact@v4
        if: always()
        with:
          name: session
          path: run.ezsession
```

Descarga el artefacto y ábrelo en **Sessions → Open…**, o léelo con `emulabzip session show`.

También puedes exigir accesibilidad en el mismo job, cuando la app esté en la pantalla que debe revisarse:

```bash
emulabzip a11y --fail-on error
```

## Notas

- El emulador debe estar en ejecución antes de que empiece `ci`; con más de un dispositivo, pasa `--device`.
- Un archivo de sesión que ya existe nunca se sobrescribe.
- Si se usa `--network`, el dispositivo debe confiar en el certificado de EmuLabZip (consulta [Network](network.md#the-certificate)).
  En CI ejecuta `emulabzip ca install` una vez, en una imagen con acceso root.
