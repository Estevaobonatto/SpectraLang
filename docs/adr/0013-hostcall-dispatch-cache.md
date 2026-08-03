# ADR-0013: Dispatch genérico cacheado de HostCall

- **Status:** Accepted
- **Data:** 2026-08-02
- **Escopo:** caminho genérico interno do toolchain entre lowering Cranelift e o registry de host calls

## Contexto

O seam HostCall/ABI da Fase 1 removeu a duplicação entre JIT e AOT, mas o
caminho genérico ainda fazia um lookup no `HashMap`, adquiria o mutex do
registry e criava a resolução de cada chamada. O batch legado repetia esse
trabalho para cada descriptor e mantinha uma fronteira de `catch_unwind` por
host call.

## Decisão

O runtime continua owner do contrato ABI e adiciona, de forma aditiva, dois
imports internos ao catálogo de `runtime/src/abi.rs`:

- `spectra_rt_host_invoke_cached` para uma chamada individual;
- `spectra_rt_host_invoke_cached_batch` para um batch cacheado.

Cada módulo JIT/AOT emite um `SpectraHostCallCache` por nome de host call. O
slot contém uma geração atômica e um ponteiro atômico de função. O primeiro uso
ou uma geração inválida consulta o registry sob o mutex atual; o ponteiro é
publicado antes da geração. Hits validam a geração com loads atômicos e não
adquirem o mutex. Entradas não encontradas também são cacheadas, mas uma
mutação do registry invalida todos os slots por meio da geração global.

Registro, substituição, remoção e `clear` continuam preservando a visibilidade
dinâmica imediata para código já compilado. Não há remoção de slots enquanto o
módulo existir, evitando ponteiros pendentes em chamadas concorrentes.

O lowering genérico individual usa o import cacheado. O batch usa descriptors
de sete palavras contendo cache, nome e buffers; mantém os limites de oito
chamadas e 4096 bytes, a ordem de origem, as dependências e a exclusão dos
fast paths. O batch cacheado possui uma única fronteira de `catch_unwind` para
a sequência e para no primeiro status diferente de sucesso. A chamada
individual mantém sua própria fronteira.

Os símbolos `spectra_rt_host_invoke` e `spectra_rt_host_invoke_batch`, os tipos
públicos existentes, o `SpectraHostCallContext`, os códigos de status, os
callbacks e o registro dinâmico permanecem sem alteração de contrato. O
algoritmo legado não é substituído; os imports novos são uma rota interna
aditiva usada apenas pelo backend atualizado.

## Ownership

- **Runtime:** layout de `SpectraHostCallCache`, geração, publicação atômica,
  resolução sob lock e semântica de status/panic.
- **Backend:** armazenamento dos slots JIT, dados graváveis AOT, descriptors e
  política de batching.
- **Catálogo ABI:** única fonte para símbolos e assinaturas JIT/AOT.

## Alternativas rejeitadas

- Snapshot do registry no momento da compilação: quebraria a visibilidade
  imediata de `register`, substituição, remoção e `clear`.
- Remover ou alterar os imports legados: aumentaria o risco de ABI e impediria
  rollback isolado do lowering.
- Arena permanente para argumentos e resultados: mistura esta fase com a
  futura remoção de alocações manuais.
- Novos fast paths: não são necessários para validar o ganho do dispatch
  genérico cacheado.

## Consequências e evolução

O caminho quente genérico deixa de repetir lookup e lock em cache hits, e batches
cacheados deixam de repetir a resolução por descriptor. A memória de cada
nome é pequena e estável durante a vida do módulo. A ausência de profiling
Linux oficial em R-3102 impede atribuir causalidade a qualquer ganho desta
fase; a evidência deve comparar controle e candidato sem alterar a baseline.

A Fase 3 poderá tratar reutilização das arenas e outras alocações, mas deve
preservar este catálogo, a geração dinâmica e os símbolos legados.
