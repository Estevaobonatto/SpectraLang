# Spectra Import System Checklist

> Rastreamento do esforço para habilitar resolução real de `import`, incluindo aliases e reexports, permitindo consumir a stdlib sem prefixar `std.` em todas as chamadas.

## Arquitetura e Design

- [x] Definir como o compilador descobre módulos importados (convenções de caminho, extensão de arquivo, resolução relativa vs. absoluta). _(ver decisões em `docs/compiler/import-system-design.md`)_
- [x] Especificar ordem de carga, regras de recompilação incremental e política para ciclos de import. _(mesma referência)_
- [x] Descrever estratégia de visibilidade/prelude (quais símbolos ficam disponíveis automaticamente, como expor a stdlib sem `std.`). _(mesma referência)_

## Parser & AST

- [x] Reconhecer declarações `import path.to.module;` e armazenar spans para diagnósticos.
- [x] Permitir aliasing explícito (`import foo.bar as baz;`) e múltiplos itens por declaração se necessário. _(aliasing disponível; importações múltiplas ficam pendentes)_
- [x] Suportar reexportes (`pub import`, `export import`, ou alternativa aprovada) caso façam parte do design. _(resolver agora propaga `ModuleAlias` para os consumidores e o analisador semântico constrói bindings encadeados, habilitando reexports efetivos)_

## Resolvedor de Módulos

- [x] Construir grafo de dependências entre módulos e detectar imports ausentes ou cíclicos. _(implementado em `ModuleResolver`, com diagnóstico de duplicatas, headers divergentes e ciclos)_
- [x] Associar cada `import` aos símbolos exportados do módulo alvo, com tratamento de visibilidade. _(cada `ResolvedImport` agora carrega `exposed` com os símbolos públicos do módulo destino, considerando aliases e reexports)_
- [x] Popular uma tabela de símbolos compartilhada entre arquivos para permitir lookup durante a análise semântica. _(ver `SemanticWorkspace::analyze` em `compiler/src/semantic/mod.rs`, que alimenta `ModuleImportBinding` para cada alias)_
- [x] Rastrear imports sintéticos gerados pelo compilador para que o resolvedor e o pipeline semântico consigam diferenciá-los de declarações escritas pelo usuário.
- [x] Implementar mecanismo de prelude/`use` automático para expor a stdlib sem prefixo. _(parser injeta `import std.prelude;` sintético, respeitando `#![no_prelude]` e sinalizando a origem para o resolvedor/semântica.)_
- [x] Emitir diagnóstico quando aliases de import conflitam, orientando o uso de `as` para desambiguar. _(implementado em `SemanticWorkspace::build_import_bindings`, com cobertura em `compiler/tests/import_alias_conflict.rs`.)_

## Integração CLI / Ferramentas

- [x] Atualizar `spectra` CLI para carregar dependências transitivas antes da compilação. _(CLI agora utiliza `ModuleResolver` em `ProjectPlan` para montar o grafo e reportar erros de resolução)_
- [x] Registrar diagnósticos claros quando um import falhar (arquivo não encontrado, módulo duplicado, conflito de nomes). _(see `ModuleResolutionError::ModuleNotFound` context: agora lista arquivo/linha da origem no CLI)_
- [x] Executar a análise semântica compartilhada antes do backend para capturar problemas de binding entre módulos.
- [x] Suportar configuração de caminhos adicionais (`--lib`, `Spectra.toml`, etc.) se necessário para localizar bibliotecas padrão ou de terceiros. _(CLI aceita múltiplos `--lib`/`-L` e `Spectra.toml` expõe `libs = [...]`, todos resolvidos antes de alimentar o `ModuleResolver`.)_

## Testes

- [x] Adicionar testes unitários do resolvedor (casos felizes, ciclos, aliasing, erros). _(ver `compiler/src/resolver/mod.rs` – novos testes cobrem grafo simples, ciclos e erros de módulo ausente)._  
- [x] Adicionar testes de integração compilando múltiplos arquivos com imports encadeados. _(ver `compiler/tests/import_chain.rs`, além do cenário de reexport existente)._  
- [x] Exercitar acesso à stdlib sem prefixo em `tests/` e `examples/`. _(ver `compiler/tests/prelude_integration.rs` e `examples/prelude_demo.spectra` após o refresh de novembro/2025.)_

## Documentação

- [x] Atualizar `docs/language-reference-alpha.md` com a semântica final de imports e exemplos de aliasing/prelude.
- [x] Complementar `docs/runtime/standard-library.md` com a estratégia de exposição da stdlib.
- [x] Revisar guias do CLI e README para refletir novo fluxo de compilação multiarquivo. _(ver atualização de `docs/cli/tooling-plan.md` descrevendo o pipeline de importação.)_
