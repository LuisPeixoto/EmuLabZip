---
source: 236e4411c357
description: A ferramenta Storage navega pelos arquivos do app no Android e no iOS Simulator e abre bancos SQLite, preferências, Realm, MMKV e outros formatos no lugar.
---
# Storage

A ferramenta Storage navega pelos arquivos do app selecionado na barra de ferramentas e abre os formatos comuns no
próprio lugar.

## Onde você pode olhar

| Raiz | Android | iOS Simulator |
|------|---------|---------------|
| Dados do app | `/data/data/<package>`, lido com `run-as`. **Somente apps depuráveis.** | O contêiner de dados (`Documents`, `Library`, `tmp`) |
| Armazenamento compartilhado | `/sdcard/Android/data/<package>`, pelo protocolo de sync do ADB | — |
| Bundle do app | — | O `.app` instalado |
| App groups | — | Todos os contêineres de app group que o app usa |

Os builds de debug do seu app são depuráveis por padrão. Builds de release e a maioria dos apps pré-instalados não são, e
o Android não deixa nenhuma ferramenta ler os dados privados deles. O EmuLabZip avisa isso em vez de mostrar uma pasta
vazia.

Para testar as ferramentas sem um app seu, instale o app de exemplo: `tools/sample-app/install.sh`.

## Visualizadores

| Arquivo | Visualizador |
|------|--------|
| Bancos de dados SQLite | Tabelas e views com contagem de linhas, tipos de coluna e chaves primárias, 100 linhas por página e uma caixa de SQL somente leitura |
| SharedPreferences (`shared_prefs/*.xml`) | Chave, tipo e valor; os sets mostram seus itens |
| Preferences DataStore (`datastore/*.preferences_pb`) | Chave, tipo e valor, decodificados do protobuf |
| Proto DataStore (`datastore/*.pb`) | O protobuf decodificado sem o seu schema: números de campo e valores |
| MMKV (um arquivo em `mmkv/` ao lado do seu `.crc`) | Chave e valor. O MMKV não armazena tipos, então cada valor é mostrado como o tipo que parece ser (string, número, bytes, set) e indica que é um palpite; o hex bruto está a um clique. Um arquivo criptografado é reconhecido e explicado |
| Realm (`*.realm`) | Classes com suas contagens de linhas, tipos de coluna e linhas, lidas uma página por vez com um leitor do formato de arquivo do Realm (versões 20 a 24). Listas, links e timestamps são mostrados; colunas de sets, dicionários, mixed, decimal e UUID ainda não são lidas e informam isso |
| AsyncStorage (`RKStorage`, ou `manifest.json` no iOS) | Chaves e valores, com os valores JSON formatados |
| Core Data (um store SQLite) | Entidades com seus atributos e linhas (sem o prefixo `Z`), ao lado das tabelas brutas |
| Property lists (binárias ou XML) | Formatadas como JSON |
| JSON | Formatado |
| Imagens (PNG, JPEG, GIF, WebP) | Prévia sobre um fundo quadriculado |
| Texto | Como está, até 2 MB |
| Qualquer outra coisa | Os primeiros 4 KB como um hex dump |

Os bancos de dados são copiados para uma pasta temporária privada junto com os arquivos `-wal` e `-shm`, então as
mudanças que o app ainda não gravou com checkpoint ficam visíveis. A cópia é aberta com `query_only` ativado: as
consultas podem ler, mas nunca escrever, e o banco de dados real do app nunca é tocado. As cópias temporárias são
apagadas quando o EmuLabZip é encerrado.

**Save…** copia o arquivo original para um local que você escolher.

![Um arquivo Realm lido do dispositivo: suas classes e as linhas de Person](images/storage-realm.png)

## Alterando e salvando

O Storage é somente leitura até você ativar **Edit**: veja [Alterando os dados de um app](storage-editing.md). **Save
state…**, **Open state…** e o diff de schema estão em [Estado do app](app-state.md).

## Pela linha de comando

```bash
emulabzip files com.example.app                   # the first readable root
emulabzip files com.example.app databases --root data
emulabzip pull com.example.app shared_prefs/settings.xml
emulabzip db com.example.app databases/app.db                     # tables
emulabzip db com.example.app databases/app.db --table users       # rows
emulabzip db com.example.app databases/app.db -q "SELECT count(*) FROM users"
emulabzip edit cell com.example.app databases/app.db -t users --rowid 1 --text name=Ana --yes   # see Changing an app's data
```
