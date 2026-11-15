---
source: 18e7488c6820
description: Abra um arquivo .pcap ou .pcapng no EmuLabZip e veja requisições HTTP, conexões TLS e consultas DNS como no tráfego ao vivo, sem usar o proxy.
---
# Importando uma captura de pacotes

O EmuLabZip pode abrir um arquivo `.pcap` ou `.pcapng` e mostrar o que há nele do mesmo jeito que mostra o tráfego ao vivo.
É útil quando você não pode usar o proxy: um app que ignora as configurações de proxy, tráfego de um serviço do sistema ou
uma captura que alguém enviou para você.

```bash
emulabzip net import capture.pcap            # lists the requests
emulabzip net import capture.pcap --dns      # and the DNS lookups
emulabzip net import capture.pcap --har out.har
```

No app, pressione **Import** na view Network. As requisições entram na lista com um selo `PCAP`, e um banner mostra o que o
arquivo continha, com as consultas DNS a um clique.

## O que sai

| Na captura | No EmuLabZip |
|----------------|--------------|
| HTTP/1.x puro sobre TCP | Requisições e respostas, com headers e bodies (os chunked e os comprimidos são decodificados) e tempos |
| Uma conexão TLS | Uma **conexão** com o nome do servidor da primeira mensagem (SNI), com os bytes de cada lado |
| Qualquer outra conexão TCP | Uma conexão para o seu endereço |
| DNS sobre a porta UDP 53 | Uma lista de consultas com suas respostas e status |

O TLS continua criptografado, então o conteúdo dele não está disponível em uma captura; use o proxy para isso. Os streams
TCP são remontados pelo número de sequência: segmentos fora de ordem e retransmitidos são tratados, e uma peça faltando é
informada em vez de adivinhada ("some packets are missing"). Pacotes IP fragmentados são ignorados. Arquivos de até 512 MB
são lidos.

Capturas Ethernet, Linux "cooked" (interface `any`), IP bruto e loopback são entendidas, em qualquer ordem de bytes e com
timestamps em nanossegundos.

## Capturando

Em um emulador Android, inicie-o com `-tcpdump`:

```bash
emulator -avd Pixel_9 -tcpdump capture.pcap
```

Tudo o que a rede emulada transporta é gravado no arquivo, que fica completo quando o emulador sai. Em uma imagem ou um
dispositivo com root você pode usar o `tcpdump` pelo shell: `adb shell tcpdump -i any -w /sdcard/capture.pcap` e depois
`adb pull` nele. Em um Mac, `tcpdump -i lo0 -w capture.pcap` na interface de loopback captura o tráfego de um simulador.
