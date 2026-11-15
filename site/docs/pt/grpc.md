---
source: 8c8e10338584
description: Veja chamadas gRPC na ferramenta Network por método e status, com streaming, trailers e mensagens protobuf lidas como JSON, com ou sem arquivos .proto.
---
# gRPC e protobuf

![Uma chamada gRPC em Network, com seu status e a mensagem protobuf lida como JSON](images/network-grpc.png)

As chamadas gRPC aparecem em **Network** pelo método que chamam, com o status que o servidor respondeu.

- A coluna **Operation** mostra `Service/Method`, tirado do caminho.
- O gRPC diz como uma chamada terminou nos **trailers**, cabeçalhos enviados depois do corpo (`grpc-status`, `grpc-message`). Eles são
  registrados e mostrados na aba **Headers**, e o status aparece na lista como `OK`, `UNAVAILABLE`, `NOT_FOUND`…
- As chamadas passam pelo proxy em streaming conforme acontecem, então chamadas de streaming funcionam, e uma chamada longa mostra suas mensagens
  conforme chegam. Um breakpoint de requisição não consegue segurar uma chamada gRPC, porque precisa da mensagem inteira. As regras ainda podem atrasá-la,
  bloqueá-la ou respondê-la.
- Só o gRPC sobre TLS (HTTP/2 negociado por ALPN) é decifrado, como todo HTTPS. O HTTP/2 em texto puro (`h2c`) não é
  suportado.

## Lendo as mensagens

Um corpo gRPC é uma série de mensagens, cada uma em protobuf. As abas **Request** e **Response** as listam como JSON, de uma de
duas maneiras:

- **Sem um schema.** O protobuf na rede tem números de campo, não nomes. O EmuLabZip os lê mesmo assim: `{"1": "hello",
  "2": 150, "3": {"1": 5}}`. Texto é mostrado como texto, uma mensagem aninhada como objeto, um campo repetido como lista e
  outros bytes como hexadecimal. Uma mensagem que não é protobuf diz isso.
- **Com os seus arquivos `.proto`.** Em **Network → Network → gRPC and protobuf**, escolha a pasta que os contém (aquela a que os
  caminhos de `import` são relativos). Eles são compilados em memória, sem precisar do `protoc`. O método da chamada passa a dizer
  quais tipos de mensagem são a sua requisição e a sua resposta, e as mensagens são mostradas como o JSON do protobuf, com os
  nomes reais dos campos. Uma mensagem que não se encaixa no seu tipo volta para a leitura sem schema.

As mensagens comprimidas com `grpc-encoding: gzip` são descompactadas.
