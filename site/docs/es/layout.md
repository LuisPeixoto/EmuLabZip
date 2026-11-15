---
source: 0fcc75cc4e31
description: Inspecciona las vistas en pantalla con Layout, lee sus propiedades y audita la accesibilidad, con simulación de daltonismo, desde la app, la CLI o MCP.
---
# Layout y accesibilidad

**Layout** muestra las vistas de la pantalla: una captura con un recuadro alrededor de cada vista, el árbol de vistas y las
propiedades de la que elijas. El mismo árbol alimenta una **auditoría de accesibilidad** que busca aquello con lo que
tropezaría un lector de pantalla o un pulgar.

## La vista Layout

Pulsa **Capture**. La captura y el árbol de vistas se leen juntos, así que coinciden.

- Mueve el puntero sobre la captura para resaltar la vista que tiene debajo; haz clic para seleccionarla. El árbol se
  abre hasta ella.
- **Outline every view** dibuja todos los recuadros a la vez.
- Busca en el árbol por texto, etiqueta, id o clase.
- Las propiedades muestran la clase, el texto, la etiqueta, el id, la posición, el tamaño (en dp en Android) y los
  indicadores: clickable, focusable, editable, scrollable, disabled, checked.

### De dónde sale el árbol

| | Fuente |
|---|---|
| Android | `uiautomator dump`, que tiene toda imagen. Es el árbol de accesibilidad que lee TalkBack. No se instala nada. |
| iOS Simulator | La API de accesibilidad de macOS sobre la ventana del Simulator, que contiene el árbol de accesibilidad de iOS (el que recorre VoiceOver), con etiquetas e identificadores. |

iOS necesita dos cosas: la ventana del Simulator tiene que estar abierta en ese dispositivo, y el programa que ejecuta
EmuLabZip (la app, o tu terminal para la línea de comandos) debe tener permiso en **System Settings, Privacy & Security,
Accessibility**. EmuLabZip lo dice cuando no es así. Las posiciones se convierten a puntos, así que coinciden con lo que
declara la app. Los dispositivos físicos no son compatibles.

`uiautomator` no puede leer pantallas que lo prohíben (ventanas seguras, algunos reproductores de video); EmuLabZip lo
informa en lugar de mostrar un árbol vacío.

## Compose, Flutter, React Native y SwiftUI

Las vistas dibujadas por un framework de UI llevan una marca en el árbol (`compose`, `flutter`, `react-native`, `swiftui`)
y el inspector nombra el framework. En Android, todo lo que hay bajo un `ComposeView`, un `FlutterView` o una vista raíz
de React Native está marcado. En iOS, elige la app en la barra de herramientas (o pasa `--app <bundle id>` en la línea de
comandos) y la pantalla se marca con el framework de esa app. Consulta [Dentro de los frameworks](frameworks.md).

## La auditoría

La pestaña **Accessibility** lista los hallazgos, los más graves primero. Haz clic en uno para ver la vista y qué hacer al
respecto.

| Regla | Busca | WCAG |
|------|-----------|------|
| `missing-label` | Una vista que se puede tocar o en la que se puede escribir y no tiene nombre: ni texto, ni etiqueta, ni nada dentro que lo tenga | 1.1.1, 4.1.2 |
| `unlabeled-focusable` | Una vista que recibe el foco y no dice nada | 4.1.2 |
| `touch-target` | Un control menor de 48 dp (Android) o 44 pt (iOS) en cualquiera de las dos direcciones | 2.5.8 |
| `contrast` | Texto con un contraste menor de 4.5:1 (3:1 para texto grande), medido a partir de los píxeles de la captura dentro de la vista | 1.4.3 |
| `duplicate-label` | Controles vecinos anunciados de la misma manera ("Delete", "Delete", "Delete") | 2.4.6 |
| `focus-order` | Un foco que salta hacia arriba en la pantalla: el diseño lista un control antes de los que están encima (un lector de pantalla y un teclado siguen el orden del diseño). Los controles de una misma fila no son un salto; se informan como máximo cinco | 2.4.3 |

El contraste se mide tomando el color más común de la vista como fondo y el color más alejado de él como texto. Una vista
sobre una foto o un degradado no se juzga, porque no hay un único fondo. A los controles deshabilitados no se les pide
que sean grandes ni que tengan nombre.

### Daltonismo

```bash
emulabzip a11y --simulate deuteranopia -o deuteranopia.png    # protanopia, deuteranopia, tritanopia or achromatopsia
```

Muestra la pantalla como la ve alguien con ese tipo de daltonismo (las matrices de Machado, Oliveira y Fernandes, con
severidad completa, aplicadas a luz lineal), escribe la imagen y lista el **texto que se leía normalmente y ya no** (`contrast-<kind>`).
El texto que ya fallaba la regla de contraste no se repite. Por [MCP](mcp.md), `simulate_color_blindness` devuelve la
imagen y los hallazgos. La vista **Audit** tiene lo mismo en *Colour vision*.

### Tamaño de texto, idiomas y texto que no cabe

Consulta [Idioma, tamaño de texto y texto que no cabe](localization.md): `i18n check` e `i18n sweep` buscan texto
cortado, fuera de pantalla o sin reflejar, y el barrido informa de los objetivos táctiles y del orden de foco que
empeoran con texto grande.

Es una primera pasada a partir del diseño solamente. No sustituye probar la app con TalkBack o VoiceOver, y no puede
saber que una etiqueta es *incorrecta*.

## Línea de comandos y asistentes

```bash
emulabzip layout                       # the tree as text
emulabzip layout --json --png screen.png
emulabzip a11y                         # the findings
emulabzip a11y --json
emulabzip a11y --fail-on warning       # exit 1 when something at least this serious is found (note, warning, error)
```

`a11y --fail-on` hace fallar un paso de CI cuando una pantalla retrocede. El [servidor MCP](mcp.md) ofrece `get_layout` y
`audit_accessibility`, para que un asistente pueda mirar una pantalla y corregir lo que encuentre.
