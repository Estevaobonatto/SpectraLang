# Spectra Import System Checklist

> Rastreamento do esforço para habilitar resolução real de `import`, incluindo aliases e reexports, permitindo consumir a stdlib sem prefixar `std.` em todas as chamadas.

## Arquitetura e Design

- [ ] Definir como o compilador descobre módulos importados (convenções de caminho, extensão de arquivo, resolução relativa vs. absoluta).
- [ ] Especificar ordem de carga, regras de recompilação incremental e política para ciclos de import.
- [ ] Descrever estratégia de visibilidade/prelude (quais símbolos ficam disponíveis automaticamente, como expor a stdlib sem `std.`).

## Parser & AST

- [x] Reconhecer declarações `import path.to.module;` e armazenar spans para diagnósticos.
- [ ] Permitir aliasing explícito (`import foo.bar as baz;`) e múltiplos itens por declaração se necessário.
- [ ] Suportar reexportes (`pub import`, `export import`, ou alternativa aprovada) caso façam parte do design.

## Resolvedor de Módulos

- [ ] Construir grafo de dependências entre módulos e detectar imports ausentes ou cíclicos.
- [ ] Associar cada `import` aos símbolos exportados do módulo alvo, com tratamento de visibilidade.
- [ ] Popular uma tabela de símbolos compartilhada entre arquivos para permitir lookup durante a análise semântica.
- [ ] Implementar mecanismo de prelude/`use` automático para expor a stdlib sem prefixo.

## Integração CLI / Ferramentas

- [ ] Atualizar `spectra` CLI para carregar dependências transitivas antes da compilação.
- [ ] Registrar diagnósticos claros quando um import falhar (arquivo não encontrado, módulo duplicado, conflito de nomes).
- [ ] Suportar configuração de caminhos adicionais (`--lib`, `Spectra.toml`, etc.) se necessário para localizar bibliotecas padrão ou de terceiros.

## Testes

- [ ] Adicionar testes unitários do resolvedor (casos felizes, ciclos, aliasing, erros).
- [ ] Adicionar testes de integração compilando múltiplos arquivos com imports encadeados.
- [ ] Exercitar acesso à stdlib sem prefixo em `tests/` e `examples/`.

## Documentação

- [ ] Atualizar `docs/language-reference-alpha.md` com a semântica final de imports e exemplos de aliasing/prelude.
- [ ] Complementar `docs/runtime/standard-library.md` com a estratégia de exposição da stdlib.
- [ ] Revisar guias do CLI e README para refletir novo fluxo de compilação multiarquivo.
