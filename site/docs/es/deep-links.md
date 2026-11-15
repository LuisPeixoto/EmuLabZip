---
source: a85b9a63c2f6
description: "Abre un deep link en el dispositivo y comprueba que el dominio y la app están de acuerdo: assetlinks.json, apple-app-site-association y firma."
---
# Deep links

Un enlace web abre una app, en lugar del navegador, solo cuando dos partes están de acuerdo: el **dominio** dice que la app puede
hacerlo (un archivo en su sitio web) y la **app** reclama el dominio. Cuando una de las dos partes falla, el enlace abre el
navegador y nada dice por qué. **Deep links** abre un enlace en el dispositivo y **comprueba** ambas partes.

## En la app

Abre **Deep links** en la barra lateral, escribe un enlace y:

- **Open on the device** lo abre y lo recuerda;
- **Check** lee el archivo del dominio, pregunta al dispositivo qué decidió y lista lo que está bien (✓), lo que conviene revisar (!) y
  lo que está mal (✗), cada cosa con lo que debes hacer al respecto.

El historial guarda los últimos 100 enlaces con el resultado de su última comprobación, y **fija** los que usas con frecuencia.

![Una comprobación de deep link con sus resultados](images/deep-links.png)

## En la línea de comandos

```bash
emulabzip link open "myapp://open/cart" --app com.example.shop
emulabzip link check "https://shop.example.com/cart/3" --app com.example.shop        # exits with an error when something is wrong
emulabzip link verify com.example.shop       # asks Android to verify the app's domains again
emulabzip link history
```

`link check --json` imprime los resultados, para CI. [MCP](mcp.md) tiene `check_link`.

## Qué se comprueba

**En Android**

- `https://<domain>/.well-known/assetlinks.json` se sirve con estado 200, sin redirección (Android no las sigue),
  como `application/json`, y es válido;
- concede permiso a la app: su nombre de paquete está en una declaración con `delegate_permission/common.handle_all_urls`;
- el **certificado de firma de la app coincide**: el SHA-256 que informa el dispositivo (`pm get-app-links`, Android 12 y posteriores) es
  uno de los que lista el archivo. Un build de depuración se firma con la clave de depuración, no con la de release;
- el **dispositivo verificó el dominio** para la app, y el interruptor "Open supported links" del usuario está activado; cuando no lo está, la
  solución es `pm verify-app-links --re-verify <app>`, que ejecutan **Verify again** y `link verify`;
- **quién abre el enlace** (`cmd package resolve-activity`): tu app, otra app o ninguna. Un selector que lista un navegador no
  se informa cuando el dominio está verificado, porque un enlace verificado va directo a la app.

**En iOS**

- `https://<domain>/.well-known/apple-app-site-association` se sirve con 200, sin redirección, y es válido
  (`applinks` con `components`, o el antiguo `paths`);
- el bundle id de la app está en `appIDs`, y una **regla coincide con la ruta del enlace**: el primer componente que coincide decide, y una
  regla `exclude` puede rechazarlo;
- el **entitlement Associated Domains** de la app tiene `applinks:<domain>`, y su team id es el del archivo (se lee de la firma de la app en el
  simulador; un build sin firmar no tiene ninguno, y se indica).

**Para un enlace del esquema propio de la app** (`myapp://…`) no hay archivo de dominio; la comprobación indica quién lo abre.

## Límites

- Un simulador no verifica dominios como lo hace un dispositivo, así que la comprobación en iOS trata de los archivos y del entitlement.
- `pm get-app-links` requiere Android 12 o posterior. En versiones anteriores el archivo se comprueba igualmente.
- Solo se lee el archivo propio del dominio, por HTTPS, de como máximo 256 KB, con un límite de diez segundos.
- GitHub Pages sirve el `apple-app-site-association` del sitio web como `application/octet-stream`, y la comprobación lo indica: un
  host que no puede fijar el tipo mostrará esa advertencia. En la práctica Apple acepta el archivo; usa otro host si quieres
  el tipo correcto.

## La demo

`tools/compose-app` gestiona `https://emulabzip.luispeixoto.dev/demo` como un app link verificado, y `emulabzip-demo://open/…` como
un enlace propio. El sitio web sirve un `assetlinks.json` que lista la demo y la clave de depuración con la que se compila, de modo que
`emulabzip link check https://emulabzip.luispeixoto.dev/demo/cart --app dev.emulabzip.composedemo` muestra todo ✓ en un emulador
que tenga la demo instalada. Su `apple-app-site-association` usa a propósito un team id de relleno, para que la comprobación en iOS muestre
cómo se ve una discrepancia.
