---
source: 0fcc75cc4e31
description: Veja as views na tela com um screenshot, a árvore de views e suas propriedades, e rode uma auditoria de acessibilidade pela interface, CLI ou assistentes.
---
# Layout e acessibilidade

**Layout** mostra as views na tela: um screenshot com uma caixa em volta de cada view, a árvore de views e as
propriedades da que você escolher. A mesma árvore alimenta uma **auditoria de acessibilidade** que procura o que um
leitor de tela ou um polegar tropeçaria.

## A view Layout

Pressione **Capture**. O screenshot e a árvore de views são lidos juntos, então coincidem.

- Passe o ponteiro sobre o screenshot para destacar a view sob ele; clique para selecioná-la. A árvore se abre até ela.
- **Outline every view** desenha todas as caixas de uma vez.
- Pesquise na árvore por texto, label, id ou classe.
- As propriedades mostram a classe, o texto, o label, o id, a posição, o tamanho (em dp no Android) e as flags:
  clickable, focusable, editable, scrollable, disabled, checked.

### De onde vem a árvore

| | Origem |
|---|---|
| Android | `uiautomator dump`, que toda imagem tem. É a árvore de acessibilidade que o TalkBack lê. Nada é instalado. |
| iOS Simulator | A API de Accessibility do macOS na janela do Simulator, que traz a árvore de acessibilidade do iOS (o que o VoiceOver percorre), com labels e identificadores. |

O iOS precisa de duas coisas: a janela do Simulator precisa estar aberta naquele dispositivo, e o programa que executa o
EmuLabZip (o app, ou o seu terminal para a linha de comando) precisa ter permissão em **System Settings, Privacy &
Security, Accessibility**. O EmuLabZip avisa quando isso não acontece. As posições são convertidas para points, então
correspondem ao que o app declara. Dispositivos físicos não têm suporte.

O `uiautomator` não consegue ler telas que o proíbem (janelas seguras, alguns players de vídeo); o EmuLabZip informa
isso em vez de mostrar uma árvore vazia.

## Compose, Flutter, React Native e SwiftUI

As views desenhadas por um framework de UI recebem uma marca na árvore (`compose`, `flutter`, `react-native`,
`swiftui`) e o inspetor mostra o nome do framework. No Android, tudo o que está dentro de uma `ComposeView`, de uma
`FlutterView` ou de uma view raiz do React Native é marcado. No iOS, escolha o app na barra de ferramentas (ou passe
`--app <bundle id>` na linha de comando) e a tela é marcada com o framework daquele app. Veja
[Por dentro dos frameworks](frameworks.md).

## A auditoria

A aba **Accessibility** lista os achados, os mais graves primeiro. Clique em um para ver a view e o que fazer a respeito.

| Regra | Procura | WCAG |
|------|-----------|------|
| `missing-label` | Uma view em que se pode tocar ou digitar e que não tem nome: sem texto, sem label, nada dentro dela que tenha um | 1.1.1, 4.1.2 |
| `unlabeled-focusable` | Uma view que recebe foco e não diz nada | 4.1.2 |
| `touch-target` | Um controle menor que 48 dp (Android) ou 44 pt (iOS) em qualquer direção | 2.5.8 |
| `contrast` | Texto com contraste abaixo de 4,5:1 (3:1 para texto grande), medido a partir dos pixels do screenshot dentro da view | 1.4.3 |
| `duplicate-label` | Controles vizinhos anunciados do mesmo jeito ("Delete", "Delete", "Delete") | 2.4.6 |
| `focus-order` | Foco que volta para cima na tela: o layout lista um controle antes dos que estão acima dele (um leitor de tela e um teclado seguem a ordem do layout). Controles em uma mesma linha não contam como salto; no máximo cinco são informados | 2.4.3 |

O contraste é medido tomando a cor mais comum na view como fundo e a cor mais distante dela como texto. Uma view sobre
uma foto ou um gradiente não é julgada, porque não há um único fundo. Controles desabilitados não são cobrados quanto a
tamanho nem a nome.

### Daltonismo

```bash
emulabzip a11y --simulate deuteranopia -o deuteranopia.png    # protanopia, deuteranopia, tritanopia or achromatopsia
```

Mostra a tela como alguém com aquele tipo de daltonismo a vê (as matrizes de Machado, Oliveira e Fernandes, com
severidade total, aplicadas à luz linear), grava a imagem e lista o **texto que era legível normalmente e deixa de ser**
(`contrast-<kind>`). O texto que já falhava na regra de contraste não é repetido. Via [MCP](mcp.md),
`simulate_color_blindness` devolve a imagem e os achados. A view **Audit** tem o mesmo em *Colour vision*.

### Tamanho do texto, idiomas e texto que não cabe

Veja [Idioma, tamanho do texto e texto que não cabe](localization.md): `i18n check` e `i18n sweep` procuram texto
cortado, fora da tela ou não espelhado, e o sweep informa alvos de toque e ordem de foco que pioram com texto grande.

Esta é uma primeira passada, feita só a partir do layout. Ela não substitui testar o app com o TalkBack ou o VoiceOver,
e não tem como saber que um label está *errado*.

## Linha de comando e assistentes

```bash
emulabzip layout                       # the tree as text
emulabzip layout --json --png screen.png
emulabzip a11y                         # the findings
emulabzip a11y --json
emulabzip a11y --fail-on warning       # exit 1 when something at least this serious is found (note, warning, error)
```

O `a11y --fail-on` faz uma etapa de CI falhar quando uma tela regride. O [servidor MCP](mcp.md) oferece `get_layout` e
`audit_accessibility`, para que um assistente possa olhar uma tela e corrigir o que encontrar.
