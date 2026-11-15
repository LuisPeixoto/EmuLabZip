---
title: Teste seu app em alemão, árabe e com texto a 200%, automaticamente
description: Encontre texto cortado, fora da tela ou sem espelhamento em vários idiomas e tamanhos de fonte com um comando, e trave a tela com regressão visual no EmuLabZip.
tags: android, i18n, testing, accessibility
series: EmuLabZip (PT)
cover: docs/images/audit-text.png
publish_after: 2026-12-08
pair: 08-i18n-sweep
lang: pt
---

Texto é onde o layout quebra. Uma palavra em alemão é maior que a em inglês, o árabe corre da direita para a esquerda, e quem
precisa de texto grande coloca a fonte em 200%. A maioria dos times descobre esses bugs por um screenshot num relatório de bug,
porque testar toda tela em todo idioma na mão não é a ideia de diversão de ninguém.

O EmuLabZip muda o idioma e o tamanho do texto do dispositivo, verifica se a tela tem texto que não cabe e varre uma tela por
vários idiomas e tamanhos com um comando. Este post mostra como, e como evitar que o resultado regrida.

![A view Audit encontrando um rótulo grande demais para a sua linha](docs/images/audit-text.png)

## Mude o dispositivo

Pseudo-locales são o teste mais barato: `en-XA` é texto longo e acentuado, e `ar-XB` é da direita para a esquerda. O Android
muda o idioma de um app, não do dispositivo inteiro, então você informa o app:

```bash
emulabzip control locale en-XA --app com.example.shop
emulabzip control locale ar-XB --app com.example.shop
emulabzip control reset-locale --app com.example.shop
emulabzip control font-scale 130        # percent of normal; 100 puts it back
```

Abra o app em cada um e veja o que quebra. Dá para fazer esse teste mesmo antes de existir qualquer tradução, desde que as
strings do app estejam em resources: texto fixo no código continua como está.

## Verifique a tela

```bash
emulabzip i18n check
emulabzip i18n check --rtl
```

O `i18n check` lê o layout e o screenshot da tela como ela está agora e reporta:

| Finding | Significado |
|---|---|
| `text-off-screen` | Texto que passa da borda da tela (a menos que esteja dentro de algo que rola) |
| `text-clipped-by-parent` | Texto que não cabe no seu contêiner, então o contêiner o corta |
| `text-cut-off` | Uma única linha de texto cuja tinta chega ao fim da caixa: provavelmente está cortada ou termina em reticências |
| `rtl-not-mirrored` | Num idioma da direita para a esquerda (`--rtl`), a maior parte do texto da tela continua à esquerda |

`text-cut-off` é uma heurística: o layout diz onde está cada view, e o screenshot diz se o texto vai até o fim da caixa. A
mensagem diz "pode estar", e um rótulo que apenas se ajusta ao seu texto não é reportado.

## Varra uma tela

```bash
emulabzip i18n sweep --app com.example.shop --locales en-XA,ar-XB --scales 100,130,200
```

A varredura testa cada idioma em cada tamanho de texto, lê o layout e imprime uma tabela: quantos findings, quais, e **quais
são piores que o dispositivo como ele está agora**. O dispositivo volta ao normal (tamanho do texto e idioma) quando termina, e
também quando um passo falha. O `--fail-on-worse` sai com 1 quando alguma configuração é pior, o que o torna usável em CI. A
200%, ele também reporta alvos de toque e ordem de foco que pioraram. No Simulador iOS só os tamanhos de texto são varridos,
porque um app só assume um novo idioma quando é iniciado de novo.

## Fixe o resultado com regressão visual

Quando a tela estiver certa, guarde uma imagem dela:

```bash
emulabzip visual capture home --status-bar
emulabzip visual compare home --status-bar
emulabzip visual update home --yes --status-bar
```

O `capture` salva a tela como um PNG de baseline, o `compare` sai com 1 quando mais que o limite mudou e desenha a diferença em
vermelho, e o `update` aceita a nova tela. Uma baseline nunca é substituída por acidente: o `capture` recusa um nome que já
existe, e o `update` exige `--yes`. O `--status-bar` mostra uma barra de status limpa (9:41, bateria cheia) durante a captura e
depois recoloca a real, para o relógio não mudar a cada minuto. Um pixel conta como alterado quando um canal difere mais que uma
tolerância (8 de 255 por padrão), e qualquer outra coisa que se mexa, como um anúncio ou uma data, pode ficar de fora com
regiões `ignore` no `emulabzip.toml`.

No CI é só mais uma flag: `emulabzip ci ... --visual home --fail-on-visual -- ./gradlew connectedCheck`.

## E acessibilidade, na mesma tela

```bash
emulabzip a11y --fail-on error
emulabzip a11y --simulate deuteranopia -o deuteranopia.png
```

O `a11y` encontra rótulos ausentes, alvos de toque pequenos e texto com baixo contraste, cada um com a view e o critério WCAG, e
o `--simulate` mostra a tela como alguém com daltonismo a enxerga.

## Limites

- As heurísticas enxergam views, não glifos: uma fonte de ícones, um rótulo desenhado à mão ou o texto de um jogo não são avaliados.
- O Android muda o idioma por app, não por dispositivo, e idiomas no dispositivo todo exigem root.

## Experimente

```bash
cargo install --git https://github.com/LuisPeixoto/EmuLabZip emulabzip-cli
```

- Idioma, tamanho do texto e texto que não cabe: [emulabzip.luispeixoto.dev/pt/docs/localization/](https://emulabzip.luispeixoto.dev/pt/docs/localization/)
- Regressão visual: [emulabzip.luispeixoto.dev/pt/docs/visual-regression/](https://emulabzip.luispeixoto.dev/pt/docs/visual-regression/)
- Layout e acessibilidade: [emulabzip.luispeixoto.dev/pt/docs/layout/](https://emulabzip.luispeixoto.dev/pt/docs/layout/)
- Site e toda a documentação: [emulabzip.luispeixoto.dev](https://emulabzip.luispeixoto.dev/pt/)
- Código-fonte (MIT): [github.com/LuisPeixoto/EmuLabZip](https://github.com/LuisPeixoto/EmuLabZip)
