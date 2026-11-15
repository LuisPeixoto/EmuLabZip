---
source: d4619f203dc1
description: Toque, digite e pressione teclas em um dispositivo Android, grave o que você faz em um fluxo, reproduza-o e exporte para Maestro ou para um shell script.
---
# Entrada e fluxos

O EmuLabZip pode tocar em um dispositivo, digitar nele e pressionar as teclas dele; pode **gravar** o que você faz em um
**fluxo**, reproduzir o fluxo de novo e exportá-lo como um fluxo do [Maestro](https://maestro.mobile.dev) ou como um shell
script. Tudo isso é para **Android**: o iOS não tem jeito público de enviar toques nem de lê-los.

## Tocar, digitar e pressionar

| Controle | CLI | O que faz |
|---------|-----|--------------|
| Tap | `emulabzip control tap <x> <y>` | Um toque em um ponto da tela, em pixels, como em um screenshot. |
| Swipe | `emulabzip control swipe <x1> <y1> <x2> <y2> [--ms 300]` | Um arrasto. De um ponto para ele mesmo, é um toque longo. |
| Text | `emulabzip control text "hello"` | Digita no campo que tem o foco. Somente ASCII imprimível. |
| Key | `emulabzip control key back` | `back`, `home`, `app_switch`, `enter`, `tab`, `delete`, `volume_up`… |
| Launch | `emulabzip control launch <app>` | Inicia um app como o launcher faria. Também no iOS. |

As posições são verificadas (no máximo 20000 pixels), o texto é verificado (não pode conter `%s`, que o Android lê como um
espaço, nem nada que `input text` não consiga digitar), e as teclas são uma lista de nomes, então nada chega a um shell sem
verificação. Os mesmos controles são [ferramentas MCP](mcp.md): `tap`, `swipe`, `input_text`, `press_key`, `launch_app`.

No app, **Overview → Screen → Interact** torna a imagem da tela tocável: um clique é um tap, um arrasto é um swipe e um
toque mantido por meio segundo é um toque longo. **Controls** tem um card para texto e teclas.

## Gravando um fluxo

```bash
emulabzip flow record --name "Add to cart" --app com.example.shop     # touch the emulator, then press Ctrl+C
```

Toque no próprio dispositivo: em um emulador, na janela dele. O EmuLabZip lê o touchscreen (`getevent`) e guarda cada tap,
toque longo e swipe, e as pausas entre eles (uma pausa de um segundo ou mais, arredondada para o décimo de segundo mais
próximo). O fluxo começa com um passo que inicia o app, a menos que você passe `--no-launch`.

Duas coisas não são gravadas, e o app avisa:

- **Toques enviados pelo EmuLabZip ou por `adb shell input`.** Eles entram mais acima no Android e nunca chegam ao
  touchscreen, então clicar na imagem do Interact ou reproduzir um fluxo durante a gravação não adiciona passos.
- **Texto digitado.** O teclado não é o touchscreen. Adicione um passo de texto depois.

Depois de cada toque, a tela é lida de novo em segundo plano, para que o próximo tap possa lembrar **no que acertou**: o id
da view (um `testTag` do Compose, um resource id do Android) ou o texto dela. Ler uma tela leva dois ou três segundos, então
um tap que vem antes disso depois de outro é mantido como um ponto.

Os fluxos são arquivos JSON na pasta `flows` do EmuLabZip, compartilhada pelo app e pela linha de comando:

```bash
emulabzip flow list
emulabzip flow show add-to-cart          # the steps; --json for the file
emulabzip flow play add-to-cart          # Ctrl+C stops it
emulabzip flow remove add-to-cart
```

Ao reproduzir, os pontos são ajustados à escala da tela em que o fluxo roda, quando o tamanho dela difere do tamanho em que
o fluxo foi gravado. Cada passo é seguido por um terço de segundo, para que a tela possa reagir.

![A view Automation com um fluxo gravado](images/automation.png)

## A view Automation

**Automation** na barra lateral grava e reproduz fluxos, lista os salvos e mostra os passos do que você escolher. Você pode
apagar um passo, ou adicionar um texto, uma tecla ou uma pausa à mão, e depois **Save changes**. **Maestro** e **Script**
salvam o fluxo nesses formatos.

## Exportar

```bash
emulabzip flow export add-to-cart --maestro > add-to-cart.yaml
emulabzip flow export add-to-cart --script --out add-to-cart.sh
```

**Maestro.** Um tap que conhece a sua view diz `tapOn: id: "checkout"` ou `tapOn: "Checkout"`; os outros dizem um ponto como
porcentagem da tela (`point: "50%,80%"`). O texto é escapado para as expressões regulares do Maestro e depois entre aspas
para YAML. As teclas usam os nomes do Maestro (`Back`, `Home`, `Enter`, `Backspace`…), e uma tecla para a qual ele não tem
nome é deixada como comentário. As pausas viram `waitForAnimationToEnd`. O arquivo não tem asserções: adicione-as, pois uma
gravação sabe o que você fez, não o que ela deveria mostrar.

**Script.** Um shell script de linhas `emulabzip control …` com `sleep`s entre elas, para o dispositivo em
`EMULABZIP_DEVICE`.

## Limites

- Somente Android. Um fluxo não pode ser gravado nem reproduzido em um simulador.
- Os rótulos são o melhor esforço. Uma tela que muda sem um toque (um timer, uma resposta de rede) pode dar a um tap o nome
  do que estava ali um instante antes; o ponto é sempre mantido, e o Maestro usa o rótulo só quando há um.
- Um fluxo tem no máximo 5000 passos, e uma pausa no máximo um minuto. O `play_flow` de um assistente recusa fluxos que
  levam mais de dois minutos; reproduza-os com `emulabzip flow play`.
