---
source: 7f222a4d726b
description: Leia o estado ao vivo de um app React Native em desenvolvimento, com Redux, React Query, contextos e stores próprios, e acompanhe as ações do Redux.
---
# Estado do React Native

**Frameworks → React Native → State** e `emulabzip rn state` leem o estado ao vivo de um app React Native em desenvolvimento:
Redux, React Query, contexto do React e as stores que o seu próprio código registra. `--watch` acompanha as ações do Redux e
mostra o que cada uma mudou.

```bash
emulabzip rn state com.example.shop
emulabzip rn state com.example.shop --watch
emulabzip rn state com.example.shop --json
```

Ele precisa do que o [React Native](frameworks.md) já precisa: uma build de desenvolvimento, o Metro rodando e um depurador
por vez.

## O que é lido

| Fonte | Como é encontrada |
|--------|-----------------|
| **Redux** | Um `Provider` (react-redux) cuja prop `store` tem `getState`, `dispatch` e `subscribe`; o estado é `getState()` |
| **React Query** (TanStack Query) | Um provider com um `client`: cada query do cache, com a chave, o status, o status da busca, quando foi atualizada, o erro e os dados |
| **React context** | Providers de contexto na tela, pelo `displayName` (ou pelo componente ao redor) e pelo `value`. Os contextos do próprio Redux e do React Query não aparecem duas vezes |
| **Named stores** | Qualquer coisa que você registre em `globalThis.__EMULABZIP_STORES__` |

### Zustand, MobX, Jotai e os outros

Registre a store por nome; o EmuLabZip chama `getState()` (ou `get()`), ou lê o objeto como ele está:

```ts
import {create} from 'zustand';
const useSettings = create(() => ({units: 'metric'}));
globalThis.__EMULABZIP_STORES__ = {settings: useSettings};
```

Nada é instalado no app, e nada acontece se o nome nunca for lido. Em uma build de release, o registro é a única coisa que
funciona: o React não mantém um hook do DevTools ali, então Redux, React Query e contextos não são encontrados.

## Como os valores são mostrados

O script da página transforma o estado em JSON e informa o que deixou de fora. Ciclos são marcados com `{"$":"circular"}`,
funções com `{"$":"function"}`, e Dates, Maps, Sets, erros, bigints e typed arrays são marcados com o seu tipo. Ele para em
8 níveis, 200 chaves ou itens por objeto, 2.000 caracteres por string e cerca de 1 MB, e a lista **cut** diz onde.

## Acompanhando ações

`--watch` (e **Watch changes**) envolve o `dispatch` de cada store do Redux, daí em diante, e guarda as últimas 200 ações
com o tipo e o payload. A cada segundo ele mostra as novas ações e a diferença do estado em nível de caminho:

```
action cart/add
  redux/cart/items/0/qty: 1 → 2
```

Quando você para, o `dispatch` é devolvido e o log é removido da página. É a única coisa desta fase que muda o app em
execução, e só enquanto você acompanha. Um componente que pegou o `dispatch` antes de você começar continua com o antigo,
então as ações dele não aparecem no log até ele renderizar de novo.

## Limites

- O estado do Flutter (Provider, Riverpod, Bloc) não é lido.
- O Redux é encontrado por um `Provider` do react-redux; uma store usada sem ele é encontrada se você a registrar.
- Os valores são um snapshot: um estado muito grande é cortado, e `getState()` de uma store com efeitos colaterais nos getters os executa.
