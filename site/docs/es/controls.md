---
source: 90122b673163
description: "Cambia lo que informa o hace un dispositivo: ubicación, apariencia, barra de estado, batería, permisos y notificaciones, y deshaz cada cambio."
---
# Controls

La herramienta Controls cambia lo que un dispositivo informa o hace, para que puedas probar una app en otro lugar, a oscuras, con una
barra de estado limpia o un permiso menos, sin tocar las pantallas de ajustes del propio dispositivo. Cada cambio se puede deshacer
desde la misma tarjeta.

| Control | Android | iOS Simulator |
|---------|---------|---------------|
| **Location** | Solo emuladores, mediante la consola del emulador | ✅ |
| **Appearance** (claro u oscuro) | Android 10 y posteriores | ✅ |
| **Status bar** (hora, batería, Wi-Fi, celular) | Modo demo de SystemUI | ✅ |
| **Battery** (nivel, carga) | ✅ | no es posible: usa la barra de estado |
| **Permissions** | Concede y revoca cualquier permiso de ejecución de la app elegida, con el estado actual a la vista | Concede, revoca o vuelve a preguntar por un servicio (fotos, ubicación, …) |
| **Notification** | Publicada desde el shell | Un push simulado a la app elegida, con tu propio payload si quieres |
| **Open a link** | ✅ | ✅ |

<p align="center">
  <img src="images/controls.png" alt="La vista Controls" />
</p>

## Inicio rápido

Abre **Controls** (<kbd>⌘9</kbd>). Solo se muestran las tarjetas que admite el dispositivo seleccionado. La tarjeta de permisos sigue
la app elegida en la barra de herramientas. Tras cada cambio se vuelve a leer el dispositivo, de modo que la página muestra lo que realmente informa.

Desde un terminal:

```bash
emulabzip control appearance dark
emulabzip control location set -23.5505 -46.6333       # São Paulo
emulabzip control location clear
emulabzip control statusbar set --time 9:41 --battery 100
emulabzip control statusbar clear
emulabzip control permission com.example.app android.permission.CAMERA revoke
emulabzip control permission com.example.app photos reset      # iOS
emulabzip control notify com.example.app --title Hello --body "A test"
emulabzip control open myapp://items/3
emulabzip control battery set 15
emulabzip control battery reset
emulabzip control info --app com.example.app                    # what is supported, and the permissions
```

## Qué hace cada uno en el dispositivo

**Android.** Todo pasa por `cmd`, `settings`, `pm`, `am` y `dumpsys`, que el shell puede ejecutar, y por
la consola del emulador para la ubicación.

- *Location* envía `geo fix` a la consola en `localhost:<port>`, con el token en
  `~/.emulator_console_auth_token`. El emulador no tiene forma de olvidar una posición, así que **Reset** la devuelve a su propio
  valor por defecto (el Googleplex). Los dispositivos físicos no son compatibles.
- *Appearance* es `cmd uimode night yes|no`, y se vuelve a leer.
- *Status bar* entra en el modo demo de SystemUI (`sysui_demo_allowed` se pone en 1 y se deja así) y muestra lo que
  pediste, con los iconos de notificación ocultos. **Clear** sale del modo demo.
- *Battery* es `dumpsys battery set level`, con `set ac 1` para cargar y `unplug` en caso contrario. **Reset** es
  `dumpsys battery reset`.
- *Notification* es `cmd notification post`. Android la publica como quien ejecuta el comando, así que cuando `adbd` es root (los
  emuladores sin Google Play) EmuLabZip la ejecuta como el usuario shell. El argumento de la app se ignora.

- *Text size* es `settings put system font_scale`, como porcentaje de lo normal (50 a 300).
- *Language* es `cmd locale set-app-locales` para **una app** (Android 13 y posteriores), incluidos los pseudolocales `en-XA`
  y `ar-XB`. *Display size* es `wm size` y `wm density`. Los tres se pueden deshacer. Consulta
  [Idioma, tamaño de texto y texto que no cabe](localization.md).

- *Touch, swipe, text and keys* son `input tap`, `input swipe`, `input text` y `input keyevent`; *Launch* es `monkey -p`
  con la categoría del launcher; *Verify links* es `pm verify-app-links --re-verify`. Consulta [Entrada y flujos](automation.md) y
  [Deep links](deep-links.md).

**iOS Simulator.** Todo es `xcrun simctl`: `location`, `ui appearance`, `status_bar`, `privacy`, `push` y
`openurl`, con `ui content_size` para el tamaño de texto y los valores globales (`AppleLanguages`, `AppleLocale`) para el idioma, que una app toma al volver a arrancar. iOS no informa qué permisos tiene una app, así que la tarjeta ofrece solo los servicios y las acciones. *Launch* es `simctl launch`;
los toques, deslizamientos, texto y teclas no se ofrecen en iOS, que no tiene una forma pública de enviarlos.

## Seguridad

Cada control se comprueba antes de que lo vea un dispositivo, venga de la app, de la línea de comandos o de un asistente:
las coordenadas deben ser números dentro del rango, la hora debe parecer una hora, los nombres solo pueden usar caracteres simples y un
payload debe ser JSON de como máximo 4 KB. Los **enlaces** pueden usar `http`, `https` o el esquema propio de una app; `file:`, `javascript:`,
`data:`, `content:`, `intent:` y similares se rechazan. Los valores que llegan al shell de Android van entrecomillados, y `simctl` se
inicia sin shell.
