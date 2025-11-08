# Spectra Standard Library Completion Checklist

> Checklist focado na implementação completa da biblioteca padrão host-driven descrita em `docs/runtime/standard-library.md`.

## Fundação e Documentação

- [x] Registrar stdlib mínima via `register_standard_library()` e documentar funções atuais
- [ ] Especificar formato de versionamento e política de breaking changes da stdlib
- [ ] Criar guia de integração para bibliotecas de terceiros consumirem/estenderem a stdlib
- [ ] Automatizar geração de documentação a partir das assinaturas das funções host

## Matemática e Numérico

- [x] Disponibilizar `abs`, `min`, `max` para inteiros
- [x] Implementar operações básicas adicionais (`add`, `sub`, `mul`, `div`, `mod`, `pow`)
- [ ] Suportar floats com conversões e funções trigonométricas, validando NaN/Inf
- [x] Fornecer utilitários de estatística simples (média, `clamp`, geração determinística de números aleatórios)
  - [x] `std.math.clamp`
  - [x] `std.math.mean`
  - [x] RNG determinístico (`rng_seed`, `rng_next`, `rng_next_range`, `rng_free`, `rng_free_all`)
  - [x] utilitários adicionais (`median`, `variance`, `std_dev`)

## I/O e Sistema

- [x] Expor `print` e `flush` síncronos
- [ ] Adicionar escrita/leitura direcionada (stdout/stderr, arquivos, buffers)
- [ ] Planejar API de logging estruturado com níveis e destinos configuráveis
- [ ] Definir API de tempo (timestamp, `sleep`) com garantias multiplataforma

## Strings e Texto

- [ ] Introduzir representações host para strings UTF-8 (alocação, concatenação, comprimento, `substring`)
- [ ] Implementar conversões número↔string e parsing seguro
- [ ] Adicionar utilitários de formatação (`format`, interpolação simples)

## Coleções e Estruturas de Dados

- [x] Oferecer listas de inteiros com `new`/`push`/`len`/`clear`/`free`
- [ ] Expandir listas para suportar tipos genéricos via ponteiros/handles
- [ ] Adicionar mapas/dicionários e conjuntos baseados em hash com ciclo de vida controlado
- [ ] Criar iteradores/visões host para coleções (comprimento, `foreach`, slice)
- [ ] Suportar operações de ordenação, busca e filtragem host-driven

## Memória e Segurança

- [ ] Expor APIs explícitas de liberação (escopos, RAII) e diagnóstico de vazamentos
- [ ] Integrar verificações de limites e retornos de erro padronizados (`HOST_STATUS_*`)
- [ ] Documentar e testar interações com `ManualBox` e `HybridMemory`

## Erros e Resultados

- [ ] Padronizar códigos de erro específicos por namespace (math/io/collections) _(math usa `HOST_STATUS_ARITHMETIC_ERROR`)_
- [ ] Adicionar APIs que retornem `Result` estruturado ou enum de erro para Spectra
- [ ] Expor utilitários para gerar mensagens de erro amigáveis no nível Spectra

## Internacionalização e Localização

- [ ] Preparar hooks para formatação dependente de locale (números, datas)
- [ ] Definir política de codificação e fallback para ambientes sem suporte

## Testes e Qualidade

- [x] Cobertura básica via testes unitários host (math/io/listas)
- [ ] Criar suíte de integração Spectra exercendo cada função da stdlib _(exemplo `examples/std_math_operations.spectra` criado como ponto de partida)_
- [ ] Adicionar testes de carga/fuzzing para coleções e I/O
- [ ] Validar comportamento cruzado em Windows/macOS/Linux com scripts dedicados

## Ferramentas e Distribuição

- [ ] Empacotar stdlib como "bundle" habilitável via CLI (`--with-stdlib`, perfis)
- [ ] Gerar metadados para tooling (language server, auto-complete, hints de documentação)
- [ ] Planejar módulos opcionais futuros (crypto, net, async) com checklists próprios
