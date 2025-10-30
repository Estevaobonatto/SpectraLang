# Plano detalhado da linguagem SpectraLang

Descrição breve: SpectraLang é uma linguagem moderna, simples e expressiva que combina paradigmas orientado a objetos, procedural e funcional; oferece tipagem forte com modo fraco opt‑in via diretivas; compilação JIT multi‑alvo (x86, ARM, WASM); gerenciamento de memória com GC e controle manual; ferramentas e ecossistema completos.

## Status atual (Out 2025)

- Monorepo Rust configurado com crates `spectra-compiler`, `spectra-runtime` e `spectra-cli`, pipeline CI GitHub Actions (fmt, clippy, test) e documentação inicial (`README`, ADRs 0001–0002).
- Lexer e parser entregues com rastreamento de spans, suporte a módulos, funções, blocos e `return`; suíte de testes cobrindo cenários básicos.
- CLI `spectra` capaz de lex/parsing, executar análise semântica inicial (escopos, retornos, identificadores, uso de símbolos, tipos primitivos) com validação de chamadas de função e imports, além de apresentar estatísticas do módulo.
- Pasta `docs/decisions` ativa (ADR 0001 – Lexer, ADR 0002 – Parser, ADR 0003 – Roadmap Semântico); plano de trabalho alinhado com backlog por fases.
- AST e parser atualizados para cobrir declarações `import` e expressões de chamada, com novos testes garantindo cobertura de casos positivos e diagnósticos.
- Analisador semântico ampliado com registro de assinaturas, checagem de aridade/tipos em chamadas, e validação de imports (auto-import e módulos desconhecidos).
- Suporte multi-módulo: CLI e analisador recebem múltiplos arquivos simultaneamente, compartilham assinaturas exportadas, detectam conflitos entre imports e reportam diagnósticos consistentes entre módulos.

## Linha do tempo da colaboração

- Levantamento inicial das especificações e criação de plano detalhado, backlog e ADRs para decisões de lexer/parser.
- Configuração do workspace Rust com crates separados, CI (fmt/clippy/test) e documentação base (`README`, ADRs, planos).
- Implementação do lexer com spans e comentários, parser com suporte a módulos/funções/blocos e suíte de testes correspondente.
- Atualização da CLI `spectra` para exibir estatísticas de parsing e refletir o estado atual do compilador.
- Execução de `cargo fmt`, `cargo clippy` e `cargo test` garantindo base limpa antes de avançar para a análise semântica.
- Implementação da primeira iteração do analisador semântico (escopos, retornos, identificadores) e integração da verificação na CLI.
- Evolução da análise semântica com rastreamento de uso de símbolos (variáveis/parâmetros não utilizados) e cobertura de testes dedicada.
- Registro do ADR 0003 detalhando o roadmap para resolução entre módulos e expansão do sistema de tipos.
- Introdução da inferência e checagem básica de tipos (literais, unários, binários e `return`) com suíte de testes dedicada.
- Extensão da AST/parser para `import` e chamadas de função, com análises semânticas que validam assinaturas e imports acompanhadas de testes automatizados.
- Habilitação da análise multi-módulo na CLI/analisador, com compartilhamento de símbolos exportados, detecção de conflitos entre módulos e suporte a múltiplos arquivos por execução.

## 1. Características técnicas

- Paradigmas
  - Orientação a Objetos: classes, herança simples com mixins, interfaces (traits), encapsulamento (public/private/protected), polimorfismo dinâmico e estático (via generics), construtores e destrutores.
  - Procedural: funções top‑level, escopo lexical, módulos, subrotinas, constantes globais controladas.
  - Funcional: funções de primeira classe, closures, imutabilidade por padrão (let), mutabilidade explícita (var), map/filter/reduce, pattern‑matching, ADTs (enum/union), pipe operator `|>`.

- Sistema de tipos
  - Estático, forte, com inferência local.
  - Tipos primitivos: `i8..i64`, `u8..u64`, `f32`, `f64`, `bool`, `char`, `string`.
  - Compostos: `array`, `slice`, `tuple`, `struct`, `class`, `enum`, `union` (sum types), `option`, `result`.
  - Generics paramétricos com constraints (trait bounds).
  - Modo fraco opt‑in via diretivas:
    - Arquivo ou bloco: `directive weak_types on/off`.
    - Tipo `any/dynamic` habilitado apenas em modo fraco; conversões implícitas entre primitivos com checagem em runtime; uso de `as?` para casts seguros.

- Compilação
  - JIT incremental com cache de funções e especialização por tipo (inline caches).
  - IR própria (SIR) em SSA.
  - Backends: Cranelift (x86_64, aarch64) e WASM (WASI). AOT opcional via LLVM em fase avançada.

- Memória
  - GC geracional/incremental com write barriers, pausas curtas configuráveis.
  - Controle manual opt‑in:
    - Arenas (Region) para alocações de curta duração; `clear/reset` explícito.
    - RAII com `defer/using` para liberar recursos (arquivos, sockets, handles).
    - Atributo `no_gc` em funções críticas; tipo `ManualRef<T>` para ciclos de vida controlados.

- Estruturas de dados (stdlib)
  - `Array`, `Slice`, `Vector`, `List`, `Deque`, `Queue`, `Stack`, `HashMap`, `OrderedMap`, `Set`, `BitSet`, `Heap/PriorityQueue`.
  - Módulos extras: `Graph`, `Tree/BTree`, `RingBuffer`, `BloomFilter`, `LRUCache`.
  - Iteradores e algoritmos: `sort`, `binary_search`, `join`, `group_by`.

## 2. Especificações da sintaxe

- Filosofia: limpa, consistente, poucas palavras‑chave em inglês, legibilidade e previsibilidade.

- Palavras‑chave núcleo
  - Módulos: `module`, `import`, `export`, `pub`, `use`.
  - Tipos e definição: `type`, `struct`, `enum`, `union`, `trait`, `impl`, `class`, `extends`, `new`.
  - Controle: `if`, `else`, `match`, `for`, `while`, `break`, `continue`, `return`, `defer`, `using`.
  - Funções: `fn`, `async`, `await`, `yield`.
  - Erros: `try`, `catch`, `throw`.
  - Mutabilidade: `let` (imutável), `var` (mutável).
  - Metaprogramação: `macro`, `comptime` (execução restrita).

- Exemplos de uso

```spectra
module core.math
import std.io as io

let x = 10
var y = 20

fn add(a: i32, b: i32): i32 {
  return a + b
}

trait Drawable { fn draw(self) }

class Circle(radius: f64) implements Drawable {
  fn draw(self) {
    io.print("Drawing circle: " + fmt(radius))
  }
}

match value {
  Some(v) => io.print(v),
  None => io.print("empty")
}

directive weak_types on
let v: any = 42
```

- Metaprogramação controlada
  - Macros higiénicas (AST macros), sem acesso a IO/rede; expansões limitadas.
  - `comptime`: funções avaliadas em compilação com sandbox; permitem gerar código e validar invariantes, sem efeitos colaterais.

- Sistema de módulos
  - Resolução via nomes canônicos (`org/pkg@version/path`), interoperável com gerenciador de pacotes.
  - `import/export` com escopo explícito; `pub` para visibilidade.

## 3. Arquitetura do compilador

- Implementação em Rust.

- Frontend
  - Lexer: autômato com suporte Unicode; tokens com localização; recuperação de erro.
  - Parser: LL(k)/LR(1) com Pratt para expressões; AST imutável; mensagens de erro ricas.
  - Semântica: resolução de nomes, tipos, lifetimes simplificados, visibilidade e mutabilidade; lints.

- Middle‑end
  - SIR (Spectra IR) em SSA: blocos básicos e instruções consistentes.
  - Otimizações: constant folding, DCE, copy‑prop, inlining seguro, LICM, strength reduction, escape analysis, especialização por tipos monomorfizados.
  - Perfilagem: contadores e hints para JIT.

- Backend
  - Cranelift para código nativo (x86_64/aarch64) com JIT e fallback AOT.
  - Lowering para WASM (WASI) mantendo semântica; execução em navegador/sandbox.
  - FFI com fronteiras seguras; ABI documentado.

- Runtime e stdlib
  - Núcleo: memória, GC, coleções, strings, tempo, IO, OS (abstração portátil), `fmt`, `math`.
  - Erros: `Result`, `Option`, `try/catch`, backtraces configuráveis.
  - Observabilidade: logs em `stderr`, níveis de log, métricas, endpoint `/health` (modo server).

## 4. Plano de desenvolvimento

- Fase 1 (3 meses): Protótipo básico
  - Mês 1: Lexer, parser, AST, erros; sintaxe essencial (`fn/let/var/if/while/return`).
  - Mês 2: Tipagem forte (primitivos, funções), SIR e JIT baseline via Cranelift; stdlib mínima (`io/fmt`).
  - Mês 3: GC inicial, runtime básico, CLI `spectra` (build/run/test), integração WASM mínima; testes de unidade/integração.

- Fase 2 (4 meses): Recursos avançados
  - Mês 4: OO (`class/trait/impl/extends`), generics, pattern matching, ADTs.
  - Mês 5: Módulos e pacote manager (`init`, `add`, `build`), metaprogramação (`macro/comptime`).
  - Mês 6: LSP (autocompletar, hover, diagnostics), formatter (`spectrafmt`), DAP (debugger).
  - Mês 7: Otimizações middle‑end, caches JIT, especialização por tipo, melhorias WASM.

- Fase 3 (2 meses): Otimização e polimento
  - Mês 8: Performance (<15% do alvo), gates de qualidade; POSIX 95% via camada de compat; cross‑platform.
  - Mês 9: Refino do GC (geracional/incremental), arenas, `no_gc`, RAII; observabilidade/logs; lints.

- Fase 4 (1 mês): Documentação e exemplos
  - Mês 10: Especificação formal EBNF, referência completa, tutoriais, exemplos end‑to‑end (CLI e WASM), melhores práticas; guias de migração e releases.

## 5. Ferramentas auxiliares

- IDE/LSP: servidor em Rust (`tower-lsp`) com autocompletar, hover, goto definition, rename, diagnostics em tempo real.
- Debugger: DAP com breakpoints, step, watch, inspeção de variáveis; integração com VS Code e outros.
- Gerenciador de pacotes: `spectra`
  - Comandos: `init`, `add`, `remove`, `update`, `build`, `run`, `test`, `publish`.
  - Manifesto: `spectra.toml` com dependências e semver; `lockfile` pinado.
- Formatter: `spectrafmt` com regras opinativas e estáveis; integração com LSP e CLI.

## 6. Critérios de qualidade

- Tempo de compilação < 500 ms para projetos médios (100–300 arquivos; medido em CI com hardware padrão).
- Compatibilidade com 95% dos padrões POSIX: API `os` cobre subset amplo; fallback em Windows sem quebrar contratos.
- Curva de aprendizagem < 2 semanas: tutoriais objetivos, mensagens de erro didáticas, sintaxe consistente.
- Performance dentro de 15% das linguagens estabelecidas (Go/Rust/Swift) em benchmarks comuns; regressões bloqueiam merge.

## 7. Estratégia de testes

- Unidade: parser, type checker, IR passes, runtime.
- Integração: pipeline completo (source → AST → SIR → JIT → execução), stdlib e ferramentas (CLI/LSP/formatter).
- Benchmarking contínuo: suíte padronizada (algoritmos, IO, coleções, GC stress); dashboards de tendências.
- Validação cross‑platform: Windows/macOS/Linux; matrizes de CI; smoke tests WASM/WASI.
- Fuzzing/property‑based: geradores de código e invariantes (não travar; mesmo resultado entre modos).

## 8. Documentação

- Especificação formal: EBNF da gramática, regras de resolução de nomes, tipagem (forte/weak/dynamic), semântica avaliativa, ABI/FFI, GC/arena/RAII.
- Tutoriais passo‑a‑passo: do “Hello SpectraLang” ao app CLI e demo WASM; guias “OO”, “Funcional”, “Metaprogramação”.
- Referência da API: stdlib por módulo (assinaturas, exemplos, edge cases).
- Melhores práticas: estilo, performance, segurança, interoperabilidade, layout de projetos, testes, publicação de pacotes.

## 9. Ecossistema

- Comunidade: código aberto (licença MIT/Apache‑2.0), guias de contribuição, código de conduta.
- Repositório central de pacotes: index com verificação, assinatura, semver, busca; políticas de segurança e remoção.
- Versionamento semântico: SpectraLang `vX.Y.Z` com changelog; compatibilidade garantida por major.
- Canal de suporte técnico: fórum/Discord/Slack + issue tracker; SLA comunitário para bugs críticos.

## Esboços formais (resumo)

### Diretivas de modo fraco

```spectra
// No topo do arquivo
directive weak_types on

// Em bloco
directive weak_types on {
  let v: any = 42
}
directive weak_types off
```

### Módulos e import/export

```spectra
module org.example.math
import std.io as io

pub fn add(a: i32, b: i32): i32 {
  return a + b
}
```

### Gramática (EBNF — trecho ilustrativo)

```
Program   := { Directive | ModuleDecl | TopDecl } ;
Directive := "directive" Identifier ("on" | "off") ;
ModuleDecl:= "module" Identifier { "." Identifier } ;
TopDecl   := FnDecl | ClassDecl | TraitDecl | TypeDecl | VarDecl ;
FnDecl    := "fn" Identifier "(" [ ParamList ] ")" [ ":" Type ] Block ;
VarDecl   := ("let" | "var") Identifier [ ":" Type ] "=" Expr ;
ClassDecl := "class" Identifier [ "(" CtorParams ")" ] [ Implements ] ClassBody ;
TraitDecl := "trait" Identifier TraitBody ;
TypeDecl  := "type" Identifier "=" Type ;
Match     := "match" Expr MatchBody ;
```

## Decisões de implementação (sugeridas)

- Linguagem de implementação: Rust.
- Backend JIT: Cranelift; geração para WASM (WASI). AOT via LLVM (opcional).
- IR: SIR (SSA), com passes de otimização e perfilagem.
- Memória: GC geracional/incremental + arenas + RAII/`defer`/`using` + `no_gc`.
- Ferramentas: LSP (`tower-lsp`), DAP (debugger), `spectrafmt` (formatter), `spectra` (package manager).

## Cronograma e marcos

- Fase 1: Protótipo básico (3 meses)
- Fase 2: Recursos avançados (4 meses)
- Fase 3: Otimização e polimento (2 meses)
- Fase 4: Documentação e exemplos (1 mês)

## Critérios de validação

- Tempo de compilação < 500 ms em projetos médios.
- Compatibilidade POSIX 95% (com fallback em Windows).
- Curva de aprendizagem < 2 semanas para programadores experientes.
- Performance dentro de 15% das linguagens estabelecidas em benchmarks comuns.

## Próximos passos imediatos

### 1. Validação de escopo e orçamento

- **Mapear stakeholders:** registrar patrocinadores, liderança técnica, responsáveis por produto e documentação; levantar disponibilidade e expectativas.
- **Consolidar premissas:** sintetizar metas (paradigmas, performance, tooling) e restrições de custo/prazo em um briefing de 2 páginas; incluir riscos de alta prioridade.
- **Oficina de alinhamento (<= 4h):** revisar metas por fase, validar orçamento estimado (equipes e infraestrutura), definir critérios de sucesso e tolerâncias; documentar decisões e itens em aberto.
- **Plano de ação pós-oficina:** atualizar cronograma ou escopo conforme ajustes aprovados, apontar gaps de recursos e aprovações pendentes.
- **Entregáveis:** ata assinada pelos stakeholders, matriz RACI enxuta, planilha de orçamento revisada com baseline liberado.

### 2. Backlog priorizado (Epics → Features → Stories)

- **Epic F1 – Núcleo do Compilador:**
  - Feature: Lexer/Parser robustos ✅ (Mês 1 concluído) → Stories entregues: spans completos, parser com módulos/funções/blocos, testes automatizados.
  - Feature: Analisador semântico básico (iteração 1.4 em andamento) → Stories entregues: escopos hierárquicos, detecção de redefinições, validação de `return`, integração na CLI, detecção de variáveis/parâmetros não utilizados com suporte a `_`, inferência/checagem de tipos primitivos em literais/operadores/`return`, registro de assinaturas de função com verificação de chamadas (aridade/tipos), visibilidade básica via `pub` exportando apenas funções públicas, diagnósticos de imports desconhecidos/auto-referenciados e resolução multi-módulo com compartilhamento automático de funções públicas e detecção de conflitos entre módulos. Próximas Stories: controlar reexportações e `export` globais, ampliar a tabela de símbolos para outros itens top-level, propagar tipos compostos e fluxos condicionais, e conectar as informações de assinatura/tipo ao futuro gerador de SIR.
  - Feature: Tipagem básica + SIR (próximo) → Stories: resolver tipos primitivos, gerar SIR SSA inicial, validar round-trip SIR→JIT→execução em CLI.
- **Epic F2 – Linguagem avançada:**
  - Feature: OO e generics → Stories: suportar `class/trait/impl`, herança simples com mixins, monomorfização de generics.
  - Feature: Módulos e pacotes → Stories: resolver imports canônicos, implementar manifesto `spectra.toml`, comando `spectra add`.
- **Epic F3 – Performance e runtime:**
  - Feature: GC geracional/arenas → Stories: promover objetos quente/frio, implementar arenas `Region`, expor atributo `no_gc`.
  - Feature: Compatibilidade POSIX → Stories: camada `os` unificada, testes smoke em Linux/macOS/Windows.
- **Epic F4 – Documentação e ecossistema:**
  - Feature: Especificação formal → Stories: publicar EBNF completa, documentar regras de tipagem forte/fraca, redigir guia de metaprogramação.
  - Feature: Portal comunitário → Stories: configurar site statico, abrir repositório de pacotes, publicar código de conduta.
- **Governança contínua:** revisar backlog a cada marco mensal, priorizar stories críticos por risco (compilador, runtime, tooling) e ligar itens a métricas de qualidade.

### 3. Protótipo inicial e pipeline de CI

- **Preparação do repositório:** criar monorepo (`compiler`, `runtime`, `tools`, `docs`), ativar proteção de branch principal e template de PR.
- **Prototipagem lexer/parser em Rust:** decidir crates base (`logos` ou implementação própria), definir AST imutável, implementar recuperação de erros com spans; rodar fuzzing leve com `cargo-fuzz` em expressões.
- **SIR e JIT baseline:** integrar Cranelift minimal, gerar código para funções puras, validar execução via CLI `spectra run` para exemplos `hello world`.
- **Pipeline de build/test:** configurar GitHub Actions (ou Azure DevOps) com jobs Windows/Linux; etapas: `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test`, upload de artefato CLI nightly.
- **Métricas iniciais:** registrar dashboards de tempo de compilação para 3 programas sintéticos, falhas de parsing e cobertura de testes; definir limite de regressão (quebra build se >20% piora).
- **Documentação viva:** abrir pasta `docs/decisions` com ADR sobre escolhas do lexer e JIT; atualizar README com instruções de build, critérios de contribuição e link para backlog.