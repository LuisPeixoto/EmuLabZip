---
source: 06c0e7164971
description: Edite linhas SQLite, SharedPreferences e property lists do app no dispositivo com confirmação, backup, verificação e desfazer, pela interface ou pela CLI.
---
# Alterando os dados de um app

<p align="center">
  <img src="images/storage-edit.png" alt="O Storage no modo de edição: um valor da tabela cart está sendo alterado" />
</p>

O [Storage](storage.md) é somente leitura até você ativar **Edit**. Editar altera os arquivos reais do app no
dispositivo, então foi feito com cuidado: cada alteração pergunta antes, para o app, guarda o arquivo que substitui, lê
o novo arquivo de volta para conferir e pode ser desfeita.

## O que pode ser alterado

| Arquivo | Desktop | Linha de comando |
|------|---------|--------------|
| Linhas SQLite | Clique duas vezes em uma célula; **New row**; a lixeira em uma linha | `edit cell`, `edit insert`, `edit delete` |
| SharedPreferences | Clique duas vezes em um valor; **New key**; a lixeira | `edit pref-set`, `edit pref-remove` |
| Property lists (iOS) | — | `edit plist-set`, `edit plist-remove` |

DataStore, MMKV, AsyncStorage, Realm e Core Data são somente leitura: seus formatos não são gravados de volta.

```bash
emulabzip edit cell com.example.shop databases/shop.db -t customers --rowid 1 --text name=Ana --set vip=1 --yes
emulabzip edit insert com.example.shop databases/shop.db -t customers --set name=Bo --yes
emulabzip edit delete com.example.shop databases/shop.db -t cart --pk id=3 --yes
emulabzip edit pref-set com.example.shop shared_prefs/settings.xml theme dark --yes
emulabzip edit pref-set com.example.shop shared_prefs/settings.xml launches 4 --as int --yes
emulabzip edit plist-set com.example.shop Library/Preferences/com.example.shop.plist flags/beta true --as boolean --yes
emulabzip edit undo <data folder>/states/edits/<folder> --yes      # the folder the edit printed
```

Um valor é um número inteiro ou decimal quando parece um e texto nos demais casos; `--text column=value` força texto e
`--null column` define NULL. Uma preferência do tipo set é formada por seus itens separados por vírgulas. Todo comando
precisa de `--yes`.

## O que uma edição faz

1. Verifica a alteração **antes** de tocar no dispositivo: a tabela e a coluna existem, uma linha corresponde à chave, o
   valor tem o tipo que foi pedido.
2. Para o app, para que ele não sobrescreva a alteração quando encerrar.
3. Guarda os bytes que está prestes a substituir em uma pasta de backup (`states/edits/` na pasta de dados do
   EmuLabZip).
4. Grava: no SQLite, o banco de dados e seu write-ahead log são baixados, a alteração é feita em uma cópia dentro de uma
   transação que precisa mudar **exatamente uma linha**, com foreign keys ativadas, o log é incorporado ao arquivo
   (`wal_checkpoint(TRUNCATE)`), o arquivo é enviado, e o `-wal` e o `-shm` do dispositivo são removidos para que o log
   antigo não seja reproduzido sobre o novo arquivo.
5. Lê o arquivo de volta. Se não for o que foi gravado, o antigo é restaurado e a edição falha.

Uma linha é encontrada por `rowid` ou, em tabelas criadas como `WITHOUT ROWID`, pela chave primária. Uma tabela sem
chave utilizável é mostrada com o motivo e não pode ser editada. Views não podem ser editadas.

No Android, o arquivo é gravado por meio do `run-as`, então o app continua sendo o dono dele; no iOS, é uma gravação de
arquivo no host que recusa um caminho cujos links simbólicos saem do contêiner.

## Desfazer

O app desktop tem **Undo last change**. Na linha de comando, cada edição imprime a pasta a ser passada para o
`edit undo`. Desfazer para o app e devolve todos os arquivos daquela edição ao lugar, ou remove o arquivo se ele não
existia.

Para qualquer coisa maior, [salve o estado do app](app-state.md) antes.

## Para assistentes

A edição **não** é oferecida via [MCP](mcp.md).
