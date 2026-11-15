---
source: a85b9a63c2f6
description: Abra deep links no dispositivo e verifique se o domínio e o app concordam, com o que está certo, o que merece atenção e o que corrigir, no Android e no iOS.
---
# Deep links

Um link da web abre um app, em vez do navegador, só quando duas coisas concordam: o **domínio** diz que o app pode (um arquivo
no site dele) e o **app** reivindica o domínio. Quando um dos lados está errado, o link abre o navegador e nada diz por quê.
**Deep links** abre um link no dispositivo e **verifica** os dois lados.

## No app

Abra **Deep links** na barra lateral, digite um link e:

- **Open on the device** (abrir no dispositivo) abre o link e o guarda no histórico;
- **Check** (verificar) lê o arquivo do domínio, pergunta ao dispositivo o que ele decidiu e lista o que está certo (✓), o que
  merece atenção (!) e o que está errado (✗), cada item com o que fazer a respeito.

O histórico guarda os últimos 100 links com o resultado da última verificação de cada um, e permite **fixar** os que você usa
com frequência.

![Uma verificação de deep link com seus resultados](images/deep-links.png)

## Na linha de comando

```bash
emulabzip link open "myapp://open/cart" --app com.example.shop
emulabzip link check "https://shop.example.com/cart/3" --app com.example.shop        # exits with an error when something is wrong
emulabzip link verify com.example.shop       # asks Android to verify the app's domains again
emulabzip link history
```

`link check --json` imprime os resultados, para CI. O [MCP](mcp.md) tem `check_link`.

## O que é verificado

**No Android**

- `https://<domain>/.well-known/assetlinks.json` é servido com status 200, sem redirecionamento (o Android não os segue), como
  `application/json`, e é válido;
- ele autoriza o app: o nome do pacote está em uma declaração com `delegate_permission/common.handle_all_urls`;
- o **certificado de assinatura do app confere**: o SHA-256 que o dispositivo informa (`pm get-app-links`, Android 12 e
  posterior) é um dos listados no arquivo. Um build de debug é assinado com a chave de debug, não com a de release;
- o **dispositivo verificou o domínio** para o app, e a chave "Open supported links" do usuário está ligada; quando não está, a
  correção é `pm verify-app-links --re-verify <app>`, que **Verify again** e `link verify` executam;
- **quem abre o link** (`cmd package resolve-activity`): seu app, outro app ou ninguém. Um seletor que lista um navegador não é
  reportado quando o domínio está verificado, porque um link verificado vai direto para o app.

**No iOS**

- `https://<domain>/.well-known/apple-app-site-association` é servido com 200, sem redirecionamento, e é válido (`applinks` com
  `components`, ou o `paths` mais antigo);
- o bundle id do app está em `appIDs`, e uma **regra confere com o caminho do link**: o primeiro componente que confere decide,
  e uma regra `exclude` pode recusá-lo;
- o **entitlement Associated Domains** do app tem `applinks:<domain>`, e o team id dele é o do arquivo (lido da assinatura do
  app no simulador; um build sem assinatura não tem um, e isso é informado).

**Para um link do esquema próprio do app** (`myapp://…`) não há arquivo de domínio; a verificação diz quem o abre.

## Limites

- Um simulador não verifica domínios como um dispositivo faz, então a verificação no iOS trata dos arquivos e do entitlement.
- `pm get-app-links` exige Android 12 ou posterior. Em versões mais antigas o arquivo ainda é verificado.
- Só o arquivo do próprio domínio é lido, por HTTPS, com no máximo 256 KB e limite de dez segundos.
- O `apple-app-site-association` do site é servido como `application/octet-stream` pelo GitHub Pages, e a verificação avisa:
  um host que não consegue definir o tipo mostrará esse aviso. Na prática a Apple aceita o arquivo; use outro host se quiser o
  tipo correto.

## A demo

O `tools/compose-app` trata `https://emulabzip.luispeixoto.dev/demo` como um app link verificado, e `emulabzip-demo://open/…`
como um link próprio. O site serve um `assetlinks.json` que lista a demo e a chave de debug com a qual ela é compilada, então
`emulabzip link check https://emulabzip.luispeixoto.dev/demo/cart --app dev.emulabzip.composedemo` mostra todos os ✓ em um
emulador que tenha a demo instalada. O `apple-app-site-association` dela usa um team id fictício de propósito, para que a
verificação no iOS mostre como é uma divergência.
