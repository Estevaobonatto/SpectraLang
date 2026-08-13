# SpectraLang Stable Core e STD Tipada — Plano de Implementação

**Intent:** levar o subconjunto geral da linguagem e a standard library de um
baseline funcional, parcialmente tipado e com contratos duplicados para um
estado estável, sem sucessos falsos, sem placeholders e com evidência de
produção nas fronteiras externas.

**Current Behavior:** o pipeline lexer/parser/semântica/midend/backend/runtime
executa uma superfície ampla e os testes locais estão verdes, mas `class` é
apenas uma palavra reservada sem AST/parser, `static` é aceito na semântica mas
não é consumido pelo lowering/backend, `Type::Unknown` ainda funciona como
compatibilidade permissiva, e o midend fabrica valores SSA/zero em caminhos
desconhecidos. A STD tem aproximadamente 26 mil linhas em
`runtime/src/stdlib/mod.rs`, usa handles inteiros por domínio, repete contratos
em semântica/runtime/API/lowering e publica collections/Option/Result com
`int`/`unknown`/sentinelas. PostgreSQL/Redis e TLS externo não têm evidência
local obrigatória neste workspace.

**Expected Outcome:** um contrato estável explícito e validável para a
linguagem; `class` removida da superfície utilizável e documentada como
reserved/deferred; `static` implementado de AST a JIT/AOT; semântica e lowering
fail-closed; um catálogo tipado único para STD/ABI/API; collections, Option,
Result e erros estruturados tipados; arrays/indexação/iteradores e ownership de
handles definidos; async integrado a rede/API; STD modularizada; e gates
obrigatórios de formato, lint, documentação, PostgreSQL, Redis, OTLP e TLS.

**Target-Perspective Output:** uma pessoa que instala o CLI consegue consultar
o contrato de estabilidade, escrever um programa com `static`, collections
tipadas, `Option`/`Result`, arrays, iteradores e async, receber diagnósticos
determinísticos para código inválido, executar o mesmo programa por JIT e AOT,
e verificar um relatório de release que distingue `passed` de
`skipped_environment` e contém versões/serviços externos sem segredos.

**Truth Owner:** `scripts/language_stability_contract.toml` será a fonte
machine-readable da maturidade da linguagem; `packages/spectra-contract/catalog/stdlib.toml`
será a fonte única dos símbolos, tipos, ABI, bindings e maturidade da STD/API.
As projeções Markdown, tabelas semânticas, descriptors de lowering, registros
runtime e tabelas API serão geradas ou validadas contra essas fontes. Os donos
de implementação continuam sendo `frontend`, `semantic`, `midend`, `backend`,
`runtime`, `web`, `db`, `tooling` e `ecosystem` conforme a camada.
Durante a migração, `scripts/stdlib_contract.toml` será somente uma projeção
gerada/compatível do catálogo novo; ele não poderá ser editado como terceira
fonte de verdade e será removido ou mantido apenas como artefato derivado no
gate final.

**Contract Boundary:**

- fonte Spectra → AST: keywords reservadas, itens, tipos genéricos e spans;
- AST → semântica: tipos resolvidos, símbolos globais, funções host e
  diagnósticos codificados;
- semântica → IR: nenhum símbolo/tipo não resolvido e nenhum valor SSA
  sintético;
- IR → JIT/AOT: `Global`, exact-width, chamadas, handles e verificações têm a
  mesma representação;
- linguagem → STD: catálogo tipado, `Option<T>`, `Result<T,E>`, erros e
  `HandleId` generacional;
- runtime/API → serviços externos: status fail-closed e relatórios
  versionados.

**Cutover:** primeiro congelar o contrato e demover `class`; depois introduzir
o catálogo e os invariantes de erro; em seguida fechar `static` e os tipos
fundamentais; somente então migrar handles, arrays e async. Cada migração deve
converter todos os consumidores internos, exemplos e fixtures para o caminho
novo antes de remover o caminho antigo. Compatibilidade temporária só pode
existir sob um namespace explícito `std.compat`, com teste de depreciação e
critério de remoção na mesma fase; não haverá duas APIs dominantes indefinidas.

**Displaced Path:**

- `class` como feature beta → keyword reserved/deferred e diagnóstico estável;
- listas/maps/Option/Result baseados em `int`/`unknown`/sentinela → APIs
  genéricas tipadas;
- strings de host calls em cinco fontes → catálogo `spectra-contract`;
- `scripts/stdlib_contract.toml` editado manualmente → projeção gerada do
  catálogo, com teste de drift;
- `Type::Unknown` curinga e `next_value()`/zero de fallback → erros explícitos;
- registries independentes de handles → `HandleTable` com geração, tipo,
  ownership e drop;
- lowering monolítico e STD monolítica → módulos por domínio com fachadas
  pequenas;
- `skipped_environment` contado como sucesso de release → gate requerido que
  falha fechado.

**Value Density:** o maior retorno vem de fechar as fronteiras que permitem um
programa inválido parecer compilado: contrato único, erro fail-closed, tipos
genéricos e ownership. A refatoração de arquivos só entra depois desses
contratos para reduzir risco e não para mascarar comportamento incompleto.

**Acceptance Evidence:** o resultado só é aceito com uma combinação de
compilação, execução normal, JIT, AOT, negativos `check --json`, testes Rust,
relatórios versionados, validação de documentação, inspeção de ausência de
duplicação e gates externos requeridos. Um teste local verde sem serviço
externo configurado não é evidência de produção.

**Evidence Lane:** `target/debug/spectralang.exe`, `tests/validation`,
`tests/errors`, `tests/projects/valid`, `scripts/validate_*.py`,
`run_tests.ps1`, `cargo test`, `cargo fmt --check`, `cargo clippy`, CI de
PostgreSQL 16/Redis 7, collector OTLP real e endpoint TLS externo.

**Kill Criteria:** parar a promoção e manter o pacote em `in_progress` se
qualquer código inválido chegar ao backend, se o catálogo deixar símbolos sem
projeção, se uma API antiga continuar sendo caminho padrão, se um gate exigido
for pulado, se houver divergência de maturidade entre contrato e docs, se JIT
e AOT divergirem, ou se `cargo fmt`, lint, documentação ou o relatório
agregado falharem. Não adicionar feature nova enquanto um desses critérios
estiver vermelho.

**Architecture Slice:**

- **Files to create:** `scripts/language_stability_contract.toml`,
  `scripts/render_language_maturity.py`, `scripts/validate_stability_release.py`,
  `packages/spectra-contract/` (crate sem dependência de runtime),
  `packages/spectra-contract/catalog/stdlib.toml`,
  `docs/goals/spectralang-stability-stdlib-hardening/PRE-REVIEW.md`,
  `runtime/src/handles/`, módulos de STD por domínio, fixtures de estabilidade
  e testes de contrato/catalog/handles.
- **Files to modify:** `Cargo.toml`, `compiler/src/ast/`,
  `compiler/src/parser/`, `compiler/src/semantic/`,
  `compiler/src/error.rs`, `compiler/src/pipeline.rs`, `midend/src/ir.rs`,
  `midend/src/lowering/` (após a decomposição), `backend/src/`,
  `runtime/src/stdlib/mod.rs` até K-10, `runtime/src/ffi.rs`, `runtime/src/memory/`,
  `runtime/src/reactor/`, `packages/spectra-api/src/`,
  `tools/spectra-cli/src/`, `scripts/stdlib_contract.toml`,
  `scripts/validate_feature_maturity.py`, `run_tests.ps1`,
  `.github/workflows/ci.yml`, `r2505-postgres.yml`, `r2507-redis.yml`,
  `release.yml`, além dos documentos referenciados pelo contrato.
- **Files to avoid:** não criar um segundo registry de host calls, um segundo
  sistema de handles, uma implementação superficial de `class`, um fallback
  novo no lowering, ou uma API paralela que mantenha `int`/`unknown` como
  caminho padrão. Não alterar `roadmap/roadmap.toml` ou backlog durante as
  fases de código; a sincronização ocorre somente depois da evidência final.
- **Source of truth:** os dois manifests acima; código concreto é o dono da
  execução, e os manifests não podem promover uma capacidade sem fixture e
  prova da camada correspondente.
- **Read path:** manifest → gerador/adapter → semantic registry, lowering
  descriptors, runtime binding, API binding, docs e validator.
- **Write path:** `.spectra` → lexer/parser → semantic typed AST → IR validada
  → JIT/AOT → runtime/serviços; handles e erros cruzam a fronteira somente por
  `SpectraHostCallContext` e `HandleId`.
- **Integration points:** `ModuleRegistry`, `runtime::ffi`, `runtime::abi`,
  Cranelift JIT/AOT, reactor platform adapter, `spectra.api`, drivers DB,
  tracing OTLP, CLI release report e `run_tests.ps1`.
- **Migration/cutover:** cada tarefa abaixo contém o gate que prova o caminho
  novo antes da remoção do antigo; aliases só permanecem até a migração de
  todos os fixtures e docs.
- **Displaced path:** os caminhos antigos listados acima devem ser apagados,
  redirecionados para `std.compat` ou reduzidos a adapters sem autoridade.
- **Acceptance evidence gate:** `K-12` só pode fechar quando todos os gates
  P0/P1 estiverem verdes e o relatório de release não contiver skips
  obrigatórios.

**Plan Review Gate:** Requires PRE review before execution.

## Decisões de escopo

### Subconjunto estável final

O alvo não é declarar todos os tokens existentes como estáveis. O contrato
final inclui módulos/projetos, imports/visibility, funções/métodos, records,
enums, traits/impl/dyn, generics validados, controle de fluxo atual,
closures por valor, `const`, `static`, exact-width validado, arrays tipados,
iteradores, collections tipadas, Option/Result, STD de I/O/tempo/strings/
conversão/FS/env, async integrado e API HTTP suportada.

Ficam fora do stable core: `class` tradicional, `repeat/until`, `foreach`,
`goto`, `yield`, Unicode identifiers, literais avançados, captures mutáveis
além do contrato definido, backends nativos não certificados e superfícies ML
explicitamente marcadas como baseline/simulation. `std.serve` in-process,
workers distribuídos simulados e embeddings hash continuam disponíveis apenas
com classificação honesta; não serão promovidos por este plano.

### Decisão sobre `class`

O plano remove `class` da promessa de linguagem: mantém a palavra como
reserved, emite erro de parser estável e orienta para `struct` + `impl` +
`trait`. Implementar classes completas, herança de campos, `override`,
`super`, layout, ABI e drop será um objetivo separado se houver requisito de
produto. Não será criado um açúcar parcial para `struct`.

### Decisão sobre `static`

`static` será um global mutável de módulo com initializer constante, acesso e
atribuição tipados, visibility, import cross-module e suporte JIT/AOT. Inicialização
dinâmica, ordem entre módulos e compartilhamento não sincronizado entre tasks
serão rejeitados por diagnóstico explícito até existir contrato próprio. O IR
`Global` já existente será reutilizado; não será criado um registry global
paralelo no runtime.

## Mapa de dependências

| Onda | Tarefas | Dependência | Pode rodar em paralelo |
| --- | --- | --- | --- |
| W0 | K-00 | nenhuma | não |
| W1 | K-01, K-02 | K-00 | sim, com integração ao final |
| W2 | K-03 | K-01 + K-02 | não |
| W3 | K-04, K-05 | K-03 | sim, desde que não compartilhem arquivos durante a implementação |
| W4 | K-06, K-07 | K-05; K-07 também exige K-03 | sim |
| W5 | K-08 | K-05 + K-06 + R-2902 auditado | não |
| W6 | K-09 | K-03 + K-05 + K-06 + K-08 | parcialmente, por dono |
| W7 | K-10 | K-02 + K-05 + K-06 | sim por crate, depois integração |
| W8 | K-11 | K-04…K-10 | não |
| W9 | K-12 | todos os anteriores | não |

## Cobertura direta dos gaps P0/P1

| Gap | Tarefa e decisão | Critério que impede falso-positivo |
| --- | --- | --- |
| P0 — subconjunto estável | K-01; manifest único, `class` reserved/deferred e lista explícita de stable | CLI, docs, manifest e positivos/negativos precisam coincidir |
| P0 — `class` | K-01; manter keyword reservada com diagnóstico, sem AST/semântica de classe | nenhuma declaração chega à semântica e nenhum exemplo promete classes |
| P0 — `static` | K-04; reutilizar `IR::Module.globals` com initializer constante e JIT/AOT | mutação, visibility, cross-module e equivalência JIT/AOT |
| P0 — placeholders/`Unknown` | K-03; semantic/midend/backend fail-closed e verifier estrutural | código inválido falha antes do codegen; nenhum Value/zero sintético |
| P0 — catálogo | K-02; `spectra-contract` como fonte única e `stdlib_contract.toml` derivado | nameset, bindings, ABI, docs e maturidade sem drift |
| P0 — STD tipada | K-05; type application, collections, Option/Result e `Error` estruturado | sem `unknown`, `-1`, string vazia ou status ambíguo nas assinaturas estáveis |
| P0 — serviços externos | K-09/K-11; PostgreSQL 16, Redis 7, collector OTLP e TLS externo | `--required` falha em ausência/skip; relatório registra versão e segredo redigido |
| P1 — exact-width | K-07; matriz de tipo/operação/ABI/JIT/AOT | só promove se overflow, narrowing e C ABI forem reproduzidos e verdes |
| P1 — arrays/iteradores | K-08; protocolo `Iterator<T>` para range/array/List/Map | `for` não tem branch semântico paralelo e array vazio não infere arbitrariamente |
| P1 — handles/Drop | K-06; `HandleId` com generation e ownership central | use-after-free, type mismatch, transfer e drop de todos os caminhos |
| P1 — async integrado | K-09; reactor/API/DB/TLS/OTLP com cancellation e `Send/Sync` | I/O real, parentage, timeout/backpressure e matriz epoll/IOCP/kqueue |
| P1 — manutenção/gates | K-10/K-11; decomposição, fmt, Clippy, docs e release aggregator | qualquer skip obrigatório ou arquivo fora do limite sem justificativa falha |

## Tarefas executáveis

Cada tarefa abaixo deve produzir o output indicado antes de abrir a próxima
dependência. Os IDs `K-*` são IDs deste pacote de execução, não novos IDs da
roadmap.

### K-00 — Baseline reproduzível e decisão de contrato

- **Owner:** `tooling`, com revisão de todos os owners de camada.
- **Arquivos:** nenhum arquivo de produção; relatórios somente em `target/`.
- **Escopo:** capturar branch/worktree, versão do binário compilado, matriz
  atual, estado de `cargo fmt`, lint, R-3007, bug hunt, R-2003, R-2107,
  language guide e gates externos. Registrar que `skipped_environment` não
  certifica PostgreSQL/Redis/TLS.
- **Output:** `target/stability/baseline.json` com comandos, exit codes,
  revisões, skips, falhas e classificação `implemented`, `partial`,
  `simulation` ou `unproven`.
- **Verificação:**
  `cargo test --workspace --all-targets --no-fail-fast`; `cargo fmt --all -- --check`;
  `cargo clippy --workspace --all-targets -- -D warnings`;
  `python scripts/validate_feature_maturity.py --binary target/debug/spectralang.exe`;
  `python scripts/validate_r3007_stdlib_contract.py --binary target/debug/spectralang.exe`;
  `python scripts/validate_stdlib_core_bug_hunt.py --binary target/debug/spectralang.exe`;
  `python scripts/validate_r2003_base_regression_audit.py --binary target/debug/spectralang.exe`;
  `git diff --check`.
- **Aceite:** o relatório diferencia verde real de skip e vira o baseline
  imutável para comparar cada onda.
- **Paralelismo:** nenhum; é o gate de entrada.

### K-01 — Contrato de estabilidade e remoção de `class` da superfície

- **Owner:** `frontend` + `tooling` + `ecosystem`.
- **Arquivos:** criar `scripts/language_stability_contract.toml`,
  `tests/errors/class_declaration_reserved.spectra` e
  `scripts/render_language_maturity.py`; modificar
  `compiler/src/parser/item.rs`,
  `scripts/validate_feature_maturity.py`, `docs/language-feature-maturity.md`,
  `docs/frontend/frontend-coverage-audit.md`,
  `docs/semantic/semantic-coverage-audit.md`,
  `docs/midend/lowering-backend-coverage-audit.md`, `README.md`,
  `SYNTAX_SUMMARY.md`, `docs/spectralang-language-guide.html` e a referência
  OOP somente onde a maturidade estiver publicada.
- **Escopo:** definir registros por feature com status, lexer/parser/semantic/
  lowering/backend/runtime, fixture positivo, fixture negativo e comando de
  prova. Tornar o Markdown uma projeção validada do manifest. Marcar `class`
  como `reserved`, remover a classificação beta e documentar o paradigma
  trait-first. Manter `--list-experimental` vazio.
- **Displaced path:** “keyword reconhecida = feature parcial” deixa de ser
  aceito; nenhuma doc ou exemplo pode prometer declaração de classe.
- **Output:** contrato versionado e relatório de sincronização sem features
  stale.
- **Verificação:** `python scripts/validate_feature_maturity.py --binary target/debug/spectralang.exe`;
  `python scripts/validate_language_guide.py`; `spectralang --list-experimental`;
  `check --json tests/errors/class_declaration_reserved.spectra` com diagnóstico
  estável; `rg` de `class` em docs/exemplos para confirmar apenas reserved/
  deferred.
- **Aceite:** a política e o CLI mostram a mesma classificação; o usuário
  recebe orientação para `struct`/`trait`, e nenhuma declaração de classe chega
  à semântica.
- **Paralelismo:** pode rodar em paralelo com K-02 após K-00.

### K-02 — Catálogo único tipado de STD/API/ABI/lowering

- **Owner:** `tooling` (schema/generator) + `runtime` (binding) + revisão
  `semantic`, `midend`, `backend`, `web` e `db`.
- **Arquivos:** criar `packages/spectra-contract/Cargo.toml`,
  `packages/spectra-contract/build.rs`,
  `packages/spectra-contract/src/lib.rs`,
  `packages/spectra-contract/catalog/stdlib.toml` e testes do crate; modificar
  `Cargo.toml`, `compiler/src/semantic/builtin_modules.rs`,
  `runtime/src/stdlib/mod.rs`, `packages/spectra-api/src/lib.rs`,
  `midend/src/lowering.rs`, `backend/src/hostcall_abi.rs`,
  `scripts/stdlib_contract.toml` e `scripts/validate_r3007_stdlib_contract.py`.
- **Escopo:** o catálogo declara path, namespace, tipo/genéricos, parâmetros,
  retorno, ABI, efeitos, error model, binding Rust, maturity, owner, docs e
  fixtures. O crate não depende de runtime/compiler; gera descriptors para
  semantic registry, lowering e validação. Runtime/API mantêm apenas adapters
  de função para bindings concretos, sem repetir paths. `runtime/src/abi.rs`
  continua dono do ABI interno de imports nativos e passa a referenciar o
  catálogo onde houver host call público.
- **Displaced path:** arrays manuais em `builtin_modules.rs`, constantes e
  `register_host_function` duplicados no runtime, `HostCallSpec` com paths
  manuais na API e descritores host em `lowering.rs` deixam de ser autoridade.
- **Output:** `spectra-contract` compilável, catalog snapshot e relatório de
  cobertura que prova uma entrada semântica, lowering, runtime/API e docs por
  símbolo.
- **Verificação:** `cargo test -p spectra-contract`; `cargo test -p spectra-compiler -p spectra-midend -p spectra-runtime -p spectra-api`;
  `python scripts/validate_r3007_stdlib_contract.py --manifest scripts/stdlib_contract.toml --binary target/debug/spectralang.exe`;
  teste que falha em paths duplicados e símbolos sem binding; comparação de
  contagem/nameset antes e depois da migração.
- **Aceite:** nenhum símbolo suportado desaparece; todo símbolo tem uma única
  declaração e todas as projeções são derivadas; o R-3007 continua detectando
  drift, mas o drift não pode mais ser criado por quatro listas independentes.
- **Paralelismo:** pode rodar em paralelo com K-01; integração obrigatória antes
  de K-03.

### K-03 — Pipeline fail-closed e eliminação de placeholders

- **Owner:** `semantic` + `midend` + `backend`.
- **Arquivos:** modificar `compiler/src/ast/mod.rs`,
  `compiler/src/semantic/mod.rs`, `compiler/src/semantic/builtin_modules.rs`,
  `compiler/src/error.rs`, `compiler/src/pipeline.rs`, `midend/src/lowering.rs`,
  `midend/src/ir.rs`, `midend/src/passes/verification.rs`,
  `backend/src/codegen.rs`, `backend/src/aot.rs`,
  `tests/errors/stability_unknown_identifier.spectra`,
  `tests/errors/stability_unknown_call.spectra`,
  `tests/errors/stability_unknown_type.spectra`,
  `tests/errors/stability_empty_array.spectra` e testes de compiler/midend.
- **Escopo:** separar estado de inferência de tipo não resolvido do tipo aceito
  pelo AST final; `Type::Unknown` não pode ser wildcard em `types_match` nem
  sobreviver a semantic success. Fazer lowering de expressão, identifier,
  call, aggregate e closure retornar erro tipado ou acumular erro fail-closed.
  Remover `ir_func.next_value()` para desconhecidos, `build_const_int(0)` para
  funções ausentes e o bypass de closures. Chamada closure deve usar caminho
  `CallIndirect` tipado ou produzir erro semântico. O verifier deve rejeitar
  uso de Value sem definição, tipo desconhecido ou global ausente.
- **Displaced path:** o backend nunca mais “compila” código inválido usando
  valor sintético. Zeros legítimos continuam permitidos apenas como constantes
  de programa ou payload acompanhado de status de sucesso; zero de erro deve
  ser substituído por status/Result.
- **Output:** invariantes de pipeline documentados, erro de midend/backend
  convertido em diagnóstico CLI estável e relatório de placeholders zero.
- **Verificação:** novos negativos para identifier/call/type/generic/empty-array
  não resolvidos; `check --json` deve falhar antes de IR; `cargo test -p spectra-midend`;
  `cargo test -p spectra-backend`; IR dump sem Value fantasma; JIT/AOT positivos
  existentes permanecem verdes.
- **Aceite:** nenhum caso inválido chega ao codegen; busca estrutural não encontra
  os fallbacks proibidos fora de helpers explicitamente marcados para constantes
  legítimas; diagnostics apontam a origem e não um crash interno.
- **Paralelismo:** não; depende do catálogo e bloqueia static/STD tipada.

**Unidades executáveis de K-03:**

| Unidade | Arquivos e saída | Verificação/aceite | Paralelismo |
| --- | --- | --- | --- |
| K-03a semântica | `compiler/src/ast/mod.rs`, `compiler/src/semantic/mod.rs`, `compiler/src/error.rs`, `compiler/src/pipeline.rs`; estado de inferência separado do tipo final e diagnósticos codificados | negativos de símbolo/tipo/genérico; `check --json` falha antes do midend; nenhum `Type::Unknown` sobrevive a semantic success | serial |
| K-03b midend | `midend/src/lowering.rs`, `midend/src/ir.rs`, `midend/src/passes/verification.rs`; lowering retorna erro e verifier rejeita Value/global/tipo inválido | `cargo test -p spectra-midend`; IR dump sem Value fantasma; `verify_module` cobre cada erro | após K-03a |
| K-03c backend | `backend/src/codegen.rs`, `backend/src/aot.rs`; codegen recebe apenas IR validada e preserva erro de compilação | `cargo test -p spectra-backend`; JIT/AOT negativos não emitem artefato e positivos existentes permanecem verdes | após K-03b |

### K-04 — `static` de ponta a ponta

- **Owner:** `frontend` + `semantic` + `midend` + `backend`.
- **Arquivos:** modificar `compiler/src/semantic/mod.rs`,
  `compiler/src/semantic/module_registry.rs`, `midend/src/ir.rs`,
  `midend/src/lowering.rs`, `midend/src/pretty.rs`, `backend/src/codegen.rs`,
  `backend/src/aot.rs`, `tools/spectra-cli/src/compiler_integration.rs`,
  `tests/validation/stability_static_mutation.spectra`,
  `tests/validation/stability_static_aot.spectra`,
  `tests/errors/stability_static_dynamic_initializer.spectra`,
  `tests/projects/valid/stability_static_cross_module/` e fixtures de validação.
- **Escopo:** registrar statics em tabela global com nome totalmente qualificado,
  visibility, tipo concreto, initializer `Constant` e mutabilidade; coletar
  `Item::Static` no `IR::Module.globals`; emitir load/store global em JIT e
  data symbol relocável em AOT; resolver imports/reexports cross-module;
  rejeitar initializer dinâmico, tipo não resolvido, colisão, acesso privado e
  uso não seguro através de task/spawn até existir política `Sync`.
- **Displaced path:** o registro semântico sem lowering e o fallback de
  identifier que ignora static são removidos; não criar um global paralelo no
  runtime.
- **Output:** fixture de `static` local, mutação, visibility, import e AOT;
  diagnóstico negativo para initializer dinâmico e corrida não autorizada.
- **Verificação:** `cargo test -p spectra-compiler -p spectra-midend -p spectra-backend`;
  `spectralang check/run --json` dos fixtures; `--dump-ir` contendo globals;
  `--emit-object` e `--emit-exe` executando o mesmo resultado do JIT.
- **Aceite:** static é observável e mutável nos caminhos suportados, não é
  confundido com const, e a matriz de maturidade só o promove após JIT/AOT e
  cross-module verdes.
- **Paralelismo:** pode iniciar com K-05 após K-03 se cada tarefa tiver arquivos
  disjuntos; integração semântica é serial.

### K-05 — ADTs genéricos, collections, Option/Result e erros estruturados

- **Owner:** `semantic` + `runtime` + `midend`, com `tooling` para docs e
  `ecosystem` para migração de exemplos.
- **Arquivos:** modificar `compiler/src/ast/mod.rs`,
  `compiler/src/semantic/mod.rs`, `compiler/src/semantic/builtin_modules.rs`,
  `midend/src/ir.rs`, `midend/src/lowering.rs`,
  `runtime/src/stdlib/mod.rs`, `runtime/src/ffi.rs`,
  `docs/reference/05-stdlib.md`, `docs/runtime/standard-library.md`,
  `scripts/stdlib_contract.toml`,
  `tests/validation/stability_typed_collections.spectra`,
  `tests/errors/stability_collection_type_mismatch.spectra`,
  todos os fixtures existentes que usam collections/option/result/fs/env e testes
  de tipos.
- **Escopo:** introduzir representação de aplicação genérica (`Option<T>`,
  `Result<T,E>`, `List<T>`, `Map<K,V>`, `Set<T>`, `Iterator<T>`) preservando
  argumentos de tipo na semântica, monomorfização e IR. Definir APIs sem
  `unknown`: `get/pop` retornam `Option<T>`, mutações e I/O retornam
  `Result<_, Error>`, `env_get/arg` retornam `Option<string>`, e falhas têm
  código, mensagem, contexto e origem. Implementar pattern matching e métodos
  tipados para payloads; não introduzir `?` sem especificação própria, usando
  `match`/propagação já suportada até essa decisão.
- **API mínima estável:** `List<T>`, `Map<K,V>`, `Set<T>` e `Iterator<T>` com
  construção, comprimento, acesso seguro, mutação, remoção, iteração e
  comparação; `Option<T>` com predicates/map/unwrap_or; `Result<T,E>` com
  predicates/map/map_err/unwrap_or; `ErrorCode` como enum fechado e `Error`
  com `code`, `message`, `operation`, origem/span, detalhes tipados,
  `retryable` e causa preservada sem depender de string sentinel.
- **Displaced path:** os nomes antigos de handle inteiro não podem continuar
  como assinatura principal. Durante a migração, `std.compat.collections` e
  `std.compat.option/result` são os únicos adapters, com warning/deprecation,
  e serão removidos antes de K-12.
- **Output:** catálogo de tipos e erros, contratos docs atualizados, fixtures
  positivos genéricos e negativos de incompatibilidade/payload.
- **Verificação:** `cargo test --workspace` dos crates afetados; fixtures
  JIT/AOT para `List<int>`, `List<string>`, `Option<T>`, `Result<T,E>` e
  nested generics; `check --json` para mismatch; R-3007 sem assinaturas
  `unknown` nos símbolos estáveis; nenhum sentinel `-1`/string vazia em erro
  sem status.
- **Aceite:** o compilador impede misturar payloads incompatíveis e o usuário
  consegue tratar falhas sem adivinhar sentinelas; a migração remove o caminho
  compatível como autoridade.
- **Paralelismo:** `Option/Result` pode ser desenvolvido em paralelo com
  collections, mas ambos precisam do type application e do catálogo K-02.

**Unidades executáveis de K-05:**

| Unidade | Arquivos e saída | Verificação/aceite | Paralelismo |
| --- | --- | --- | --- |
| K-05a type application | `compiler/src/ast/mod.rs`, `compiler/src/semantic/mod.rs`, `midend/src/ir.rs`, `midend/src/lowering.rs`; aplicação genérica preservada até o IR | positivos/negativos de `List<T>`, `Option<T>`, `Result<T,E>` e nested generics; JIT/AOT não apagam os argumentos | após K-03 |
| K-05b Option/Result/Error | `compiler/src/semantic/builtin_modules.rs`, `runtime/src/stdlib/mod.rs`, `runtime/src/ffi.rs`, catálogo e docs; payload/error estruturados | R-3007 sem `unknown` em símbolos estáveis; `match`, `map`, `map_err`, `unwrap_or` e status sem sentinel | após K-05a, em paralelo com K-05c |
| K-05c collections | `runtime/src/stdlib/mod.rs`, lowering, catálogo, fixtures e docs; `List/Map/Set/Iterator` migram por namespace | `get/pop/env_get/arg` não usam `-1`/string vazia como erro; tipos incompatíveis falham no `check --json` | após K-05a, em paralelo com K-05b |

### K-06 — Ownership, lifetime, handles generacionais e Drop

- **Owner:** `runtime` + `midend` + `backend`.
- **Arquivos:** criar `runtime/src/handles/mod.rs` e testes; modificar
  `runtime/src/lib.rs`, `runtime/src/memory/mod.rs`, `runtime/src/ffi.rs`,
  `runtime/src/stdlib/mod.rs` até a divisão por domínio de K-10,
  `runtime/src/reactor/mod.rs`,
  `midend/src/lowering.rs`, `midend/src/ir.rs`, `backend/src/codegen.rs`,
  `backend/src/aot.rs`, `tests/validation/stability_handle_lifecycle.spectra`
  e relatório OOP/Drop.
- **Escopo:** definir `HandleId` com kind, slot e generation; `0` permanece
  inválido; invalid/use-after-free/type mismatch retorna `HOST_STATUS_*` e
  `Error`, nunca payload ambíguo. Centralizar criação, borrow temporário,
  clone/refcount, release/drop, task transfer e process shutdown. Migrar
  primeiro list/map/range/time, depois tensor/ML, async e API/DB. O lowering
  emite drop glue em retorno normal, early return, branch, loop e falha; valores
  que cruzam task precisam de contrato `Send/Sync`.
- **Displaced path:** `HashMap<usize, ...>`/`Vec<Option<...>>` particulares,
  `free_all` como ownership normal e handles sem generation deixam de ser
  autoridade; `free_all` fica apenas como limpeza de processo/teste.
- **Output:** especificação de ownership, matriz de lifecycle e implementação
  central; relatório de handles vivos/invalidos sem vazamento.
- **Verificação:** testes Rust de generation/use-after-free/type mismatch,
  fixtures `.spectra` de scope/drop/branches/tasks, stress repetido e
  `cargo test --workspace`; relatório OOP deixa Drop verde ou mantém a feature
  não certificada.
- **Aceite:** handles não podem apontar para objeto reciclado, recursos são
  liberados uma vez em ordem definida, e async/API não usam valores destruídos.
- **Paralelismo:** migrações de domínios podem ser paralelas após a tabela
  central, sem editar o mesmo módulo simultaneamente.

### K-07 — Fechamento verificável de exact-width

- **Owner:** `runtime` + `semantic` + `midend` + `backend` + `tooling`.
- **Arquivos:** revisar `compiler/src/ast/mod.rs`,
  `compiler/src/semantic/mod.rs`, `midend/src/ir.rs`,
  `midend/src/lowering.rs`, `backend/src/codegen.rs`, `backend/src/aot.rs`,
  `runtime/src/numeric.rs`, `runtime/src/stdlib/mod.rs`,
  `scripts/validate_r2901_exact_width.py`, `tests/validation/189_exact_width_numeric_semantics.spectra`,
  `tests/validation/274_exact_width_aggregate_boundary.spectra`,
  `tests/fixtures/stability/exact_width_ffi.c` e docs.
- **Escopo:** auditar antes de mudar: i8/i16/i32/i64/isize/u8/u16/u32/usize,
  f32/f64, casts checked/wrapping, overflow, literals, operators, arrays,
  globals, aggregates, host calls, JIT, AOT e C ABI. Corrigir apenas lacunas
  reproduzidas; f16/bf16 ficam como metadata/tensor se não houver scalar
  representation. Remover texto alpha/deferred se a evidência atual realmente
  cobrir o critério; caso contrário manter explicitamente incompleto.
- **Displaced path:** nenhum alias é promovido só por aparecer no parser ou
  passar por slot i64.
- **Output:** matriz por tipo/operação/target com relatório de narrowing,
  overflow, ABI e C interoperability.
- **Verificação:** validator R-2901, negativos para overflow/narrowing,
  `cargo test` backend/runtime, JIT/AOT, fixture C e `git diff --check`.
- **Aceite:** os critérios passam ou a feature permanece fora do stable; não
  existe declaração simultânea “complete” na roadmap e “in progress” na docs.
- **Paralelismo:** pode rodar com K-06 depois de K-03.

### K-08 — Arrays, indexação e iteradores reais

- **Owner:** `frontend` + `semantic` + `midend` + `runtime`.
- **Arquivos:** modificar `compiler/src/ast/`, `compiler/src/parser/`,
  `compiler/src/semantic/`, `midend/src/ir.rs`, `midend/src/lowering.rs`,
  `runtime/src/stdlib/mod.rs`, catálogo STD,
  `tests/validation/stability_arrays_iterators.spectra`,
  `tests/errors/stability_array_bounds.spectra`, docs e fixtures; reutilizar o
  contrato de `Range` de R-2902.
- **Escopo:** definir `Array<T,N>`/array dinâmico, representação contígua,
  tamanho, mutação, bounds e erro; arrays vazios exigem tipo/contexto, não
  inferem `int` arbitrariamente. Definir `Iterator<T>` com `next() -> Option<T>`
  e adaptar range, array, List e Map. Fazer `for` consumir o protocolo de
  iterator; manter `std.range` apenas como implementação/adaptador do caminho
  novo, removendo o branch especial quando a migração terminar.
- **Displaced path:** `range_map` e indexação parcial não podem continuar sendo
  a segunda semântica de iteração.
- **Output:** fixtures de arrays vazios/tipados, nested arrays, bounds,
  mutação, range/collection iteration, early break e drop de iterador.
- **Verificação:** `check/run`, dump IR, JIT/AOT, negativos de tipo/bounds,
  `cargo test -p spectra-compiler -p spectra-midend -p spectra-runtime`,
  R-2902 sem regressão.
- **Aceite:** o mesmo protocolo funciona para todas as fontes de iteração,
  valores/erros são tipados e não há fallback arbitrário para array vazio.
- **Paralelismo:** depende de K-05 e K-06; não paralelizar com a migração da
  mesma estrutura de handles.

### K-09 — Async completo e integrado à rede/API

- **Owner:** `runtime` + `web` + `db` + `semantic` + `tooling`.
- **Arquivos:** revisar `runtime/src/reactor/`, `runtime/src/stdlib/mod.rs` async,
  `compiler/src/semantic/mod.rs`, `midend/src/lowering.rs`,
  `packages/spectra-api/src/`, `packages/spectra-db/src/`,
  `backend/src/`, fixtures async/API/DB, `scripts/validate_r2107_async_stdlib.py`,
  `scripts/validate_r2505_postgres.py`, `scripts/validate_r2507_redis.py`,
  `scripts/validate_r2701_tracing.py`, `scripts/run_phase27_tracing.ps1`,
  `tests/validation/193_opentelemetry_tracing.spectra`,
  `tests/projects/valid/stability_async_api/` e docs de concorrência.
- **Escopo:** congelar o contrato de Task/Future/Stream, await, cancellation,
  timeout, parent/child scope, backpressure, Send/Sync e erro. Garantir que
  FS/TCP/UDP/channel e HTTP server/client sejam nonblocking e polláveis nos
  adapters epoll/IOCP/kqueue, sem host blocking silencioso (`E2113`). Fazer
  handlers, middleware, TLS, SQLite, PostgreSQL e Redis propagar task,
  cancellation, deadline, Result e tracing parent/child. Fechar object safety e
  auto-traits onde o contrato promete estabilidade; manter feature fora do
  stable se algum item não tiver prova multiplataforma.
- **Displaced path:** async “baseline” que apenas simula prontidão ou cai para
  worker bloqueante não é caminho de produção; se usado por compatibilidade,
  deve ser nomeado e classificado.
- **Output:** contrato async v1, matriz plataforma/API/driver, benchmarks e
  fixtures de sucesso, cancelamento, timeout, worker failure, backpressure,
  Send/Sync e tracing.
- **Verificação:** gates R-2107, async benchmark, API conformance, SQLite,
  PostgreSQL 16, Redis 7, OTLP parentage, TLS externo e JIT/AOT quando
  aplicável; comandos mínimos são
  `python scripts/validate_r2107_async_stdlib.py`,
  `python scripts/validate_r2505_postgres.py --database-url $env:SPECTRA_POSTGRES_URL --require-database --report target/stability/postgres.json`,
  `python scripts/validate_r2507_redis.py --redis-url $env:SPECTRA_REDIS_URL --require-redis --report target/stability/redis.json`,
  `$env:SPECTRA_RUN_EXTERNAL_TLS='1'; python scripts/validate_r2207_tls_rustls.py`
  e `python scripts/validate_r2701_tracing.py --binary target/debug/spectralang.exe --fixture tests/validation/193_opentelemetry_tracing.spectra --report target/stability/tracing.json`;
  nenhum teste obrigatório ignorado.
- **Aceite:** um request de API pode aguardar I/O real, cancelar e propagar
  erro sem bloquear executor ou perder span; a matriz mostra epoll/IOCP/kqueue.
- **Paralelismo:** reactor, API e DB podem ter donos separados após K-05/K-06,
  mas exigem fixture de integração único antes do gate.

### K-10 — Decomposição dos monólitos sem criar novos contratos

- **Owner:** cada owner de crate; `tooling` coordena a ordem.
- **Arquivos:** dividir `runtime/src/stdlib/mod.rs`,
  `compiler/src/semantic/mod.rs`, `compiler/src/semantic/builtin_modules.rs`,
  `midend/src/lowering.rs`, `packages/spectra-api/src/lib.rs`,
  `packages/spectra-api/src/http.rs`, `tools/spectra-cli/src/main.rs` e
  testes inline em módulos/integration tests por domínio.
- **Escopo:** primeiro preservar APIs públicas e mover código sem alteração
  semântica; depois retirar reexports temporários. STD deve separar core
  (`io`, `math`, `string`, `convert`, `char`, `random`, `time`, `fs`, `env`),
  collections/handles, tensor, ML, async, serving e tests. Semantic deve
  separar declarations/types/builtin projections/async/API. Midend deve
  separar context, declarations/globals, expressions, control-flow/patterns,
  calls/host, aggregates, closures, arrays/iterators e verifier adapters.
- **Displaced path:** a fachada `mod.rs` fica apenas como composição/exports;
  não pode continuar contendo a implementação completa nem nova tabela de
  symbols.
- **Output:** módulos pequenos, dependências explícitas e mapa de ownership.
- **Verificação:** snapshots IR/diagnostics, `cargo test --workspace`,
  `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`;
  regra de tamanho reporta qualquer arquivo de produção acima de 1.000 linhas
  com exceção justificada e aprovada.
- **Aceite:** cada módulo tem uma razão de mudança única, nenhuma funcionalidade
  muda por causa da divisão, e o catálogo continua sendo a única autoridade.
- **Paralelismo:** um crate por vez ou pastas disjuntas; integração obrigatória
  após cada lote.

### K-11 — Gates obrigatórios de qualidade e serviços externos

- **Owner:** `tooling` + `runtime` + `db` + `web` + `ecosystem`.
- **Arquivos:** criar ou atualizar `scripts/validate_stability_release.py`;
  atualizar `run_tests.ps1`, `scripts/validate_r2505_postgres.py`,
  `scripts/validate_r2507_redis.py`, `scripts/validate_r2207_tls_rustls.py`,
  `scripts/validate_r2701_tracing.py`, `scripts/validate_language_guide.py`,
  `scripts/validate_feature_maturity.py`, `scripts/stdlib_contract.toml`,
  `scripts/run_phase27_tracing.ps1`,
  `.github/workflows/ci.yml`, `.github/workflows/r2505-postgres.yml`,
  `.github/workflows/r2507-redis.yml`, `.github/workflows/release.yml` e
  `docs/testing/`.
- **Escopo:** separar modo local permissivo de modo release requerido. Em modo
  required, PostgreSQL deve ser realmente versão 16, Redis versão 7, OTLP deve
  ser recebido por collector real e TLS deve validar cadeia/handshake contra
  endpoint externo configurado. Relatórios devem omitir credenciais, registrar
  versões, capability checks, exit codes e parentage. Os validadores de
  PostgreSQL e Redis devem ter flags explícitas `--require-database` e
  `--require-redis`; `run_tests.ps1` deve
  integrar um agregador único, sem duplicar validadores.
- **Gates obrigatórios:** `cargo fmt --check`, Clippy `-D warnings`, cargo
  workspace, validator de maturidade, documentação/links/exemplos, R-3007,
  bug hunts, API conformance, PostgreSQL, Redis, OTLP e TLS. `skipped_environment`
  só é aceitável em desenvolvimento local e nunca no release report.
- **Output:** `target/stability/release-report.json` e `.md` com schema,
  target, binary hash, git revision, environment capabilities e decisão final.
- **Verificação:** execução local de todos os gates disponíveis mais execução
  CI required com services/endpoint; teste unitário do agregador para garantir
  que skip obrigatório falha.
- **Aceite:** o relatório responde “por que estável?” com evidência por
  capability; nenhuma dependência crítica fica implícita ou pulada.
- **Paralelismo:** só após K-09; validators individuais podem ser ajustados em
  paralelo com escopos disjuntos.

### K-12 — Certificação final, sincronização e remoção de compatibilidade

- **Owner:** `ecosystem` + `tooling`, com sign-off de todos os owners.
- **Arquivos:** `README.md`, `docs/language-feature-maturity.md`,
  `docs/reference/05-stdlib.md`, `docs/runtime/standard-library.md`,
  `docs/frontend/`, `docs/semantic/`, `docs/midend/`, `docs/diagnostics/`,
  `ARCHITECTURE.md`, `SYNTAX_SUMMARY.md`, `docs/roadmap-backlog.md`,
  `roadmap/roadmap.toml`, `docs/production-ai-implementation-plan.md` e
  changelog/release metadata quando o código estiver comprovado.
- **Escopo:** migrar todos exemplos/fixtures para APIs tipadas, remover
  adapters `std.compat` cujo prazo venceu, atualizar links e marcar apenas
  capacidades realmente certificadas. Reconciliar status contraditórios da
  roadmap/backlog/strategic plan; não fechar item por intenção ou por teste
  focused isolado. Produzir guia de migração para handles/sentinelas.
- **Output:** release candidate com contrato estável, relatório requerido,
  matriz de migração e documentação sem claims stale.
- **Verificação:** `python scripts/validate_stability_release.py --required`;
  `.\run_tests.ps1`; `cargo test --workspace --all-targets --no-fail-fast`;
  `cargo fmt --all -- --check`; `cargo clippy --workspace --all-targets -- -D warnings`;
  `git diff --check`; revisão manual do relatório do ponto de vista de um
  usuário novo.
- **Aceite:** só então o subconjunto estável pode ser chamado de estável. Se
  qualquer item falhar, manter status `in_progress` e registrar exatamente a
  evidência ausente.
- **Paralelismo:** nenhum; é a integração e cutover final.

## Matriz de ownership e evidência

| Resultado | Dono primário | Evidência mínima | Dependências |
| --- | --- | --- | --- |
| estabilidade/classificação | tooling + frontend | manifest, docs, CLI, positivos/negativos | K-01 |
| `class` removida da promessa | frontend | parser diagnostic + docs sem claim | K-01 |
| `static` real | semantic + midend/backend | JIT/AOT, cross-module, mutation | K-03 |
| pipeline sem placeholder | semantic + midend | negativos fail-closed + IR verifier | K-02 |
| catálogo único | tooling/runtime | generated projections + no duplicate paths | K-02 |
| typed STD/errors | semantic/runtime | generic fixtures + no unknown/sentinel | K-05 |
| handle ownership/Drop | runtime/midend | lifecycle/use-after-free/drop matrix | K-06 |
| exact-width | runtime/backend | overflow/narrowing/C ABI/JIT/AOT | K-07 |
| arrays/iterators | frontend/midend/runtime | Array/List/Range Iterator<T> matrix | K-08 |
| async/API | runtime/web/db | real I/O, cancel, tracing, external services | K-09 |
| maintainability | all crate owners | module size, fmt, clippy, tests | K-10 |
| release proof | tooling/ecosystem | required aggregate report | K-11/K-12 |

## Regras de compatibilidade e migração

1. Não alterar assinaturas antigas silenciosamente: introduzir o adapter
   explícito, migrar consumidores, emitir deprecation e remover no gate final.
2. Não contar `std.compat`, baseline, simulation ou `skipped_environment` como
   stable production.
3. Toda mudança de tipo deve ter positivo `check/run`, negativo `check --json`
   e, quando houver lowering/backend, JIT e AOT.
4. Toda mudança de host call deve atualizar o catálogo, não quatro listas.
5. Toda mudança de maturidade deve atualizar manifest, projeções docs,
   exemplos, CLI e validator na mesma alteração.
6. Nenhuma tarefa pode reformatar ou limpar arquivos fora do seu escopo; o
   gate de fmt será corrigido em lote próprio, com diff auditável.

## Critérios de conclusão do pacote

O pacote só está concluído quando:

- `class` não é anunciada como utilizável;
- `static` executa com o mesmo resultado em JIT e AOT;
- semantic success não contém `Type::Unknown` curinga;
- lowering/backend não têm fallback de símbolo desconhecido para Value/zero;
- STD/API/lowering/runtime usam o catálogo único;
- collections/Option/Result/errors são tipados e sem sentinelas ambíguas;
- handles têm geração, ownership, release e Drop comprovados;
- exact-width, arrays e iteradores têm matriz de operadores/targets;
- async integra I/O/API/DB/TLS/OTLP com cancellation e Send/Sync;
- monólitos foram decompostos sem nova duplicação;
- fmt, Clippy, docs e workspace estão verdes;
- PostgreSQL 16, Redis 7, OTLP e TLS externo estão em modo required e
  certificados;
- o relatório final e a documentação não usam “complete”, “production” ou
  “stable” para capacidades que ainda são baseline/simulation.

## Riscos e respostas

| Risco | Resposta planejada |
| --- | --- |
| catálogo compartilhado criar ciclo de crates | crate de contrato sem dependência de runtime/compiler; bindings ficam em adapters |
| migração typed STD quebrar centenas de fixtures | compatibilidade explícita, migração por namespace e snapshot do nameset |
| remover `Unknown` expor falhas antigas | negativos primeiro, diagnóstico por fase e nenhum fallback no backend |
| `static` gerar corrida global | initializer constante + regra Send/Sync; não prometer sincronização implícita |
| handles centrais aumentarem custo | generation/slot benchmarkado e fast paths mantidos apenas após prova |
| refatoração monolítica esconder regressão | split behavior-preserving, snapshots e gate por crate |
| PostgreSQL/Redis/TLS indisponíveis localmente | CI required; local skip visível e nunca promovido |
| roadmap/backlog divergentes | sincronização somente em K-12 com evidência atual, não copiar texto histórico |

## Itens explicitamente fora deste pacote

- implementação de classes tradicionais;
- novo transporte distribuído de ML, serving de rede de modelos ou embeddings
  model-backed, salvo a integração async/API necessária ao core;
- novos backends CUDA/ROCm/Metal/Vulkan;
- hosted package registry;
- reescrita cosmética sem vínculo com contrato, evidência ou remoção de
  caminho deslocado.

Esses itens continuam visíveis como trabalho de produto separado e não podem
ser implicitamente promovidos por este pacote.
