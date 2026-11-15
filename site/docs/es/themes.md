---
source: ae0f1b688c5b
description: Usa uno de los diez temas de EmuLabZip, crea el tuyo en el editor, impórtalo o expórtalo como JSON y compártelo en la galería de la comunidad.
---
# Themes

EmuLabZip incluye diez temas, y puedes crear los tuyos, importar el de otra persona y compartir el tuyo en la galería de la
comunidad. Abre **Settings** (⌘,) y ve a **Appearance**.

![Los ajustes de Appearance con una cuadrícula de tarjetas de temas](images/themes.png)

## Usar un tema

La app guarda un tema para el modo oscuro y otro para el claro. Elegir un tema fija el de su base, y el interruptor **Mode**
(dark, light o system) elige cuál se muestra. El botón de sol y luna de la barra de herramientas alterna entre los dos.

Los temas incluidos:

| Tema | Base | |
| --- | --- | --- |
| **Midnight** | oscuro | El predeterminado: un oscuro más claro que se lee mejor. |
| **Classic Dark** | oscuro | El tema casi negro de versiones anteriores. |
| **Graphite**, **Fjord**, **Ember**, **Forest** | oscuro | Gris neutro, azul frío, cálido y verde. |
| **Daylight** | claro | El tema claro predeterminado. |
| **Paper** | claro | Un claro más suave y cálido. |
| **High Contrast Dark** y **Light** | oscuro, claro | Contraste AAA para el texto. |

## Crear un tema

**New theme** abre el editor, y **Duplicate** en cualquier tarjeta parte de ese tema. Toda la app muestra tus cambios mientras
editas; **Cancel** devuelve el tema elegido.

- **Start from one colour** rellena las superficies y el texto a partir de un acento, con la luminosidad que necesita la base.
- Cada color tiene un selector y un campo de texto. Los colores son `#rgb`, `#rrggbb`, `#rrggbbaa`, `rgb()` o `hsl()`.
- **Fonts** son nombres de familia, separados por comas. Deben estar instaladas en el equipo; no se descarga nada.
- **Roundness** va de cuadrado (0) a muy redondeado (2).
- El editor avisa cuando el texto sería difícil de leer: comprueba el contraste WCAG de los pares principales (texto sobre la ventana y
  sobre los paneles, texto secundario, pistas, acento, errores, advertencias).

Debajo de las tarjetas, **Fine-tune** cambia cosas que son tuyas y no forman parte de ningún tema: tamaño de texto (85% a 140%), escala de la
interfaz, densidad, redondez y fuentes. **Reset the fine-tuning** las devuelve a su valor.

![El editor de temas, con una advertencia de contraste](images/theme-editor.png)

## El formato de archivo

Un tema es un único archivo JSON de unos cientos de bytes de colores, y de como máximo 64 KB:

```json
{
  "schema": 1,
  "id": "aurora",
  "name": "Aurora",
  "author": "EmuLabZip",
  "description": "Violet accents on a deep, cool grey.",
  "base": "dark",
  "colors": { "bg": "#1d1a23", "panel": "#24212c", "fg": "#f1eff7", "accent": "#8b5cf6" },
  "fonts": { "sans": "Inter, system-ui, sans-serif", "mono": "Menlo, monospace" },
  "shape": { "radius": 1 }
}
```

Los nombres de color son los grupos del editor: superficies (`bg`, `panel`, `panel-2`, `elevated`, `hover`, `line`,
`line-strong`), texto (`fg`, `muted`, `faint`), acento (`accent`, `accent-2`, `accent-soft`, `select`), estado (`ok`, `warn`,
`err`, `info`), gráficos (`series-1`, `series-2`, `flame-app`, `flame-framework`, `flame-native`, `flame-kernel`) y niveles de
log (`lv-verbose`, `lv-debug`, `lv-info`, `lv-warn`, `lv-error`, `lv-fatal`). Un color que omitas se toma del
tema predeterminado de la `base`.

### Por qué es seguro abrir un tema

Un tema puede venir de cualquiera, así que se comprueba antes de usarlo:

- un color debe ser un color hexadecimal o `rgb()`/`hsl()` con números, y nada más;
- una fuente es una lista de nombres de familia. `url(`, llaves y puntos y coma se rechazan, así que un tema no puede llevar CSS ni cargar un archivo;
- las claves desconocidas se descartan, y el archivo no puede superar los 64 KB;
- la política de seguridad de contenido de la app no permite nada remoto excepto la dirección propia de la galería.

## Importar y exportar

**Export** en una tarjeta guarda un tema como `<id>.json`. **Import file** lee uno, y **Paste JSON** toma el texto. El tema
se muestra primero en una tarjeta, y solo se instala cuando pulsas Install.

## La galería de la comunidad

La pestaña **Gallery** lista los temas compartidos por otras personas, leídos de
`https://emulabzip.luispeixoto.dev/themes/index.json`. Instala uno y es tuyo para usarlo y editarlo. Sin conexión, la pestaña lo
indica y todo lo demás sigue funcionando.

![La pestaña Gallery con temas de la comunidad](images/theme-gallery.png)

### Comparte tu tema

1. Da al tema un autor y una descripción en el editor.
2. En **Gallery**, elige el tema en **Share a theme** y pulsa **Open on GitHub**.
3. GitHub abre un archivo nuevo, `site/static/themes/<id>.json`, con tu tema dentro. Inicia sesión y pulsa **Propose new file**;
   se convierte en un pull request.

También puedes añadir el archivo a mano en `site/static/themes/` en un pull request. El build del sitio comprueba cada archivo con las
mismas reglas que la app (`node site/build.mjs --check`), y un pull request que no pasa la comprobación no se puede fusionar:

- el archivo se llama como su `id`, que debe ser letras minúsculas, dígitos y guiones, y no el de un tema incluido;
- tiene un `author` y una `description`;
- cada color y cada fuente pasan las comprobaciones anteriores;
- el texto es legible: al menos 4.5:1 para el texto y el texto secundario sobre la ventana y sobre los paneles.

El índice que lee la app lo genera el build a partir de estos archivos, así que no hay nada más que editar.
