---
source: e1dcec842375
description: Guarda una imagen de cómo debe verse una pantalla (línea base), compara y dibuja lo que cambió, y haz que CI falle cuando supera el límite.
---
# Regresión visual

<p align="center">
  <img src="images/visual-diff.png" alt="La vista Visual: una pantalla comparada con su línea base, con el área modificada dibujada en rojo" />
</p>

Guarda una imagen de cómo debe verse una pantalla (una **línea base**) y mira qué cambió cuando deja de verse así.

```bash
emulabzip visual capture home --status-bar          # save the screen as a baseline
emulabzip visual compare home --status-bar          # exit 1 when more than the limit changed; draws the difference
emulabzip visual update home --yes --status-bar     # accept the screen as the new baseline
emulabzip visual list
emulabzip visual diff a.png b.png -o diff.png       # two screenshot files
emulabzip visual session a.ezsession b.ezsession -o diffs/    # the screenshots of two recordings, in order
```

Las líneas base son archivos PNG en la carpeta `visual/` del proyecto (o `[visual] baseline_dir`), con nombres de letras, dígitos,
`.`, `_` y `-`. Una línea base **nunca se reemplaza por accidente**: `capture` rechaza un nombre que ya existe, y `update` necesita
`--yes`.

## Qué cuenta como cambio

Cada píxel se compara con su línea base, canal por canal. Un píxel cuenta como cambiado cuando un canal difiere en más
que la **tolerancia** (8 de 255 por defecto, que absorbe el antialiasing y el redondeo del codificador). La comparación informa de
cuántos píxeles cambiaron, el porcentaje de la pantalla y el rectángulo más pequeño que contiene todos los cambios. La diferencia
se dibuja como un PNG, con los píxeles cambiados en rojo sobre una copia desvaída de la pantalla. El límite es un porcentaje
(`--threshold`, 0,1 por defecto).

Las pantallas de distinto tamaño no se comparan: vienen de dispositivos, orientaciones o tamaños de pantalla distintos, y el
error lo dice.

## El reloj y la batería

La barra de estado cambia cada minuto. `--status-bar` muestra una limpia para la captura (9:41, batería y señal completas) y
restaura la real después, de modo que esos píxeles son iguales cada vez. Todo lo demás que se mueve (un anuncio, un video, una
fecha) puede dejarse fuera de la comparación con regiones `ignore`.

## En `emulabzip.toml`

```toml
[visual]
threshold_percent = 0.1          # how much of the screen may change
tolerance = 8                    # how far a colour channel may differ before a pixel counts
baseline_dir = "ui/baselines"    # relative to the project; default "visual"

[[visual.ignore]]                # parts of the screen to leave out, in pixels
name = "clock"
x = 0
y = 0
width = 1080
height = 120
```

## En `ci`

```bash
emulabzip ci -o run.ezsession --visual home --fail-on-visual -- ./gradlew connectedCheck
```

Después del comando, la pantalla se compara con la línea base `home`, el resultado va al resumen en Markdown, y con
`--fail-on-visual` la ejecución falla si supera el límite. La línea base se lee antes de que se ejecute nada.

## Entre dispositivos

Compara dos capturas de dos dispositivos con `visual diff`, o captura en uno y usa `compare -d <other>` en otro. Deben
tener el mismo tamaño en píxeles.

## En el escritorio

La vista **Visual** lista las líneas base (guardadas en la carpeta `visual` propia de EmuLabZip), guarda la pantalla como una, la compara y
muestra la diferencia, la línea base o la pantalla actual. **Update** pregunta antes de reemplazar una imagen.

## Para asistentes

Por [MCP](mcp.md), `visual_compare` devuelve los números y la imagen de la diferencia. No crea ni reemplaza una
línea base.

## Límites

- Una línea base es una captura de pantalla, así que puede contener cualquier cosa que hubiera en la pantalla. No subas al repositorio una de una pantalla con datos
  personales.
- Compara píxeles, no significado: un desplazamiento de un píxel de todo cambia todo. Usa la tolerancia y el límite
  a tu gusto, y mantén limpia la barra de estado.
