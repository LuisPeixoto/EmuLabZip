# React Native state

**Frameworks → React Native → State** and `emulabzip rn state` read the live state of a React Native development app:
Redux, React Query, React context and the stores your own code registers. `--watch` follows Redux actions and prints what
each one changed.

```bash
emulabzip rn state com.example.shop
emulabzip rn state com.example.shop --watch
emulabzip rn state com.example.shop --json
```

It needs what [React Native](frameworks.md) already needs: a development build, running Metro, and one debugger at a
time.

## What is read

| Source | How it is found |
|--------|-----------------|
| **Redux** | A `Provider` (react-redux) whose `store` prop has `getState`, `dispatch` and `subscribe`; the state is `getState()` |
| **React Query** (TanStack Query) | A provider with a `client`: every query of its cache, with the key, status, fetch status, when it was updated, the error and the data |
| **React context** | Context providers on screen, by their `displayName` (or the component around them) and their `value`. Redux's and React Query's own contexts are not listed twice |
| **Named stores** | Anything you register in `globalThis.__EMULABZIP_STORES__` |

### Zustand, MobX, Jotai and the rest

Register the store by name; EmuLabZip calls `getState()` (or `get()`), or reads the object as it is:

```ts
import {create} from 'zustand';
const useSettings = create(() => ({units: 'metric'}));
globalThis.__EMULABZIP_STORES__ = {settings: useSettings};
```

Nothing is installed in the app, and nothing happens if the name is never read. In a release build the registration is
the only thing that works at all: React keeps no DevTools hook there, so Redux, React Query and contexts are not found.

## How values are shown

The page script turns state into JSON, and says what it left out. Cycles are marked `{"$":"circular"}`, functions
`{"$":"function"}`, and Dates, Maps, Sets, errors, bigints and typed arrays are marked with their kind. It stops at 8
levels, 200 keys or items per object, 2,000 characters per string and about 1 MB, and the **cut** list says where.

## Following actions

`--watch` (and **Watch changes**) wraps `dispatch` of each Redux store, from then on, and keeps the last 200 actions with
their type and payload. Every second it prints the new actions and a path-level difference of the state:

```
action cart/add
  redux/cart/items/0/qty: 1 → 2
```

When you stop, `dispatch` is put back and the log is removed from the page. It is the one thing in this phase that changes
the running app, and only for as long as you watch. A component that took `dispatch` before you started keeps the old
one, so its actions are not in the log until it renders again.

## Limits

- Flutter state (Provider, Riverpod, Bloc) is not read.
- Redux is found through a react-redux `Provider`; a store used without one is found if you register it.
- Values are a snapshot: a very large state is cut, and `getState()` of a store with side effects in its getters runs them.
