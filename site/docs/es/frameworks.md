---
source: dcdd776b1e52
description: "Mira dentro de Flutter, React Native, Jetpack Compose y SwiftUI: detecta el framework de una app e inspecciona widgets, componentes, frames y memoria."
---
# Dentro de los frameworks

Flutter y React Native dibujan su propia pantalla, y Jetpack Compose y SwiftUI dibujan la suya de forma distinta a las
vistas de la plataforma. El árbol de vistas habitual dice poco sobre ellos, así que EmuLabZip averigua qué framework usa
una app y mira dentro con las herramientas propias del framework.

```bash
emulabzip frameworks com.example.app
```

Lee los archivos de la app, esté o no en ejecución:

| Framework | Qué lo delata |
|-----------|--------------------|
| Flutter | `libflutter.so` en el APK, o `Flutter.framework` en el bundle de la app |
| React Native | `libreactnative.so`, `libreactnativejni.so` o `index.android.bundle`; `main.jsbundle` o `React.framework` en iOS |
| Hermes | `libhermes.so` o `libhermesvm.so`; `hermes.framework` |
| Jetpack Compose | los archivos `META-INF/androidx.compose.*.version` que las bibliotecas escriben en el APK (la versión se lee de ellos) |
| SwiftUI | `SwiftUI.framework` entre las bibliotecas con las que está enlazado el ejecutable |

En la app de escritorio, **Frameworks** muestra lo mismo, y debajo los paneles de Flutter y React Native.

## Flutter

![El árbol de widgets de una app de Flutter en la vista Frameworks, con el archivo y la línea de cada widget que creó la app](images/frameworks-flutter.png)

Los builds de debug y profile tienen un **Dart VM service**, que la app anuncia en el log:
`The Dart VM service is listening on http://127.0.0.1:41233/token=/`. EmuLabZip encuentra esa línea, llega al puerto
a través del dispositivo (`adb` en Android, la propia red del Mac en el simulador, no se redirige ni se deja nada) y
habla JSON-RPC sobre un WebSocket.

```bash
emulabzip flutter tree com.example.app
emulabzip flutter frames com.example.app --seconds 10     # use the app meanwhile
emulabzip flutter memory com.example.app
```

- **Widgets**: el árbol que muestra el inspector de Flutter. `●` marca los widgets creados por tu propio código, con el
  archivo y la línea; `○` marca los del framework y de los paquetes. Los nombres de archivo necesitan que la app sepa
  dónde se crearon sus widgets. Las apps iniciadas con `flutter run` lo saben. Para `flutter build` añade
  `--extra-front-end-options=--track-widget-creation` (`tools/flutter-app/install.sh` lo hace). Sin ello, el árbol no
  tiene nombres de archivo y lo dice.
- **Frames**: tiempo de build (el hilo de UI) y tiempo de raster (el hilo de dibujo) de cada frame que dibuja la app, con
  los frames con jank contados: un frame tiene jank cuando cualquiera de los dos supera 16.7 ms. **No se dibuja nada
  mientras la app está inactiva**, así que el número de frames depende de lo que hagas mientras graba. Un emulador sin
  tarjeta gráfica rasteriza por software, así que sus tiempos de raster dicen poco sobre un teléfono.
- **Memory**: el heap de Dart en uso y su tamaño, la memoria fuera del heap que mantienen viva los objetos de Dart
  (imágenes, búferes) y las clases que más usan.

Si la dirección ya no está en el log (el log de una app ocupada se desplaza), inicia la app de nuevo, o pásala:
`--uri http://127.0.0.1:41233/token=/`. Un build de release no tiene VM service.

## React Native

![El árbol de componentes de React de una app de React Native en la vista Frameworks, con props](images/frameworks-react-native.png)

Las apps de React Native en desarrollo hablan con **Metro**, el servidor de desarrollo, que lista el runtime de JavaScript
de cada app conectada y hace de proxy de su protocolo Chrome DevTools. EmuLabZip se conecta a Metro en tu equipo (puerto
8081, o `--metro <port>`), elige el runtime que pertenece a la app y lee:

```bash
emulabzip rn tree com.example.app           # the React components, with props
emulabzip rn heap com.example.app           # Hermes' heap in use
emulabzip rn console com.example.app --seconds 20
```

- **Components**: el árbol de React, leído del hook de React DevTools que instala un build de desarrollo. `●` marca los
  componentes, `○` las vistas nativas en que se convierten. Las props se abrevian: `title="Hello"`, `onPress=ƒ`.
- **Hermes heap** y la versión de Hermes.
- **Console**: lo que la app escribe con `console.log` y similares mientras escuchas.

La pestaña **State** (y `emulabzip rn state`) lee Redux, React Query, el contexto de React y tus propios stores
registrados: consulta [Estado de React Native](react-native-state.md).

**Metro deja que un solo depurador se conecte a una app a la vez.** Leer desde EmuLabZip puede cerrar React Native
DevTools o el inspector de Chrome, y al revés. Los builds de release no tienen Metro ni hook.

En Android la app llega a Metro mediante `adb reverse tcp:8081 tcp:8081`, que EmuLabZip no toca.

## Jetpack Compose y SwiftUI

![Una pantalla de Jetpack Compose en Layout, con sus vistas marcadas como Compose y el inspector nombrando el framework](images/layout-compose.png)

En **Layout**, las vistas que dibuja un framework están marcadas, y el inspector muestra cuál. Todo lo que hay bajo un
`ComposeView` es Compose; los test tags (`Modifier.testTag`) se muestran como ids cuando la app activa
`testTagsAsResourceId`. Bajo un `FlutterView` o una vista raíz de React Native, las marcas dicen `flutter` y
`react-native`.

En iOS el árbol de accesibilidad no tiene ninguna clase que diga qué dibujó una vista, así que con
`layout --app <bundle id>` (y al seleccionar una app en la app de escritorio) toda la pantalla se marca con el framework
de la app: Flutter, React Native, Compose, luego SwiftUI, en ese orden de preferencia. Los tipos de vista propios de
SwiftUI (`VStack`, `Button`) no están disponibles mediante accesibilidad: ves roles de accesibilidad y los
identificadores que diste.

**Los recuentos de recomposición no están disponibles.** El Layout Inspector de Android Studio los obtiene de un agente
cargado en la app depurada, que usa interfaces que no son públicas. EmuLabZip solo usa interfaces públicas, así que no
los muestra.

## Pruébalo

`tools/` tiene una app de demostración para cada uno: `compose-app`, `flutter-app`, `rn-app` y `swiftui-app`, cada una con
un README.
