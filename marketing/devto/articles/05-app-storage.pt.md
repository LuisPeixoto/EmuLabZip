---
title: Veja o que há dentro do armazenamento do seu app no emulador: SQLite, SharedPreferences, MMKV e Realm
description: Navegue pelos arquivos de um app e abra SQLite, SharedPreferences, DataStore, MMKV e Realm no lugar, rode SQL somente leitura e salve o estado com o EmuLabZip.
tags: android, ios, sqlite, debugging
series: EmuLabZip (PT)
cover: docs/images/storage-realm.png
publish_after: 2026-11-26
pair: 05-app-storage
lang: pt
---

Para onde foi esse valor? A migration está certa? Por que esse usuário está preso no onboarding? As respostas estão no
armazenamento privado do app, e chegar até elas normalmente significa `adb shell run-as`, puxar um banco, achar um visualizador
para ele e repetir tudo isso depois da próxima execução.

A ferramenta **Storage** do EmuLabZip navega pelos arquivos de um app no emulador ou simulador e abre os formatos mais comuns
no lugar. Este post mostra o que ela lê, como rodar SQL com segurança e como salvar e comparar o estado inteiro de um app.

![Um arquivo Realm lido do dispositivo: suas classes e as linhas de Person](docs/images/storage-realm.png)

## Onde ela consegue olhar

| Raiz | Android | iOS Simulator |
|---|---|---|
| Dados do app | `/data/data/<package>`, lido com `run-as`. Só apps debuggable | O contêiner de dados (`Documents`, `Library`, `tmp`) |
| Armazenamento compartilhado | `/sdcard/Android/data/<package>` | n/a |
| Bundle do app | n/a | O `.app` instalado |
| App groups | n/a | Todos os contêineres de app group que o app usa |

Os builds de debug do seu app são debuggable por padrão. Builds de release e a maioria dos apps pré-instalados não são, e o
Android não deixa nenhuma ferramenta ler os dados privados deles. O EmuLabZip avisa isso em vez de mostrar uma pasta vazia.

## O que ela abre

Escolha um arquivo e o visualizador acompanha o formato:

- **SQLite**: tabelas e views com contagem de linhas, tipos de coluna e chaves primárias, 100 linhas por página e uma caixa de
  SQL somente leitura.
- **SharedPreferences** e **DataStore** (Preferences e Proto): chave, tipo e valor. O DataStore Proto é decodificado sem o
  schema, então você vê números de campo e valores.
- **MMKV**: chave e valor. O MMKV não guarda tipos, então cada valor aparece com o tipo que parece ter e é marcado como
  palpite.
- **Realm**: classes com contagem de linhas, tipos de coluna e linhas.
- **AsyncStorage** (React Native) e **Core Data**: chaves e valores, ou entidades com seus atributos.
- **Property lists**, JSON, imagens e texto, mostrados da forma mais legível possível, e um hex dump para todo o resto.

Os bancos são copiados para uma pasta temporária privada junto com os arquivos `-wal` e `-shm`, então as alterações que o app
ainda não passou por checkpoint ficam visíveis. A cópia é aberta somente leitura: as consultas leem, mas nunca escrevem, e o
banco real do app nunca é tocado.

## A mesma coisa pelo terminal

```bash
emulabzip files com.example.app
emulabzip files com.example.app databases --root data
emulabzip pull com.example.app shared_prefs/settings.xml
emulabzip db com.example.app databases/app.db
emulabzip db com.example.app databases/app.db --table users
emulabzip db com.example.app databases/app.db -q "SELECT count(*) FROM users"
```

`files` lista, `pull` copia um arquivo para o seu computador e `db` mostra as tabelas, as linhas de uma tabela ou o resultado
de uma consulta somente leitura.

## Alterando dados, com cuidado

O Storage é somente leitura até você ativar o **Edit**. Editar muda os arquivos reais do app, então ele foi feito com cuidado:
cada alteração é validada antes de o dispositivo ser tocado, o app é parado para não sobrescrever a sua mudança, o arquivo
substituído é guardado, o novo arquivo é lido de volta para conferência e a edição pode ser desfeita.

```bash
emulabzip edit cell com.example.shop databases/shop.db -t customers --rowid 1 --text name=Ana --set vip=1 --yes
emulabzip edit pref-set com.example.shop shared_prefs/settings.xml theme dark --yes
emulabzip edit undo <folder the edit printed> --yes
```

Linhas de SQLite e SharedPreferences (e property lists no iOS) podem ser editadas. DataStore, MMKV, AsyncStorage, Realm e
Core Data são somente leitura, porque seus formatos não são gravados de volta.

## Salve o estado inteiro e compare

O estado de um app é o que ele guarda: bancos, preferências, arquivos. Você pode salvar tudo isso em um único arquivo,
colocar de volta e ver o que mudou entre duas versões:

```bash
emulabzip state save com.example.shop -o v1.ezstate
# use the app, or install the new version
emulabzip state save com.example.shop -o v2.ezstate
emulabzip state diff v1.ezstate v2.ezstate
emulabzip state restore v1.ezstate --yes
```

O `state diff` compara a estrutura dos dados (o que uma migration adicionou ou mudou, como uma coluna, um índice ou uma
tabela), que é exatamente o que você quer conferir depois de uma mudança de schema. Restaurar leva você de volta a um ponto
conhecido para reproduzir um bug de novo.

![Um estado salvo aberto no Storage, comparado com outro](docs/images/state-compare.png)

## Experimente

```bash
cargo install --git https://github.com/LuisPeixoto/EmuLabZip emulabzip-cli
```

- Guia do Storage: [emulabzip.luispeixoto.dev/pt/docs/storage/](https://emulabzip.luispeixoto.dev/pt/docs/storage/)
- Estado do app: [emulabzip.luispeixoto.dev/pt/docs/app-state/](https://emulabzip.luispeixoto.dev/pt/docs/app-state/)
- Site e toda a documentação: [emulabzip.luispeixoto.dev](https://emulabzip.luispeixoto.dev/pt/)
- Código-fonte (MIT): [github.com/LuisPeixoto/EmuLabZip](https://github.com/LuisPeixoto/EmuLabZip)
