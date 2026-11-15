---
source: d4619f203dc1
description: Toca, escribe y pulsa teclas en un dispositivo Android, graba lo que haces como flujo, repítelo y expórtalo como flujo de Maestro o script de shell.
---
# Entrada y flujos

EmuLabZip puede tocar un dispositivo, escribir en él y pulsar sus teclas; puede **grabar** lo que haces en un **flujo**, repetir el flujo
y exportarlo como un flujo de [Maestro](https://maestro.mobile.dev) o como un script de shell. Todo esto es para **Android**:
iOS no tiene una forma pública de enviar toques ni de leerlos.

## Tocar, escribir y pulsar

| Control | CLI | Qué hace |
|---------|-----|--------------|
| Tap | `emulabzip control tap <x> <y>` | Un toque en un punto de la pantalla, en píxeles, como en una captura de pantalla. |
| Swipe | `emulabzip control swipe <x1> <y1> <x2> <y2> [--ms 300]` | Un arrastre. De un punto a sí mismo es una pulsación larga. |
| Text | `emulabzip control text "hello"` | Escribe en el campo que tiene el foco. Solo ASCII imprimible. |
| Key | `emulabzip control key back` | `back`, `home`, `app_switch`, `enter`, `tab`, `delete`, `volume_up`… |
| Launch | `emulabzip control launch <app>` | Inicia una app como lo haría el launcher. También en iOS. |

Las posiciones se comprueban (como máximo 20000 píxeles), el texto se comprueba (no puede contener `%s`, que Android lee como un espacio, ni
nada que `input text` no pueda escribir), y las teclas son una lista de nombres, así que nada llega a un shell sin comprobar. Los mismos controles son
[herramientas MCP](mcp.md): `tap`, `swipe`, `input_text`, `press_key`, `launch_app`.

En la app, **Overview → Screen → Interact** hace que la imagen de la pantalla sea táctil: un clic es un tap, un arrastre es un swipe,
y una pulsación mantenida durante medio segundo es una pulsación larga. **Controls** tiene una tarjeta para texto y teclas.

## Grabar un flujo

```bash
emulabzip flow record --name "Add to cart" --app com.example.shop     # touch the emulator, then press Ctrl+C
```

Toca el dispositivo mismo: en un emulador, su propia ventana. EmuLabZip lee la pantalla táctil (`getevent`) y conserva cada tap,
pulsación larga y deslizamiento, y las pausas entre ellos (una pausa de un segundo o más, redondeada a la décima de segundo más cercana). El flujo
empieza con un paso que inicia la app, salvo que pases `--no-launch`.

Hay dos cosas que no se graban, y la app lo indica:

- **Los taps enviados por EmuLabZip o por `adb shell input`.** Entran más arriba en Android y nunca llegan a la pantalla táctil, así que
  hacer clic en la imagen de Interact o reproducir un flujo mientras se graba no añade pasos.
- **El texto escrito.** El teclado no es la pantalla táctil. Añade un paso de texto después.

Después de cada toque la pantalla se vuelve a leer en segundo plano, para que el siguiente tap pueda recordar **qué tocó**: el id de la vista
(un `testTag` de Compose, un id de recurso de Android) o su texto. Leer una pantalla tarda dos o tres segundos, así que un tap que llega
antes que eso tras otro se conserva como un punto.

Los flujos son archivos JSON en la carpeta `flows` de EmuLabZip, compartida por la app y la línea de comandos:

```bash
emulabzip flow list
emulabzip flow show add-to-cart          # the steps; --json for the file
emulabzip flow play add-to-cart          # Ctrl+C stops it
emulabzip flow remove add-to-cart
```

Al reproducir, los puntos se escalan a la pantalla en la que se reproduce, cuando su tamaño difiere del de aquella en la que se grabó el flujo. A cada
paso le sigue un tercio de segundo, para que la pantalla pueda reaccionar.

![La vista Automation con un flujo grabado](images/automation.png)

## La vista Automation

**Automation** en la barra lateral graba y reproduce flujos, lista los guardados y muestra los pasos del que eliges. Puedes
eliminar un paso, o añadir a mano un texto, una tecla o una pausa, y luego **Save changes**. **Maestro** y **Script** guardan el flujo en
esos formatos.

## Exportar

```bash
emulabzip flow export add-to-cart --maestro > add-to-cart.yaml
emulabzip flow export add-to-cart --script --out add-to-cart.sh
```

**Maestro.** Un tap que conoce su vista dice `tapOn: id: "checkout"` o `tapOn: "Checkout"`; los demás indican un punto como
porcentaje de la pantalla (`point: "50%,80%"`). El texto se escapa para las expresiones regulares de Maestro, y luego se entrecomilla para YAML. Las teclas usan
los nombres de Maestro (`Back`, `Home`, `Enter`, `Backspace`…), y una tecla para la que no tiene nombre se deja como comentario. Las pausas se convierten en
`waitForAnimationToEnd`. El archivo no tiene aserciones: añádelas, ya que una grabación sabe lo que hiciste y no lo que debería mostrar.

**Script.** Un script de shell con líneas `emulabzip control …` y `sleep` entre ellas, para el dispositivo de `EMULABZIP_DEVICE`.

## Límites

- Solo Android. Un flujo no puede grabarse ni reproducirse en un simulador.
- Las etiquetas son de mejor esfuerzo. Una pantalla que cambia sin un toque (un temporizador, una respuesta de red) puede dar a un tap el nombre de
  lo que había un momento antes; el punto siempre se conserva, y Maestro usa la etiqueta solo cuando hay una.
- Un flujo tiene como máximo 5000 pasos, y una pausa como máximo un minuto. El `play_flow` de un asistente rechaza flujos que tardan más de
  dos minutos; reprodúcelos con `emulabzip flow play`.
