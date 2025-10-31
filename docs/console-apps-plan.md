# SpectraLang Console Application Support Plan

## Visão Geral
- Capacitar desenvolvedores a criar aplicações de console simples e complexas em SpectraLang com a mesma naturalidade encontrada em linguagens como C++ e Python.
- Preservar o princípio central da linguagem: **sintaxe simples, direta e fácil de usar**, evitando configuração excessiva ou cerimônia ao criar projetos de linha de comando.
- Entregar uma experiência consistente para criação, compilação, execução e empacotamento de diversos binários dentro de um mesmo workspace SpectraLang.

## Princípios Norteadores
- **Sintaxe simples primeiro**: fluxos de I/O e operações comuns devem exigir o mínimo de código boilerplate possível.
- **Erro único, contexto rico**: mensagens claras apontando tanto problemas de sintaxe quanto de execução (ex.: `main` ausente, múltiplos `main`).
- **Ferramenta opinativa, extensível**: CLI com defaults sensatos, porém permitindo customização de diretórios, nomes de binários e opções de build.
- **Paridade multi-plataforma**: comportamento idêntico (ou explicitamente documentado) em Windows, Linux e macOS.
- **Evolução incremental**: desenho modular para permitir futuras extensões (ex.: empacotamento, instaladores, integração com IDEs).

## Diagnóstico Atual
- `spectra-cli` compila múltiplos arquivos, mas não diferencia alvos executáveis vs. bibliotecas.
- Runtime carece de API padrão para entrada/saída, argumentos e códigos de saída.
- Não há templates oficiais ou convenções de layout para aplicações de console.
- Documentação descreve pipeline do compilador, mas não orienta criação de apps completas.

## Objetivos Funcionais
- Permitir `fn main(): i32` como ponto de entrada obrigatório para executáveis.
- Suporte a múltiplos binários no mesmo projeto, cada um com seu módulo `main`.
- Disponibilizar biblioteca padrão `std.console` (stdout/stderr/stdin) e `std.args`.
- Expor CLI com comandos `new`, `build`, `run`, `test` para facilitar ciclo de desenvolvimento.
- Permitir passagem de argumentos e coleta de código de saída pela CLI.

## Requisitos Não Funcionais
- Build e execução em menos de 2 segundos para projetos simples.
- Geração de mensagens de erro e logs em português e inglês (avaliar internacionalização).
- Respeitar padrões de formatação existentes (`cargo fmt`, `cargo clippy`).
- Cobertura de testes E2E para fluxos críticos (criar → build → run → erro).

## Plano de Implementação

### Fase 0 — Alinhamento e ADR
- Produzir ADR em `docs/decisions/` definindo objetivo, escopo, trade-offs.
- Validar interface da CLI e convenções de projeto com stakeholders.
- Definir métricas de sucesso (tempo de build, experiência de onboarding, bugs críticos).

### Fase 1 — Fundamentos do Runtime
- Implementar módulos `std.console` (print/println/print_err/read_line) e `std.args` (lista de argumentos, contagem).
- Criar abstrações portáveis para stdout/stderr/stdin no crate `runtime/`.
- Adicionar testes unitários e exemplos de uso simples (`hello_world`, `echo`).

### Fase 2 — Suporte a Entrypoints no Compilador
- Atualizar `semantic.rs` para reconhecer `fn main(): i32` como assinatura obrigatória.
- Validar existência única de `main` por alvo e reportar conflitos ou ausência.
- Permitir metadados indicando qual módulo gera binário (ex.: `module app.cli;` → `cli`).
- Acrescentar testes de análise semântica para entradas válidas/invalidas.

### Fase 3 — Ampliação da CLI (`spectra-cli`)
- Implementar `spectra new <nome>` com templates:
  - Console simples (um `main` e README minimalista).
  - Console modular (múltiplos módulos + testes).
- Implementar `spectra build` e `spectra run`:
  - Seleção de alvo (`--bin`, `--all`).
  - Flags de otimização (`--release`, `--debug`).
  - Encaminhamento de argumentos pós `--`.
- Registrar comandos de erro usuais (ex.: ausência de target) com mensagens amigáveis.

### Fase 4 — Infraestrutura de Build e Artefatos
- Definir diretórios de saída (`target/debug/<bin>` e `target/release/<bin>`).
- Adicionar manifestos simples por binário (metadados para depuração futura).
- Avaliar necessidade de linking com runtime estático/dinâmico.
- Documentar processo para embutir recursos adicionais (assets simples, configs).

### Fase 5 — Testes End-to-End
- Testes automatizados para os fluxos `new → build → run` com asserts em stdout/stderr.
- Casos negativos: múltiplos `main`, `main` inválido, acesso a args sem import, etc.
- Inclusão no pipeline CI (Windows e Linux inicialmente).

### Fase 6 — Documentação e Materiais de Onboarding
- Atualizar `README.md` com guia rápido.
- Criar tutoriais em `docs/` (ex.: "Construindo um ToDo CLI", "Parsing de argumentos avançado").
- Elaborar FAQ abordando erros comuns e convenções da sintaxe simples.

### Fase 7 — Roadmap Evolutivo
- Suporte a testes unitários integrados à CLI (`spectra test`).
- Geração de binários distribuíveis (zip/tar, instaladores básicos).
- Integração com IDEs (VS Code tasks, templates de launch).
- Monitoramento de feedback: canal dedicado, telemetria opt-in.

## Entregáveis por Fase
- Código + testes + documentação parcial entregues incrementalmente por PR.
- Exemplos em `examples/console_*` demonstrando cenários simples e complexos.
- Scripts de automação para gerar projetos de demonstração durante workshops.

## Riscos e Mitigações
- **Complexidade de runtime multiplataforma**: priorizar abstrações mínimas; usar crates consolidados do ecossistema Rust.
- **Escopo do template**: restringir àquilo que reforça a simplicidade sintática, evitando frameworks pesados.
- **Manutenção futura**: documentação clara e testes asseguram evolução sem regressões.

## Próximos Passos Imediatos
- Revisar plano com time para refinar requisitos da CLI e runtime.
- Criar ADR inicial e backlog (issue tracking) alinhado às fases.
- Iniciar protótipo do módulo `std.console` e exemplos de uso.
