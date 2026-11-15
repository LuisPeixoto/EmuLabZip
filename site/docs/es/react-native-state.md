---
source: 7f222a4d726b
description: Lee el estado en vivo de una app de desarrollo de React Native, con Redux, React Query, contextos y stores propios, y sigue las acciones de Redux.
---
# Estado de React Native

**Frameworks → React Native → State** y `emulabzip rn state` leen el estado en vivo de una app de desarrollo de React Native:
Redux, React Query, el contexto de React y los stores que registra tu propio código. `--watch` sigue las acciones de Redux
e imprime lo que cambió cada una.

```bash
emulabzip rn state com.example.shop
emulabzip rn state com.example.shop --watch
emulabzip rn state com.example.shop --json
```

Necesita lo mismo que ya necesita [React Native](frameworks.md): un build de desarrollo, Metro en ejecución y un solo
depurador a la vez.

## Qué se lee

| Fuente | Cómo se encuentra |
|--------|-------------------|
| **Redux** | Un `Provider` (react-redux) cuya prop `store` tiene `getState`, `dispatch` y `subscribe`; el estado es `getState()` |
| **React Query** (TanStack Query) | Un provider con un `client`: cada query de su caché, con la clave, el estado, el estado de la carga, cuándo se actualizó, el error y los datos |
| **React context** | Los providers de contexto en pantalla, por su `displayName` (o el componente que los rodea) y su `value`. Los contextos propios de Redux y de React Query no se listan dos veces |
| **Named stores** | Cualquier cosa que registres en `globalThis.__EMULABZIP_STORES__` |

### Zustand, MobX, Jotai y el resto

Registra el store por nombre; EmuLabZip llama a `getState()` (o `get()`), o lee el objeto tal como está:

```ts
import {create} from 'zustand';
const useSettings = create(() => ({units: 'metric'}));
globalThis.__EMULABZIP_STORES__ = {settings: useSettings};
```

No se instala nada en la app, y no ocurre nada si el nombre nunca se lee. En un build de release, el registro es lo único
que funciona: React no mantiene allí ningún hook de DevTools, así que Redux, React Query y los contextos no se encuentran.

## Cómo se muestran los valores

El script de la página convierte el estado en JSON e indica lo que dejó fuera. Los ciclos se marcan como `{"$":"circular"}`,
las funciones como `{"$":"function"}`, y las fechas, Maps, Sets, errores, bigints y arrays tipados se marcan con su tipo.
Se detiene a los 8 niveles, 200 claves o elementos por objeto, 2.000 caracteres por cadena y alrededor de 1 MB, y la lista
**cut** indica dónde.

## Seguir las acciones

`--watch` (y **Watch changes**) envuelve el `dispatch` de cada store de Redux, desde ese momento, y guarda las últimas 200
acciones con su tipo y su payload. Cada segundo imprime las acciones nuevas y una diferencia, a nivel de ruta, del estado:

```
action cart/add
  redux/cart/items/0/qty: 1 → 2
```

Cuando paras, `dispatch` se restaura y el log se elimina de la página. Es lo único de esta fase que cambia la app en
ejecución, y solo mientras observas. Un componente que tomó `dispatch` antes de que empezaras conserva el antiguo, así que
sus acciones no están en el log hasta que se vuelva a renderizar.

## Límites

- El estado de Flutter (Provider, Riverpod, Bloc) no se lee.
- Redux se encuentra a través de un `Provider` de react-redux; un store usado sin él se encuentra si lo registras.
- Los valores son un snapshot: un estado muy grande se corta, y `getState()` de un store con efectos secundarios en sus getters los ejecuta.
