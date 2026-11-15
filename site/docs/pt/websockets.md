---
source: ad59e3a2fe37
description: Veja como o EmuLabZip lê o tráfego WebSocket como mensagens, na aba Messages, e quais são os limites de mensagens guardadas e do export HAR.
---
# Mensagens WebSocket

A ferramenta [Network](network.md) lê o tráfego WebSocket como mensagens, não como bytes. Abra uma requisição WebSocket e a
aba **Messages** lista o que foi em cada direção, com o tempo desde que a conexão foi aberta.

- **Setas** mostram a direção: para cima vai do app para o servidor, para baixo volta.
- Mensagens de texto aparecem como texto, e o JSON é formatado quando você abre uma. Mensagens binárias mostram um hex dump.
- O filtro procura dentro das mensagens de texto.
- Frames de controle (ping, pong, close) também são listados.

## Como é lido

O proxy continua copiando cada byte entre o app e o servidor sem alterar nada e apenas escuta. Ele segue a RFC 6455:
frames com tamanhos de 7, 16 e 64 bits, máscaras, mensagens fragmentadas juntadas em uma só, frames de controle no meio
delas e `permessage-deflate` quando os dois lados concordaram com ele (inclusive `no_context_takeover`). Se o que passa
não for WebSocket, a leitura para em silêncio e a conexão segue normalmente.

## Limites

Um fluxo guarda até 1.000 mensagens e 64 KB de cada uma; a aba informa quantas mensagens posteriores não foram guardadas e
quando uma mensagem foi cortada. As contagens continuam. O [export HAR](network.md#export) inclui as mensagens em
`_webSocketMessages`, como o Chrome faz.
