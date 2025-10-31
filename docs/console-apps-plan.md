# SpectraLang Console Application Support Plan

## Visão Geral
- Capacitar desenvolvedores a criar aplicações de console simples e complexas em SpectraLang com a mesma naturalidade encontrada em linguagens como C++ e Python.
- Preservar o princípio central da linguagem: **sintaxe simples, direta e fácil de usar**, evitando configuração excessiva ou cerimônia ao criar projetos de linha de comando.
- Entregar uma experiência consistente para criação, compilação, execução e empacotamento de diversos binários dentro de um mesmo workspace SpectraLang.

## Status Atual (Out/2025)
- `runtime::console` exposto com `print/println/print_err/println_err`; `runtime::args` provê `all/len/is_empty` para integração futura com CLI.
- Novo módulo `compiler::project` coleta e valida entradas `fn main(): i32`, emitindo diagnósticos ricos quando a assinatura está incorreta ou ausente.
- `spectra-cli` reestruturado em subcomandos `new`, `build` e `run`, com varredura automática de `src/**/*.spc`, geração de manifesto em `target/<profile>/<bin>.build.txt`, scaffold inicial de projetos e seleção explícita de entrypoint via `--main`.
- README atualizado para refletir o fluxo baseado em projeto e os novos comandos.

> Pendências imediatas: entrada padrão (`read_line`) no runtime, suporte multi-binário (escolha de `main` por módulo), execução real no comando `run` quando o backend estiver disponível.

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
- ✅ `std.console` (print/println/print_err/println_err) e `std.args` (all/len/is_empty) publicados no crate `runtime/`.
- 🔜 Completar `read_line`/entrada padrão e amostras de uso simples (`hello_world`, `echo`).

### Fase 2 — Suporte a Entrypoints no Compilador
- ✅ `compiler::project::find_console_entry_point` garante `fn main(): i32` sem parâmetros, com testes para cenários válidos e inválidos.
- 🔜 Propagar validação para a fase semântica (erros ainda são reportados pelo CLI) e permitir metadados para múltiplos binários (ex.: associação módulo→bin no manifesto).

### Fase 3 — Ampliação da CLI (`spectra-cli`)
- ✅ `spectra new`, `spectra build` e `spectra run` implementados com scaffold básico, seleção de profile (`--release`) e captura de argumentos para uso futuro.
- ✅ Seleção de `main` por módulo através da flag `--main`.
- 🔜 Templates adicionais (console modular + testes), múltiplos artefatos por build e execução real no `run`.

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
- Implementar `std.console::read_line` e exemplos de uso que demonstrem entrada/saída simples.
- Estender o manifesto/CLI para gerar múltiplos artefatos por build (mapa módulo → binário) e cobrir o fluxo com testes E2E.
- Escrever ADR resumindo as decisões do suporte a console apps e alinhar backlog para execução incremental das próximas fases.
