---
source: dabee4094ffd
description: Coloque dois dispositivos lado a lado enquanto usa o app nos dois, com telas, gráficos de CPU e memória e uma lista das diferenças entre eles.
---
# Comparando dois dispositivos

**Compare** põe dois dispositivos lado a lado enquanto você usa o seu app nos dois: um Android antigo ao lado de um novo, um
Pixel ao lado de um iPhone, um celular ao lado de um tablet.

Inicie um segundo emulador ou simulador e ele aparece no seletor **against**. A view então tem:

- **Both screens**, atualizadas a cada poucos segundos.
- **Gráficos de CPU e memória** dos dois ao mesmo tempo, para o app escolhido na barra de ferramentas quando ele roda nos
  dois (do dispositivo todo, caso contrário), para que você veja qual deles trabalha mais.
- **Differences**: cada fato que os dois dispositivos informam (SO, build, hardware, tela, bateria, idioma) alinhado, com o
  que difere destacado. Marque **Show what matches** para a lista completa.

Em um terminal:

```bash
emulabzip compare emulator-5554 "iPhone 17"          # only what differs
emulabzip compare emulator-5554 emulator-5556 --all  # everything
emulabzip compare emulator-5554 emulator-5556 --json
```

Fatos que um dispositivo não tem aparecem como `—`. Comparar um dispositivo Android com um iOS é permitido, embora a maioria
das linhas vá diferir porque as plataformas dão nomes diferentes às coisas.
