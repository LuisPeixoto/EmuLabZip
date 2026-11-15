---
source: 685bcdfe2f24
description: Cambia el idioma y el tamaño de texto del dispositivo, comprueba si el texto no cabe en pantalla y barre una pantalla por varios idiomas y tamaños.
---
# Idioma, tamaño de texto y texto que no cabe

<p align="center">
  <img src="images/audit-text.png" alt="La vista Audit encontrando una etiqueta demasiado larga para su línea" />
</p>

El texto es donde se rompen los diseños: una palabra alemana es más larga, el árabe va de derecha a izquierda, una persona que necesita texto grande lo pone al
200%. EmuLabZip puede **cambiar** el idioma y el tamaño de texto de un dispositivo, **comprobar** si la pantalla tiene texto que no cabe, y
**barrer** una pantalla por varios idiomas y tamaños con un solo comando.

## Cambiar el dispositivo

```bash
emulabzip control locale en-XA --app com.example.shop   # long, accented text
emulabzip control locale ar-XB --app com.example.shop   # right to left
emulabzip control reset-locale --app com.example.shop
emulabzip control font-scale 130                        # percent of normal; 100 puts it back
emulabzip control display --width 720 --height 1600 --density 320     # Android
emulabzip control reset-display
```

| | Android | iOS Simulator |
|---|---------|---------------|
| Idioma | `cmd locale set-app-locales`: **una app a la vez**, Android 13 y posterior. Los pseudo-locales `en-XA` y `ar-XB` funcionan. Se avisa a la app y esta recrea su pantalla | `defaults write -g AppleLanguages` y `AppleLocale` para **todo el simulador**; una app lo toma **cuando se inicia de nuevo** |
| Tamaño de texto | `settings put system font_scale` | `simctl ui … content_size`: el tamaño de Dynamic Type más cercano al porcentaje (130% es `extra-extra-extra-large`, 200% `accessibility-extra-large`) |
| Tamaño y densidad de pantalla | `wm size`, `wm density` | No disponible |

Una etiqueta de idioma son letras, dígitos y guiones (`de-DE`, `pt-BR`, `zh-Hans-CN`). El tamaño de texto va del 50 al 300 por ciento. Todo
esto también está en la vista **Controls**, bajo *Text size and language* y *Display size*.

## Comprobar una pantalla

```bash
emulabzip i18n check            # the screen as it is now
emulabzip i18n check --rtl      # it is shown in a right-to-left language: also check the layout was mirrored
```

| Regla | Busca |
|------|-------|
| `text-off-screen` | Texto que sobrepasa el borde de la pantalla (salvo que esté dentro de algo que se desplaza) |
| `text-clipped-by-parent` | Texto que no cabe dentro de su contenedor, de modo que el contenedor lo corta |
| `text-cut-off` | Una sola línea de texto limitada al espacio que se le dio, cuya tinta llega al final de su caja: probablemente está cortada o termina en puntos suspensivos |
| `rtl-not-mirrored` | En un idioma de derecha a izquierda, la mayor parte del texto de la pantalla sigue a la izquierda |

`text-cut-off` es una heurística. El diseño dice dónde está cada vista, y la captura dice si su texto llega al final de
la caja. Una etiqueta que simplemente se ajusta a su texto no se informa (su caja es solo tan ancha como el texto), y una etiqueta solo se
informa cuando su caja termina donde su contenedor, o la siguiente vista de su fila, no deja espacio. El mensaje dice "may be".
Sin una captura de pantalla solo se ejecutan las reglas de diseño.

## Barrer

```bash
emulabzip i18n sweep --app com.example.shop --locales en-XA,ar-XB --scales 100,130,200
```

El barrido prueba cada idioma en cada tamaño de texto, lee el diseño e imprime una tabla: cuántos hallazgos, cuáles y
**cuáles son peores que el dispositivo tal como está ahora**. El dispositivo se restaura (tamaño de texto e idioma) cuando termina, y
también cuando un paso falla. `--fail-on-worse` sale con 1 cuando algún ajuste es peor. En el iOS Simulator solo se barren los tamaños de
texto, porque una app solo toma un idioma nuevo cuando se inicia de nuevo.

El escalado de texto también se revisa mediante el barrido: al 200% informa de áreas táctiles y orden de foco que empeoraron.

## Para asistentes

Por [MCP](mcp.md): `control_locale`, `control_font_scale`, `control_display_size` y `check_text`.

## Límites

- Android cambia el idioma de una app, no del dispositivo. Los idiomas de todo el dispositivo necesitan root.
- Las heurísticas ven vistas, no glifos. Una fuente de iconos, una etiqueta dibujada a medida o el texto de un juego no se juzgan.
- Los pseudo-locales dependen de que la app tenga sus cadenas en recursos; el texto escrito directamente en el código se queda como está.
