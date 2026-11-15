---
source: ae0f1b688c5b
description: O EmuLabZip tem dez temas; você pode criar o seu, importar o de outra pessoa e compartilhar o seu na galeria da comunidade, em Settings, Appearance.
---
# Temas

O EmuLabZip vem com dez temas, e você pode criar o seu, importar o de outra pessoa e compartilhar o seu na galeria da
comunidade. Abra **Settings** (⌘,) e vá para **Appearance**.

![As configurações de Appearance com uma grade de cards de temas](images/themes.png)

## Usando um tema

O app guarda um tema para o escuro e outro para o claro. Escolher um tema define o da sua base, e a chave **Mode** (escuro,
claro ou sistema) escolhe qual é mostrado. O botão de sol e lua na barra de ferramentas alterna entre os dois.

Os temas embutidos:

| Tema | Base | |
| --- | --- | --- |
| **Midnight** | escuro | O padrão: um escuro mais claro, mais fácil de ler. |
| **Classic Dark** | escuro | O tema quase preto das versões anteriores. |
| **Graphite**, **Fjord**, **Ember**, **Forest** | escuro | Cinza neutro, azul frio, quente e verde. |
| **Daylight** | claro | O tema claro padrão. |
| **Paper** | claro | Um claro mais suave e mais quente. |
| **High Contrast Dark** e **Light** | escuro, claro | Contraste AAA para o texto. |

## Criando um tema

**New theme** abre o editor, e **Duplicate** em qualquer card começa a partir daquele tema. O app inteiro mostra suas mudanças
enquanto você edita; **Cancel** devolve o tema escolhido.

- **Start from one colour** preenche as superfícies e o texto a partir de um destaque, na luminosidade que a base precisa.
- Toda cor tem um seletor e um campo de texto. As cores são `#rgb`, `#rrggbb`, `#rrggbbaa`, `rgb()` ou `hsl()`.
- **Fonts** são nomes de famílias, separados por vírgulas. Elas devem estar instaladas no computador; nada é baixado.
- **Roundness** vai de quadrado (0) a muito arredondado (2).
- O editor avisa quando o texto seria difícil de ler: ele verifica o contraste WCAG dos pares principais (texto na janela e nos
  painéis, texto secundário, dicas, destaque, erros, avisos).

Abaixo dos cards, **Fine-tune** muda coisas que são suas e não fazem parte de nenhum tema: tamanho do texto (85% a 140%),
escala da interface, densidade, arredondamento e fontes. **Reset the fine-tuning** as restaura.

![O editor de temas, com um aviso de contraste](images/theme-editor.png)

## O formato do arquivo

Um tema é um arquivo JSON de algumas centenas de bytes de cores, com no máximo 64 KB:

```json
{
  "schema": 1,
  "id": "aurora",
  "name": "Aurora",
  "author": "EmuLabZip",
  "description": "Violet accents on a deep, cool grey.",
  "base": "dark",
  "colors": { "bg": "#1d1a23", "panel": "#24212c", "fg": "#f1eff7", "accent": "#8b5cf6" },
  "fonts": { "sans": "Inter, system-ui, sans-serif", "mono": "Menlo, monospace" },
  "shape": { "radius": 1 }
}
```

Os nomes das cores são os grupos do editor: superfícies (`bg`, `panel`, `panel-2`, `elevated`, `hover`, `line`,
`line-strong`), texto (`fg`, `muted`, `faint`), destaque (`accent`, `accent-2`, `accent-soft`, `select`), status (`ok`, `warn`,
`err`, `info`), gráficos (`series-1`, `series-2`, `flame-app`, `flame-framework`, `flame-native`, `flame-kernel`) e níveis de
log (`lv-verbose`, `lv-debug`, `lv-info`, `lv-warn`, `lv-error`, `lv-fatal`). Uma cor que você omite é tirada do tema padrão da
`base`.

### Por que um tema é seguro de abrir

Um tema pode vir de qualquer pessoa, então ele é verificado antes de ser usado:

- uma cor deve ser uma cor hexadecimal ou `rgb()`/`hsl()` com números, e nada mais;
- uma fonte é uma lista de nomes de famílias. `url(`, chaves e ponto e vírgula são recusados, então um tema não pode carregar
  CSS nem buscar um arquivo;
- chaves desconhecidas são descartadas, e o arquivo não pode passar de 64 KB;
- a content security policy do app não permite nada remoto exceto o endereço da própria galeria.

## Importar e exportar

**Export** em um card salva um tema como `<id>.json`. **Import file** lê um, e **Paste JSON** recebe o texto. O tema é
mostrado primeiro em um card, e só é instalado quando você pressiona Install.

## A galeria da comunidade

A aba **Gallery** lista temas compartilhados por outras pessoas, lidos de
`https://emulabzip.luispeixoto.dev/themes/index.json`. Instale um e ele é seu para usar e editar. Sem conexão, a aba avisa e
todo o resto continua funcionando.

![A aba Gallery com temas da comunidade](images/theme-gallery.png)

### Compartilhe seu tema

1. Dê ao tema um autor e uma descrição no editor.
2. Em **Gallery**, escolha o tema em **Share a theme** e pressione **Open on GitHub**.
3. O GitHub abre um novo arquivo, `site/static/themes/<id>.json`, com seu tema dentro. Entre na sua conta e pressione
   **Propose new file**; ele vira um pull request.

Você também pode adicionar o arquivo manualmente em `site/static/themes/` em um pull request. O build do site verifica cada
arquivo com as mesmas regras do app (`node site/build.mjs --check`), e um pull request que falha na verificação não pode ser
mesclado:

- o arquivo tem o nome do seu `id`, que deve ser letras minúsculas, dígitos e hifens, e não ser o de um tema embutido;
- ele tem um `author` e uma `description`;
- toda cor e fonte passa nas verificações acima;
- o texto é legível: pelo menos 4,5:1 para o texto e o texto secundário na janela e nos painéis.

O índice que o app lê é gerado pelo build a partir desses arquivos, então não há mais nada para editar.
