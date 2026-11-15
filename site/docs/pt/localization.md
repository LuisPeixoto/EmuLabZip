---
source: 685bcdfe2f24
description: Mude o idioma e o tamanho do texto de um dispositivo, encontre texto que não cabe na tela e teste várias combinações de idioma e tamanho com um comando.
---
# Idioma, tamanho do texto e texto que não cabe

<p align="center">
  <img src="images/audit-text.png" alt="A view Audit encontrando um rótulo comprido demais para a sua linha" />
</p>

O texto é onde os layouts quebram: uma palavra em alemão é mais longa, o árabe corre da direita para a esquerda, e quem
precisa de texto grande o coloca em 200%. O EmuLabZip pode **mudar** o idioma e o tamanho do texto de um dispositivo,
**verificar** se há na tela texto que não cabe e **varrer** uma tela por vários idiomas e tamanhos em um só comando.

## Mudando o dispositivo

```bash
emulabzip control locale en-XA --app com.example.shop   # long, accented text
emulabzip control locale ar-XB --app com.example.shop   # right to left
emulabzip control reset-locale --app com.example.shop
emulabzip control font-scale 130                        # percent of normal; 100 puts it back
emulabzip control display --width 720 --height 1600 --density 320     # Android
emulabzip control reset-display
```

| | Android | iOS Simulator |
|---|---------|---------------|
| Idioma | `cmd locale set-app-locales`: **um app por vez**, Android 13 e mais novos. Os pseudo-locales `en-XA` e `ar-XB` funcionam. O app é avisado e recria a tela dele | `defaults write -g AppleLanguages` e `AppleLocale` para o **simulador inteiro**; um app assume o idioma **quando inicia de novo** |
| Tamanho do texto | `settings put system font_scale` | `simctl ui … content_size`: o tamanho de Dynamic Type mais próximo da porcentagem (130% é `extra-extra-extra-large`, 200% é `accessibility-extra-large`) |
| Tamanho e densidade da tela | `wm size`, `wm density` | Não disponível |

Uma tag de idioma tem letras, dígitos e hifens (`de-DE`, `pt-BR`, `zh-Hans-CN`). O tamanho do texto vai de 50 a 300 por
cento. Tudo isso também está na view **Controls**, em *Text size and language* e *Display size*.

## Verificando uma tela

```bash
emulabzip i18n check            # the screen as it is now
emulabzip i18n check --rtl      # it is shown in a right-to-left language: also check the layout was mirrored
```

| Regra | Procura |
|------|-----------|
| `text-off-screen` | Texto que passa da borda da tela (a menos que esteja em algo que rola) |
| `text-clipped-by-parent` | Texto que não cabe dentro do seu contêiner, que então o corta |
| `text-cut-off` | Uma única linha de texto presa ao espaço que recebeu, cuja tinta chega ao fim da sua caixa: provavelmente está cortada ou termina em reticências |
| `rtl-not-mirrored` | Em um idioma da direita para a esquerda, a maior parte do texto da tela ainda à esquerda |

`text-cut-off` é uma heurística. O layout diz onde está cada view, e o screenshot diz se o texto dela vai até o fim da
caixa. Um rótulo que apenas abraça o seu texto não é reportado (a caixa dele só é tão larga quanto o texto), e um rótulo só
é reportado quando a caixa dele termina onde o contêiner, ou a próxima view da linha, não deixa espaço. A mensagem diz "may
be". Sem um screenshot, só as regras de layout rodam.

## Varrendo

```bash
emulabzip i18n sweep --app com.example.shop --locales en-XA,ar-XB --scales 100,130,200
```

A varredura testa cada idioma em cada tamanho de texto, lê o layout e imprime uma tabela: quantos achados, quais e **quais
são piores do que o dispositivo como está agora**. O dispositivo é restaurado (tamanho do texto e idioma) quando termina, e
também quando um passo falha. `--fail-on-worse` sai com 1 quando alguma configuração é pior. No iOS Simulator só os tamanhos
de texto são varridos, porque um app só assume um novo idioma quando inicia de novo.

A escala do texto também é examinada pela varredura: em 200% ela reporta alvos de toque e ordem de foco que pioraram.

## Para assistentes

Por [MCP](mcp.md): `control_locale`, `control_font_scale`, `control_display_size` e `check_text`.

## Limites

- O Android muda o idioma de um app, não o do dispositivo. Idiomas para o dispositivo todo exigem root.
- As heurísticas veem views, não glifos. Uma fonte de ícones, um rótulo desenhado à mão ou texto em um jogo não é avaliado.
- Os pseudo-locales dependem de o app ter as strings dele em resources; texto fixo no código fica como está.
