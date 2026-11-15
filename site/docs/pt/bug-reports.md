---
source: bde0d784ece0
description: Empacote screenshot, dispositivo, log, requisições com falha e crashes legíveis em um único arquivo HTML e, se quiser, abra uma issue já preenchida.
---
# Relatórios de bug

Um relatório de bug precisa das mesmas coisas toda vez: como a tela estava, qual dispositivo e app, o que o log dizia, quais
requisições falharam e como o app deu crash, de forma legível. O EmuLabZip empacota tudo em **um único arquivo HTML** e, se
você quiser, abre uma nova issue com um resumo já preenchido.

<p align="center">
  <img src="images/bug-report-dialog.png" alt="A janela Report a bug" />
</p>

Nada é enviado a lugar nenhum. Você vê o arquivo e decide para onde ele vai.

## Início rápido

No app desktop, pressione **Report a bug** na barra de ferramentas. Escreva o que aconteceu, escolha o que incluir e
pressione **Save HTML…**. Se você indicar um tracker, a issue pré-preenchida abre no seu navegador e o arquivo é mostrado
na pasta dele, pronto para ser anexado a ela.

Em um terminal:

```bash
emulabzip report --app com.example.shop -o report.html
emulabzip report --app com.example.shop --issue github:owner/repo --open
emulabzip report --session run.ezsession -o report.html       # from a recording
```

## O que há no arquivo

<p align="center">
  <img src="images/bug-report-html.png" alt="O relatório aberto em um navegador: screenshot, dispositivo e o crash legível" />
</p>

| Parte | O que guarda |
|------|---------------|
| Screenshot | A tela como está agora (ou a última de uma sessão), dentro do arquivo |
| Gravação de tela | De uma sessão que tem vídeo: o pedaço que contém o crash, ou o último. Até 20 MB ele faz parte do arquivo; um clipe maior é escrito ao lado dele como `emulabzip-report.mp4`, ao qual o arquivo se refere |
| Faixa da linha do tempo | De uma sessão: o último minuto antes do crash (ou o fim da execução), uma linha por tipo de coisa: toques, requisições, problemas do log, quadros lentos, garbage collections, o crash. Desenhada a partir de números, sem script |
| Dispositivo | O nome, SO, modelo, ABI, memória, tela, idioma e fuso horário |
| Crashes | Os dos últimos 30 minutos, lidos com os mappings, source maps e dSYMs guardados para o app: veja [Stacks legíveis](symbolication.md) |
| Requisições com falha | Requisições que não tiveram resposta ou um status de 400 ou mais, com o erro e o começo da resposta. Elas vêm do proxy em [Network](network.md), então inicie-o e conecte o dispositivo antes |
| Log | Os avisos e erros (erros primeiro quando há muitos) e as últimas linhas |

<p align="center">
  <img src="images/bug-report-video.png" alt="Um relatório feito a partir de uma sessão: o clipe em volta do crash e o último minuto em uma faixa" />
</p>

O arquivo não tem script e não carrega nada de um servidor, então abre igual em qualquer navegador, offline, anos depois.
Tudo nele é escapado, então o texto de um log ou de uma resposta nunca vira marcação. As listas têm limite, e um screenshot
muito grande é deixado de fora com uma nota, para que um relatório continue sendo algo que uma pessoa consegue ler.

Deixe uma parte de fora quando ela não deve sair da sua máquina: `--without screenshot`, `logs`, `requests`, `crashes` ou
`video` (repita para mais de uma), ou as caixas de seleção na janela.

## Abrindo uma issue

`--issue` recebe um tracker:

| Tracker | Escreva como |
|---------|-------------|
| GitHub | `github:owner/repo` |
| GitLab | `gitlab:group/project`, ou `gitlab:git.example.com/group/project` para um servidor próprio |
| Jira | `jira:yoursite.atlassian.net/PROJECTID/ISSUETYPEID`, sendo os dois ids números |

O link abre a página de nova issue do tracker com o título e um resumo preenchidos: a descrição, o app e o dispositivo, o
crash e os primeiros frames dele, as requisições com falha e os primeiros erros do log. O resumo é cortado para caber no
tamanho que os trackers aceitam e termina com uma linha dizendo que o relatório completo está anexado. Você mesmo anexa o
arquivo HTML. Nenhum token é pedido nem guardado, porque o EmuLabZip nunca fala com o tracker: quem fala é o seu navegador,
com o seu próprio login.

## A partir de uma gravação

Uma [sessão](sessions.md) já guarda os logs, requisições, crashes e screenshots de uma execução, então um relatório pode ser
feito a partir de uma depois, ou de uma execução de CI: abra-a em **Sessions** e pressione **Bug report**, ou

```bash
emulabzip report --session run.ezsession -o report.html
```

## Para assistentes

`create_bug_report` escreve o arquivo em `reports/` na pasta de dados do EmuLabZip e devolve o caminho dele, um resumo em
Markdown e, com `issue`, o link. Um relatório com um clipe grande demais para o arquivo ganha uma pasta própria, com o clipe
dentro. Veja [MCP](mcp.md).

## O que conferir antes de compartilhar

Um relatório guarda o que estava na tela e no log do seu app, e os endereços das requisições que falharam. Um clipe da tela
pode mostrar qualquer coisa que estava nela. Leia e assista antes de ele ir para uma issue que outros possam ver, e use
`--without` para o que deve ficar de fora. Veja
[Segurança](../SECURITY.md).
