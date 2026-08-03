# ADR-0012: Catálogo compartilhado para HostCall/ABI lowering

- **Status:** Accepted
- **Data:** 2026-08-02
- **Escopo:** seam interno do toolchain entre runtime, lowering Cranelift JIT e AOT

## Contexto

Os backends mantinham listas paralelas de símbolos, assinaturas e `FuncId`s de
runtime. A classificação de fast host calls também estava duplicada em uma
lista manual e em comparações de strings dentro do lowering. Esse desenho
aumentava o risco de JIT e AOT divergirem e tornava mudanças de ABI difíceis de
revisar, mesmo quando o dispatch do runtime permanecia correto.

## Decisão

O runtime é o owner do contrato ABI estável usado internamente pelo toolchain.
`runtime/src/abi.rs` mantém o catálogo `RuntimeImport` para imports nativos e
`FastHostCall` para nomes que possuem lowering explícito. Cada entrada informa:

- nome do símbolo nativo;
- assinatura escalar usada para a declaração Cranelift;
- aridade;
- endereço nativo para registro do JIT;
- relação com o nome Spectra e o fast path, quando existir.

O módulo é `doc(hidden)` e não adiciona sintaxe, semântica ou API pública da
linguagem.

`backend/src/hostcall_abi.rs` é o adapter dos backends. `RuntimeBindings`
declara a tabela completa uma vez para JIT ou AOT e a indexa por enum.
`HostCallLoweringContext` reúne os bindings e o estado transitório do lowering,
incluindo nomes/literais internados e estatísticas de batching. O JIT usa o
mesmo catálogo para registrar endereços; o AOT usa-o para declarar símbolos e
assinaturas relocáveis.

O lowering aplica estas regras compatíveis:

1. nome conhecido e aridade correta usa o fast ABI existente;
2. nome conhecido com aridade incorreta cai no caminho genérico;
3. nome desconhecido cai no caminho genérico;
4. fast ABI nunca participa do batch genérico;
5. uma chamada que depende de resultado anterior não é agrupada;
6. os limites atuais de oito chamadas e 4096 bytes não mudam.

O catálogo não altera o contrato de `SpectraHostCallContext`, `HostFunction`,
`HostCallBatchStats`, registry dinâmico, códigos de status, callbacks, ordem de
resultados ou nomes `spectra.std.*`, `spectra.api.*` e `spectra_rt_*`.

## Ownership

- **Runtime:** contrato ABI, símbolos nativos e metadados estáveis; mantém o
  algoritmo de lookup, lock, criação de contexto e `catch_unwind`.
- **Backend:** adaptação para Cranelift, classificação aplicada ao lowering e
  política de batching.
- **JIT/AOT:** consumidores da mesma tabela; não possuem listas paralelas.

## Alternativas rejeitadas

- Manter tabelas independentes em `codegen.rs` e `aot.rs`: preservaria a fonte
  da divergência que este ADR corrige.
- Alterar o dispatch em `runtime/src/ffi.rs` nesta fase: misturaria a
  normalização estrutural com uma otimização de runtime de maior risco.
- Expor o catálogo como API da linguagem: criaria um contrato público sem
  necessidade e limitaria a evolução interna do toolchain.

## Consequências e evolução

A fase reduz acoplamento e torna a equivalência JIT/AOT testável, sem declarar
ganho de runtime. A fase seguinte poderá usar o catálogo para estudar resolução
de função e dispatch batch mais baratos, mas qualquer mudança de lookup,
locking ou tratamento de panic deverá ser avaliada separadamente e preservar o
fallback genérico.
