A seguir está o plano detalhado para criar a linguagem de programação SpectraLang do zero, alinhado ao documento “Especificações iniciais.md” e organizado em seções que cobrem todas as áreas solicitadas. No fim, incluo trilhas de progresso, relatório do que foi feito e próximos passos recomendados.

Visão geral

- Objetivo: criar uma linguagem moderna, simples e expressiva, com suporte a OO, procedural e funcional; tipagem forte com opção de modo fraco; compilação JIT multi-alvo (x86, ARM, WASM); GC com controle manual; documentação sólida; ferramentas e ecossistema completos.
- Stack sugerido para implementação: Rust (seguro, performático e multiplataforma) + Cranelift para JIT (leve e simples) + WASM para execução em navegador/sandbox; opcionalmente LLVM AOT em fase avançada se necessário.
1. Características técnicas
- Paradigmas
  - OO: classes, herança simples com mixins, interfaces (traits), encapsulamento (public/private/protected), polimorfismo (virtual dynamic e estático via generics), construtores e destrutores, sobrecarga de métodos proibida por padrão, mas permitida por assinatura distinta (nome + tipo).
  - Procedural: funções top‑level, escopo lexical, módulos, subrotinas, constantes globais controladas.
  - Funcional: funções de primeira classe, closures, imutabilidade por padrão (let), mutabilidade explícita (var), map/filter/reduce, pattern‑matching, ADTs (enum/union), currying opcional, pipe operator |>.
- Sistema de tipos
  - Estático, forte, com inferência local; tipos primitivos (i8..i64, u8..u64, f32, f64, bool, char, string), compostos (array, slice, tuple), registros (struct), classes, enums, union (sum types), option/result, generics paramétricos com constraints (trait bounds).
  - Modo fraco opt‑in: diretivas no escopo de módulo/arquivo/bloqueio. Proposta:
    - directive weak_types on/off no topo do arquivo ou em bloco.
    - Tipo dynamic/any habilitado apenas em modo fraco; conversões implícitas permitidas entre primitivos com checagem em tempo de execução.
    - Barreiras de tipo claras quando interagir com código forte: cast seguro (as?) com fallback em runtime.
- Compilação
  - JIT incremental com cache de funções e especialização por tipo (inline caches). IR própria (SIR) em SSA.
  - Backends: Cranelift (x86_64, aarch64), gerador para WASM; AOT opcional na Fase 3 via plugin (LLVM) se metas de performance pedirem.
  - Suporte a inline IR dumps e perf counters; desativar JIT para testes determinísticos.
- Memória
  - GC geracional/incremental com write barriers; pausas curtas e configuráveis.
  - Controle manual opt‑in:
    - Arenas (Region) para alocações de curta duração; clear/reset explícito.
    - RAII com defer/using para liberar recursos (files, socket, manual handles).
    - Atributo no_gc para funções críticas e “ManualRef
      ” para ciclos de vida controlados; unsafe block limitado exige justificativa e análise no linter.
- Estruturas de dados
  - Built‑ins e stdlib: Array, Slice, Vector, List, Deque, Queue, Stack, HashMap, OrderedMap, Set, BitSet, Heap/PriorityQueue.
  - Módulos extras: Graph, Tree/BTree, RingBuffer, BloomFilter, LRUCache.
  - Todas genéricas, com iteradores e algoritmos (sort, binary_search, join, group_by).
- Documentação sólida
  - Especificação formal (EBNF e regras semânticas).
  - Referência completa da linguagem e da stdlib.
  - Tutoriais e guias de melhores práticas.
  - Exemplos de ponta a ponta (CLI, web via WASM, lib).
2. Especificações da sintaxe
- Filosofia: limpa, consistente, poucas palavras‑chave em inglês, legibilidade e previsibilidade.
- Palavras‑chave núcleo
  - Módulos: module, import, export, pub, use
  - Tipos e definição: type, struct, enum, union, trait, impl, class, extends, new
  - Controle: if, else, match, for, while, break, continue, return, defer, using
  - Funções: fn, async, await, yield
  - Erros: try, catch, throw
  - Mutabilidade: let (imutável), var (mutável)
  - Metaprogramação controlada: macro, comptime (execução restrita)
- Exemplos (só para ilustrar sem formalizar totalmente)
  - Declaração:
    - module core.math
    - import std.io as io
    - let x = 10
    - var y = 20
  - Função:
    - fn add(a: i32, b: i32): i32 { return a + b }
  - Classe/trait:
    - trait Drawable { fn draw(self) }
    - class Circle(radius: f64) implements Drawable { fn draw(self) { /* ... */ } }
  - Pattern matching:
    - match value { Some(v) => io.print(v), None => io.print("empty") }
  - Diretiva modo fraco:
    - directive weak_types on
    - let v: any = 42
- Metaprogramação controlada
  - Macros higiénicas (AST macros) sem acesso a IO/rede; expansões limitadas.
  - comptime: funções avaliadas em compilação com sandbox; permitem gerar código, validar invariantes, mas sem efeitos colaterais.
- Sistema de módulos integrado
  - Resolução via nomes canônicos (org/pkg@version/path); interoperável com gerenciador de pacotes.
  - import/export com escopo explícito; “pub” para visibilidade.
3. Arquitetura do compilador
- Linguagem de implementação: Rust.
- Frontend
  - Lexer baseado em autômato gerado; suporte a Unicode; tokens com localização; recuperação de erro.
  - Parser LL(k) ou LR(1) com Pratt para expressões; AST imutável; mensagens de erro ricas.
  - Checagem semântica: resolução de nomes, tipos, lifetimes simplificados, verificações de visibilidade e mutabilidade; lints.
- Middle‑end
  - SIR (Spectra IR) em SSA: blocos básicos, instruções simples e consistentes.
  - Otimizações: constant folding, DCE, copy‑prop, inlining seguro, loop‑invariant code motion, strength reduction, escape analysis, specialization por tipos monomorfizados.
  - Perfilagem: contadores e hints para JIT.
- Backend
  - Cranelift para código nativo x86_64/aarch64 com JIT e fallback AOT.
  - Lowering para WASM (WASI) mantendo semântica; possibilitar execução em navegador.
  - Suporte a chamadas FFI com fronteiras seguras; ABI documentado.
- Runtime e stdlib
  - Núcleo: memória, GC, coleções, strings, tempo, IO, OS (abstração portátil), fmt (format), math.
  - Erros: Result, Option, try/catch, backtraces configuráveis.
  - Segurança: sem segredos em código; carregar de env-var; timeouts/retries em IO.
  - Observabilidade: logs para stderr, níveis de log, métricas, /health (modo server).
4. Plano de desenvolvimento
- Fase 1 (3 meses): Protótipo básico
  - Mês 1: Lexer, parser, AST, erros; sintaxe essencial; “fn/let/var/if/while/return”.
  - Mês 2: Tipagem forte (primitivos, funções), SIR e JIT baseline via Cranelift; stdlib mínima (io/fmt).
  - Mês 3: GC inicial, runtime básico, CLI “spectra” (build/run/test), integração WASM mínima; testes de unidade/integração.
- Fase 2 (4 meses): Recursos avançados
  - Mês 4: OO (class/trait/impl/extends), generics, pattern matching, ADTs.
  - Mês 5: Módulos e pacote manager (init, add, build), metaprogramação controlada (macro/comptime).
  - Mês 6: LSP (autocompletar, hover, diagnostics), formatter (spectrafmt), DAP (debugger).
  - Mês 7: Otimizações middle‑end, caches JIT, especialização por tipo, melhorias WASM.
- Fase 3 (2 meses): Otimização e polimento
  - Mês 8: Performance (<15% do alvo), gating de qualidade; POSIX 95% via camada de compat; cross‑platform.
  - Mês 9: Refino do GC (generational/incremental), arenas, no_gc, RAII; observabilidade/logs; lints.
- Fase 4 (1 mês): Documentação e exemplos
  - Mês 10: Especificação formal EBNF, referência completa, tutoriais, exemplos end‑to‑end (CLI e WASM), melhores práticas; guias de migração e releases.
5. Ferramentas auxiliares
- IDE: LSP em Rust (tower-lsp) suportando autocompletar, hover, goto definition, rename, diagnostics em tempo real.
- Debugger: DAP (Debug Adapter Protocol) com breakpoints, step, watch, inspeção de variáveis; integração com VS Code e outros.
- Gerenciador de pacotes: “spectra”
  - Comandos: init, add, remove, update, build, run, test, publish.
  - Manifesto: spectra.toml com dependências e versões semânticas; lockfile pinado.
- Formatter: “spectrafmt”
  - Regras opinativas e estáveis; integração com LSP e CLI.
6. Critérios de qualidade
- Compilação < 500 ms para projetos médios (ex.: 100–300 arquivos; medido em CI com hardware padrão).
- Compatibilidade POSIX 95%: API “os” cobre subset amplo; fallback em Windows sem quebrar contratos.
- Curva de aprendizagem < 2 semanas: tutoriais objetivos, mensagens de erro didáticas, sintaxe consistente.
- Performance dentro de 15% das linguagens estabelecidas (ex.: Go/Rust/Swift em benchmarks comuns): micro/macrobench com tolerâncias definidas; regressão bloqueia merge.
7. Estratégia de testes
- Unidade: parser, tipe checker, IR passes, runtime.
- Integração: pipeline completo (source → AST → SIR → JIT → execução), stdlib e ferramentas (CLI/LSP/formatter).
- Benchmarking contínuo: suíte padronizada (algoritmos, IO, coleções, GC stress); dashboards de tendências.
- Validação cross‑platform: Windows/macOS/Linux; matrizes de CI; smoke tests WASM/WASI.
- Fuzzing/property-based: geradores de código e asserts de invariantes (não travar, mesmo resultado entre modos).
8. Documentação
- Especificação formal: EBNF da gramática, regras de resolução de nomes, tipagem (forte/weak/dynamic), semântica avaliativa, ABI/FFI, GC/arena/RAII.
- Tutoriais passo‑a‑passo: do “Hello SpectraLang” ao app CLI e demo WASM; guias “OO”, “Funcional”, “Metaprogramação”.
- Referência da API: stdlib por módulo (assinaturas, exemplos, edge cases).
- Melhores práticas: estilo, performance, segurança, interoperabilidade, layout de projetos, testes, publicação de pacotes.
9. Ecossistema
- Comunidade: código aberto (licença permissiva, ex.: Apache‑2.0/MIT), guias de contribuição, código de conduta.
- Repositório central de pacotes: index com verificação, assinatura, semver, busca; políticas de segurança e remoção.
- Versionamento semântico: SpectraLang vX.Y.Z com changelog; compatibilidade garantida por major.
- Canal de suporte técnico: fórum/Discord/Slack + issue tracker; SLA comunitário para bugs críticos.
Esboços formais (resumo)

- Diretivas de modo fraco
  - No topo do arquivo: directive weak_types on
  - Em bloco: directive weak_types on { /* código com any/conversões */ } directive weak_types off
- Módulos e import
  - module org.example.math
  - import std.io as io
  - export pub fn add(a: i32, b: i32): i32
- EBNF (trecho ilustrativo)
  - Program := { Directive | ModuleDecl | TopDecl }
  - TopDecl := FnDecl | ClassDecl | TraitDecl | TypeDecl | VarDecl
  - FnDecl := "fn" Identifier "(" [ParamList] ")" [":" Type] Block
  - VarDecl := ("let" | "var") Identifier [":" Type] "=" Expr

