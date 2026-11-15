---
source: e1dcec842375
description: Guarde uma imagem de como uma tela deve ser (a baseline) e veja o que mudou quando ela é diferente, com limite de tolerância, regiões ignoradas e CI.
---
# Regressão visual

<p align="center">
  <img src="images/visual-diff.png" alt="A view Visual: uma tela comparada com a sua baseline, com a área alterada desenhada em vermelho" />
</p>

Guarde uma imagem de como uma tela deve ser (uma **baseline**) e veja o que mudou quando ela não é assim.

```bash
emulabzip visual capture home --status-bar          # save the screen as a baseline
emulabzip visual compare home --status-bar          # exit 1 when more than the limit changed; draws the difference
emulabzip visual update home --yes --status-bar     # accept the screen as the new baseline
emulabzip visual list
emulabzip visual diff a.png b.png -o diff.png       # two screenshot files
emulabzip visual session a.ezsession b.ezsession -o diffs/    # the screenshots of two recordings, in order
```

As baselines são arquivos PNG na pasta `visual/` do projeto (ou em `[visual] baseline_dir`), com nomes de letras, dígitos,
`.`, `_` e `-`. Uma baseline **nunca é substituída por acidente**: `capture` recusa um nome que já existe, e `update`
precisa de `--yes`.

## O que conta como mudança

Cada pixel é comparado com a sua baseline, canal por canal. Um pixel conta como alterado quando um canal difere por mais do
que a **tolerância** (8 de 255 por padrão, o que absorve anti-aliasing e arredondamento do encoder). A comparação informa
quantos pixels mudaram, a porcentagem da tela e o menor retângulo que contém todas as mudanças. A diferença é desenhada em
um PNG, com os pixels alterados em vermelho sobre uma cópia esmaecida da tela. O limite é uma porcentagem (`--threshold`,
0,1 por padrão).

Telas de tamanhos diferentes não são comparadas: vêm de dispositivos, orientações ou tamanhos de tela diferentes, e o erro
diz isso.

## O relógio e a bateria

A barra de status muda a cada minuto. `--status-bar` mostra uma limpa para a captura (9:41, bateria e sinal cheios) e
depois restaura a real, para que esses pixels sejam os mesmos toda vez. Qualquer outra coisa que se mexe (um anúncio, um
vídeo, uma data) pode ficar fora da comparação com regiões `ignore`.

## No `emulabzip.toml`

```toml
[visual]
threshold_percent = 0.1          # how much of the screen may change
tolerance = 8                    # how far a colour channel may differ before a pixel counts
baseline_dir = "ui/baselines"    # relative to the project; default "visual"

[[visual.ignore]]                # parts of the screen to leave out, in pixels
name = "clock"
x = 0
y = 0
width = 1080
height = 120
```

## No `ci`

```bash
emulabzip ci -o run.ezsession --visual home --fail-on-visual -- ./gradlew connectedCheck
```

Depois do comando, a tela é comparada com a baseline `home`, o resultado vai para o resumo em Markdown e, com
`--fail-on-visual`, a execução falha acima do limite. A baseline é lida antes de qualquer coisa rodar.

## Entre dispositivos

Compare dois screenshots de dois dispositivos com `visual diff`, ou capture em um e use `compare -d <other>` em outro. Eles
precisam ter o mesmo tamanho em pixels.

## No desktop

A view **Visual** lista as baselines (guardadas na pasta `visual` do próprio EmuLabZip), salva a tela como uma, compara e
mostra a diferença, a baseline ou a tela de agora. **Update** pergunta antes de substituir uma imagem.

## Para assistentes

Por [MCP](mcp.md), `visual_compare` devolve os números e a imagem da diferença. Ele não cria nem substitui uma baseline.

## Limites

- Uma baseline é um screenshot, então pode conter qualquer coisa que estava na tela. Não faça commit de uma de uma tela com
  dados pessoais.
- Ele compara pixels, não significado: um deslocamento de um pixel em tudo muda tudo. Ajuste a tolerância e o limite ao seu
  gosto e mantenha a barra de status limpa.
