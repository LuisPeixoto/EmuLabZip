---
source: 2240302ecefc
description: Salve todo o estado de um app (bancos, preferências, arquivos) em um .ezstate, restaure-o quando quiser e compare a estrutura entre dois salvamentos.
---
# Estado do app: salvar, restaurar e comparar

<p align="center">
  <img src="images/state-compare.png" alt="Um estado salvo aberto no Storage, comparado com outro: uma coluna, um índice e uma tabela foram adicionados" />
</p>

O estado de um app é o que ele guarda: bancos de dados, preferências, arquivos. **Save state** põe tudo isso em um único
arquivo `.ezstate`; **Restore** o devolve; **compare** diz o que mudou na estrutura entre dois salvamentos. É o que você
precisa para repetir um bug com os mesmos dados toda vez, para testar uma migração e para começar um teste de um ponto
conhecido.

```bash
emulabzip state save com.example.shop -o v1.ezstate
# …use the app, or install the new version…
emulabzip state save com.example.shop -o v2.ezstate
emulabzip state diff v1.ezstate v2.ezstate
emulabzip state restore v1.ezstate --yes
```

## O que é salvo

| | Android | iOS Simulator |
|---|---------|---------------|
| O quê | A pasta privada do app (`/data/data/<package>`), exceto `cache`, `code_cache` e `lib`; com `--external`, também a pasta dele no armazenamento compartilhado | O contêiner de dados (`Documents`, `Library`, `tmp`) |
| Como | `run-as <package> tar` (o app precisa ser **debuggable**, como para o [Storage](storage.md)) | Um tar do contêiner, escrito pelo EmuLabZip |
| O app | É parado antes, para que nenhum arquivo fique escrito pela metade | É encerrado antes |

Um arquivo `.ezstate` é um zip: `state.json` (o app, a versão dele, o dispositivo, a hora), o tar e `schemas.json`, uma
descrição de cada banco SQLite (tabelas, colunas, índices) e de cada arquivo de preferências, DataStore e property list
(chaves e tipos). Essa descrição é o que torna a comparação rápida: nada precisa ser desempacotado.

## Restaurando

`state restore` substitui os dados do app pelo que está no arquivo. Ele:

1. recusa um arquivo feito em outra plataforma e avisa quando o app é de uma versão diferente da de onde o estado veio;
2. precisa de `--yes` (o app desktop pergunta antes);
3. **salva os dados que substitui** em um arquivo em `states/auto/` da pasta de dados do EmuLabZip e imprime o nome dele.
   Restaurar esse arquivo desfaz a restauração. Se o backup não puder ser feito, nada é substituído;
4. para o app, esvazia a pasta do app (mantendo `lib` no Android), desempacota o tar e diz se algo falhou.

Cada entrada de um tar é verificada antes de qualquer coisa ser escrita: sem caminhos absolutos, sem `..`, sem hard links,
sem dispositivos e sem link simbólico que aponte para fora da pasta. Um arquivo `.ezstate` de outra pessoa não consegue
escrever fora do app.

O Android executa o desempacotamento como o app (`run-as`), então os arquivos continuam tendo o app como dono.

## O diff de schema

Uma linha por mudança: `+` adicionado, `-` removido, `~` alterado, depois o arquivo, o que é e o nome dele, e o detalhe.

```
$ emulabzip state diff v1.ezstate v2.ezstate
+ databases/shop.db: column cart.note  TEXT
+ databases/shop.db: index idx_cart_sku  CREATE INDEX idx_cart_sku ON cart (sku)
+ databases/shop.db: table orders  3 columns
```

Ele lista tabelas, colunas (e o tipo delas, `NOT NULL` e chave primária), índices, e as chaves e os tipos de arquivos de
preferências, DataStore e property list, adicionados, removidos ou alterados. O conteúdo das linhas não é comparado: isto é
para verificar uma migração, não para fazer diff de dados. `state diff v1.ezstate` sem um segundo arquivo compara com o app
como está agora. `--json` o entrega como dados.

## No app desktop

O Storage tem **Save state…** e **Open state…** na barra de ferramentas. Um estado aberto mostra o que há dentro e oferece
**Compare with the app now**, **Compare with another file…** e **Restore…**. O Restore pergunta antes e diz onde os dados
antigos ficam guardados.

## Para assistentes

Por [MCP](mcp.md), `save_state` e `diff_state` são oferecidos, e `list_states` mostra os nomes salvos. Restaurar **não** é:
um assistente pode ver o que mudou, não substituir os dados de um app.

## Limites

- O Android precisa de um app debuggable. A pasta privada de um build release não pode ser lida por nenhuma ferramenta.
- Dados mantidos só na memória, o keychain e o que outros apps guardam para este (contas, content providers) não fazem
  parte da pasta, então não fazem parte do estado.
- iOS: contêineres de app group não são salvos.
- Um estado não inclui o app em si: instale a mesma versão antes de restaurar.
