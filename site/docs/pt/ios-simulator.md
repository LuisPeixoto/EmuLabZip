---
source: 3fbc535a9ed8
description: Como o EmuLabZip inspeciona o iOS Simulator com o xcrun simctl, o que ele coleta, como lidar com o volume do log unificado e quais são as limitações atuais.
---
# iOS Simulator

O iOS Simulator tem suporte no macOS com o Xcode instalado. No Windows e no Linux, o backend de iOS se declara
indisponível e o restante do EmuLabZip funciona normalmente.

## Como ele se conecta

O EmuLabZip usa a ferramenta documentada `xcrun simctl`. Os argumentos são passados diretamente ao processo, nunca por
um shell. O Instruments usa frameworks privados para conversar com os simuladores; o EmuLabZip deliberadamente não usa,
para continuar funcionando entre versões do Xcode. Veja [Como as ferramentas oficiais funcionam](research/how-the-official-tools-work.md).

## O que é coletado

| Recurso | Origem |
|---------|--------|
| Lista de dispositivos | `simctl list devices -j`, apenas simuladores iOS inicializados |
| Overview | JSON do simulador, `simctl list runtimes -j`, `defaults read -g AppleLocale` / `AppleLanguages` dentro do simulador, `sw_vers` e `xcodebuild -version` no host |
| Apps | `simctl listapps` (convertido com `plutil`), ids de processo de `launchctl list` dentro do simulador |
| Logs | `simctl spawn <udid> log stream --style ndjson --level debug` |
| Screenshots | `simctl io <udid> screenshot` |

## O log unificado é movimentado

Um simulador escreve facilmente vários milhares de linhas por segundo no nível debug. O EmuLabZip agrupa as linhas a
cada 100 ms e mantém um buffer limitado (50.000 linhas por padrão), para que a interface continue responsiva. Para ver
menos:

- Escolha o seu app e ative o filtro de app em Logs.
- Aumente o nível mínimo para **I** ou **W**.
- Desative *Verbose and debug lines* em Settings para pedir ao `log stream` apenas info e acima.

As linhas de log do iOS trazem o **nome do processo** como tag e o **subsystem e a category** (por exemplo
`com.apple.network:connection`), que você vê ao clicar em uma linha.

## Limitações

- O `log stream` não tem histórico, então o log começa quando o dispositivo é selecionado.
- Dispositivos iOS físicos ainda não têm suporte (veja o [roadmap](roadmap.md)).
