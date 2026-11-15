---
source: c99e4d2c05ff
description: "Inspecciona las páginas web de las apps híbridas: lista las web views, lee su consola, ejecuta JavaScript y muestra sus peticiones en la vista Network."
---
# Páginas web en las apps

Las apps híbridas muestran páginas web en una **WebView**. EmuLabZip las lee como lo hace DevTools de Chrome: lista las
páginas, muestra su consola, ejecuta JavaScript en ellas y muestra las peticiones que hace la página, en la vista Network
junto a todo lo que vio el proxy. En iOS te lleva al Web Inspector de Safari.

## Qué tiene que permitir una app

En Android, una WebView solo se puede inspeccionar cuando su app activa la depuración web:

```kotlin
WebView.setWebContentsDebuggingEnabled(true)   // in a debug build
```

Chrome en el dispositivo también aparece en la lista, mientras está abierto. No hay nada que instalar ni puerto que
redirigir: EmuLabZip lee la lista de sockets abiertos del dispositivo, encuentra `webview_devtools_remote_<pid>` y
`chrome_devtools_remote`, y habla con ellos a través de ADB.

`tools/compose-app` tiene una pantalla para probarlo: **Open the web page** inicia una WebView con una página que escribe
en la consola, hace un fetch al sitio web, hace un fetch de algo que no existe y lanza un error.

## En la app

**Frameworks → Web pages** lista todas las páginas que se pueden inspeccionar, con la app a la que pertenecen. Elige una y:

- la **consola** sigue en vivo: logs, advertencias, errores y los errores que la página no captura, cada uno con su origen;
- la línea de la parte inferior **ejecuta JavaScript** en la página, como `document.title`, y muestra el valor;
- **Reload** recarga la página, ignorando su caché;
- **Show requests in Network** también lista las peticiones de la página en la vista [Network](network.md), marcadas como **WEBVIEW**. Vuelve a elegir la
  página después de cambiarla.

Las peticiones vienen de la propia página, así que aparecen incluso cuando el proxy no pudo descifrar el tráfico (el
certificate pinning no afecta a la página) y sin instalar un certificado. No se pueden modificar ni retener en un
breakpoint como las peticiones que pasan por el proxy; para eso, usa el proxy.

![El panel Web pages con la consola de la WebView de la demo](images/webview-console.png)

![Las peticiones de la WebView en Network, marcadas como WEBVIEW](images/webview-network.png)

## En iOS

Apple no permite que otras herramientas lean un `WKWebView`, así que en un simulador el panel da los pasos y un botón que abre Safari:

![El panel Web pages en un simulador de iOS](images/webview-ios.png)

1. En Safari, abre Settings → Advanced y activa "Show features for web developers".
2. Abre la página en tu app. Un `WKWebView` tiene que tener `isInspectable = true` (iOS 16.4 y posteriores; los builds de depuración suelen activarlo).
3. En el menú **Develop** de Safari, elige el simulador y luego la página.

## En la línea de comandos

```bash
emulabzip webview list                                   # the sockets and their pages
emulabzip webview console --app com.example.shop         # what the page writes, for 10 seconds (--seconds N)
emulabzip webview eval "document.title"                  # run JavaScript; a script that throws fails the command
emulabzip webview network --reload --seconds 10 --har page.har
```

`--app` elige la WebView de la app, `--socket` un socket por nombre, `--page` una página por id o por parte de su
dirección. Cada uno se puede omitir cuando solo hay una opción. `--json` está disponible en cada comando.

## Desde un asistente

`webview_list` y `webview_console` son [herramientas MCP](mcp.md). Ejecutar JavaScript no se ofrece por MCP, porque un
script puede cambiar lo que guarda la app; se queda en la app y en la línea de comandos.

## Límites

- Solo Android, y solo WebViews depurables y Chrome. Un build de release que no activó la depuración no se puede leer.
- Los cuerpos de respuesta se leen hasta 10 MB. Un cuerpo que ya no está en la memoria del navegador cuando se pide se
  muestra sin su texto.
- La página se lee a través de una conexión propia. Otro depurador (`chrome://inspect`) puede estar abierto al mismo tiempo.
