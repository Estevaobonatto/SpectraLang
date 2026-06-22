# Tutorial para desenvolvedores de pacotes Spectra

Este guia mostra o fluxo completo para criar, testar, versionar, registrar e
validar um pacote Spectra consumido por outros projetos via:

```powershell
spectralang package add meu.pacote
```

O modelo atual usa repositórios Git públicos como fonte de pacote e catálogos
versionados como índice de descoberta. O usuário final não precisa informar a
URL do Git quando o pacote já está registrado em um catálogo configurado.

## 1. Conceitos

Um pacote Spectra é um diretório com:

- `spectra.toml`: manifesto do pacote;
- `src/*.spectra`: módulos exportados;
- tags Git semver, como `v1.2.3`;
- entrada de catálogo com nome, versão, URL Git, tag, compatibilidade e módulos.

O fluxo de publicação tem duas partes:

1. O código do pacote fica em um repositório Git público.
2. Os metadados do pacote ficam em um catálogo `package.index.toml`.

O catálogo é o que permite:

```powershell
spectralang package search math
spectralang package info meu.pacote
spectralang package add meu.pacote
```

## 2. Criar pacote

Estrutura mínima:

```text
meu-pacote/
  spectra.toml
  src/
    core.spectra
```

Manifesto:

```toml
[project]
name = "meu.pacote"
version = "0.1.0"
entry = "src/core.spectra"
src_dirs = ["src"]

[release]
channel = "stable"
compatibility = "spectralang-0.1"

[dependencies]
```

Código:

```spectra
module meu.pacote.core;

pub fn dobro(valor: int) -> int {
    return valor * 2;
}
```

Regras importantes:

- Use nome estável e único, por exemplo `org.nome` ou `meu.pacote`.
- Use versionamento semver exato `MAJOR.MINOR.PATCH`.
- Prefixe módulos com o nome do pacote: `meu.pacote.core`.
- Exporte apenas APIs que usuários devem importar com `pub`.
- Evite função `main` em bibliotecas, salvo se o pacote também for executável.

## 3. Testar pacote localmente

Na raiz do pacote:

```powershell
spectralang package lock --root .
spectralang package check --root .
spectralang package doc --root .
```

Se o pacote tiver testes em `tests/*.spectra`:

```powershell
spectralang package test --root .
```

O lockfile `spectra.lock` deve ser determinístico. Rode duas vezes e confirme
que não muda:

```powershell
spectralang package lock --root .
git diff -- spectra.lock
```

## 4. Testar consumo antes de registrar

Crie um projeto consumidor temporário:

```text
consumer/
  spectra.toml
  src/
    main.spectra
```

Manifesto do consumidor:

```toml
[project]
name = "consumer"
version = "0.1.0"
entry = "src/main.spectra"
src_dirs = ["src"]

[dependencies]
```

Código do consumidor:

```spectra
module consumer.main;

import { dobro } from meu.pacote.core;

pub fn main() -> int {
    let resultado = dobro(21);
    if resultado != 42 {
        return resultado;
    }
    return 0;
}
```

Instale direto do Git local ou público:

```powershell
spectralang package add meu.pacote --root consumer --git https://github.com/org/meu-pacote.git --tag v0.1.0
spectralang package run --root consumer
```

Depois do `add`, o consumidor terá:

```toml
[dependencies."meu.pacote"]
version = "0.1.0"
git = "https://github.com/org/meu-pacote.git"
tag = "v0.1.0"
checksum = "<sha256>"
```

## 5. Preparar repositório Git

O pacote deve estar commitado e tagueado:

```powershell
git init
git add spectra.toml src
git commit -m "release meu.pacote 0.1.0"
git tag v0.1.0
git remote add origin https://github.com/org/meu-pacote.git
git push origin main
git push origin v0.1.0
```

O `spectralang package add` resolve a tag para commit SHA e grava isso no
`spectra.lock` do consumidor.

## 6. Registrar pacote em catálogo

Um catálogo é um diretório ou arquivo com `package.index.toml`.

Registrar pacote no catálogo:

```powershell
spectralang package register --root . --git https://github.com/org/meu-pacote.git --tag v0.1.0 --catalog ./catalog
```

Isso cria ou atualiza:

```text
catalog/
  package.index.toml
```

Entrada gerada:

```toml
schema = "spectra-package-catalog-v1"

[[packages]]
name = "meu.pacote"
version = "0.1.0"
git = "https://github.com/org/meu-pacote.git"
tag = "v0.1.0"
resolved_rev = "<commit-sha>"
checksum = "<sha256>"
description = ""
keywords = []
compatibility = "spectralang-0.1"
license = ""
modules = ["meu.pacote.core"]
owner = ""
```

Complete os campos humanos antes de publicar o catálogo:

```toml
description = "Funções utilitárias para exemplos Spectra"
keywords = ["utils", "math"]
license = "MIT"
owner = "org"
```

Regras de segurança aplicadas pelo CLI:

- use `--tag` ou `--rev`; `--branch` é rejeitado para publicação em catálogo,
  porque branch muda com o tempo, e `--rev` precisa ser commit SHA;
- a tag ou rev informada deve apontar para o `HEAD` do checkout usado em
  `--root`;
- `resolved_rev` grava o commit real que será auditado pelo catálogo;
- publicar a mesma combinação `name` + `version` com Git URL, ref, commit,
  checksum, módulos ou compatibilidade diferente falha;
- módulos exportados devem ficar dentro do namespace do pacote. Para
  `meu.pacote`, use módulos como `meu.pacote.core`;
- `checksum` precisa ser SHA-256 e nomes/refs/metadados não podem conter
  caracteres de controle.

Se precisar corrigir código já publicado, lance nova versão. Não mova tag já
publicada.

## 7. Publicar metadados sem alterar catálogo

Se você quer gerar um arquivo de metadados para enviar em PR ao catálogo:

```powershell
spectralang package publish-metadata --root . --git https://github.com/org/meu-pacote.git --tag v0.1.0 --out package.index.toml
```

Use esse arquivo como base para abrir PR no repositório de catálogo.

Checklist recomendado para PR no repositório de catálogo:

1. Tag existe no repositório público e não é movida depois do PR.
2. `spectralang package publish-metadata --root . --git <url> --tag <tag> --out package.index.toml` roda sem erro.
3. Entrada tem `resolved_rev` com commit SHA e `checksum` com 64 caracteres hex.
4. `modules` lista só módulos públicos do pacote.
5. `description`, `keywords`, `license` e `owner` estão preenchidos.
6. Mesma versão não altera source/checksum de release anterior.

## 8. Testar catálogo como usuário final

Crie projeto consumidor com catálogo configurado:

```toml
[project]
name = "consumer"
version = "0.1.0"
entry = "src/main.spectra"
src_dirs = ["src"]

[package.catalogs]
local = "./catalog"

[dependencies]
```

Buscar pacote:

```powershell
spectralang package search pacote --root consumer
spectralang package info meu.pacote --root consumer
spectralang package versions meu.pacote --root consumer
```

Instalar com um comando:

```powershell
spectralang package add meu.pacote --root consumer
```

Instalar versão específica:

```powershell
spectralang package add meu.pacote@0.1.0 --root consumer
```

Validar uso:

```powershell
spectralang package check --root consumer
spectralang package run --root consumer
spectralang package tree --root consumer
```

## 9. Atualizar pacote

Para lançar `0.2.0`:

1. Atualize `version` no `spectra.toml`.
2. Atualize código e docs.
3. Rode validações locais.
4. Commit.
5. Crie tag `v0.2.0`.
6. Rode `package register` de novo.

Comandos:

```powershell
spectralang package check --root .
spectralang package test --root .
git add spectra.toml src docs
git commit -m "release meu.pacote 0.2.0"
git tag v0.2.0
git push origin main
git push origin v0.2.0
spectralang package register --root . --git https://github.com/org/meu-pacote.git --tag v0.2.0 --catalog ./catalog
```

Usuários sem versão fixa recebem a versão mais nova compatível do catálogo.
Usuários que chamam `package add meu.pacote@0.1.0` continuam fixos em `0.1.0`.

## 10. Dependências entre pacotes

Um pacote pode depender de outro pacote Git:

```toml
[dependencies."base.lib"]
version = "1.0.0"
git = "https://github.com/org/base-lib.git"
tag = "v1.0.0"
checksum = "<sha256>"
```

Código:

```spectra
module meu.pacote.core;

import { seed } from base.lib.core;

pub fn valor() -> int {
    return seed() + 2;
}
```

O consumidor final só instala o pacote principal:

```powershell
spectralang package add meu.pacote --root consumer
```

O resolver instala também dependências transitivas e registra tudo no
`spectra.lock`.

## 11. Validar offline e integridade

Depois de uma instalação bem-sucedida:

```powershell
spectralang package fetch --root consumer --offline
spectralang package check --root consumer
```

Se o conteúdo instalado for alterado e o checksum no manifesto não bater, a
compilação falha antes de usar o pacote.

## 12. Checklist de publicação

Antes de registrar ou abrir PR no catálogo:

- `spectra.toml` tem `name`, `version`, `entry`, `src_dirs`, `[release]`;
- versão segue semver exato;
- módulos usam prefixo do pacote;
- APIs públicas têm `pub`;
- pacote compila com `spectralang package check --root .`;
- testes passam com `spectralang package test --root .`, quando existirem;
- Git tag existe e foi enviada ao remoto;
- `package register` gera entrada com módulos corretos;
- consumidor de teste consegue `package add nome`, `package check`, `package run`;
- `package tree` mostra grafo esperado;
- `package fetch --offline` passa após cache local.

## 13. Troubleshooting

Erro: `package 'x' was not found`

- Verifique `[package.catalogs]` no consumidor.
- Rode `spectralang package search x --root consumer`.
- Confirme que `package.index.toml` contém `[[packages]] name = "x"`.

Erro: `checksum mismatch`

- Conteúdo instalado não bate com checksum do manifesto.
- Rode `package add` novamente após publicar tag correta.
- Não mova tag já publicada; crie nova versão.

Erro: import não resolve

- Confirme módulo no arquivo `.spectra`: `module meu.pacote.core;`.
- Confirme que o módulo aparece em `modules = [...]` no catálogo.
- Use import normal:

```spectra
import { simbolo } from meu.pacote.core;
```

Erro: versão errada

- Use pin explícito:

```powershell
spectralang package add meu.pacote@0.1.0 --root consumer
```

Erro: Git não baixa

- Confirme que o repo é público ou acessível pelo Git local.
- Confirme que a tag existe:

```powershell
git ls-remote --tags https://github.com/org/meu-pacote.git
```

## 14. Validação usada pelo repositório

O fluxo production do package manager é validado por:

```powershell
python scripts\validate_r914_package_catalog_git.py --binary target\debug\spectralang.exe
.\run_tests.ps1
```

O validator cria repositórios Git locais, registra pacote em catálogo, busca
metadados, instala com `package add nome`, valida dependência transitiva,
compila, executa, gera docs, testa modo offline e força falha de checksum.
