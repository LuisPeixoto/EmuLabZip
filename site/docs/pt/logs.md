---
source: dfe16053c631
description: "O visualizador de logs acompanha o logcat no Android e o log unificado no iOS: filtros, leitura, pausa, exportação e limites de linhas explicados."
---
# Logs

O visualizador de logs acompanha o **logcat** no Android e o **log unificado** no iOS em uma única tabela com as mesmas
colunas: hora, id do processo, nível, tag e mensagem.

## Filtros

| Controle | O que faz |
|---------|--------------|
| **V D I W E F** | Nível mínimo. Clicar em **W** mostra avisos, erros e linhas fatais. |
| **Tag** | Linhas cuja tag contém o texto (sem diferenciar maiúsculas de minúsculas). |
| **Search** | Linhas cuja mensagem ou tag contém o texto. Pressione <kbd>⌘F</kbd> para ir até aqui. |
| **.\*** | Trata a pesquisa como uma expressão regular, por exemplo `skipped \d+ frames`. Uma expressão inválida deixa o campo vermelho. |
| **App** | Apenas linhas escritas pelo app escolhido na barra de ferramentas. |

No iOS, as mensagens `Default` aparecem como **I** e as mensagens `Fault` como **F**. No Android, `A` (assert) aparece
como **F**.

## Leitura

- A lista acompanha as novas linhas enquanto você está no fim. Role para cima para parar de acompanhar; **Jump to
  latest** leva você de volta.
- **Pause** congela a visualização sem interromper o fluxo. As linhas que chegam nesse meio-tempo aparecem quando você
  retoma.
- Clique em uma linha para ver a mensagem inteira, o id da thread e (no iOS) o subsystem. Clique duas vezes em uma linha
  para copiá-la.

## Exportação

O botão de download exporta **as linhas que você vê no momento**, depois de filtradas, como texto simples no layout
`threadtime` do logcat ou como JSON lines (um objeto por linha). A CLI faz o mesmo com `emulabzip logs --json`.

## Limites

Por padrão, o visualizador mantém as 50.000 linhas mais recentes; mude isso em Settings. Quando as linhas chegam mais
rápido do que a interface consegue exibi-las, a barra de status conta as linhas descartadas em vez de esconder o fato.
