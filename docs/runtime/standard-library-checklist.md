# Spectra Standard Library Completion Checklist

> Checklist focado na implementação completa da biblioteca padrão host-driven descrita em `docs/runtime/standard-library.md`.

## Fundação e Documentação

- [x] Registrar stdlib mínima via `register_standard_library()` e documentar funções atuais
- [x] Especificar formato de versionamento e política de breaking changes da stdlib
- [ ] Criar guia de integração para bibliotecas de terceiros consumirem/estenderem a stdlib
- [ ] Automatizar geração de documentação a partir das assinaturas das funções host

## Matemática e Numérico

- [x] Disponibilizar `abs`, `min`, `max` para inteiros
- [x] Implementar operações básicas adicionais (`add`, `sub`, `mul`, `div`, `mod`, `pow`)
- [x] Suportar floats com conversões e funções trigonométricas, validando NaN/Inf
  - [x] Conversões `int_to_float` / `float_to_int` com saturação
  - [x] Operações básicas (`float_add`, `float_sub`, `float_mul`, `float_div`, `float_pow`)
  - [x] Funções auxiliares (`float_abs`, `float_sqrt`, `float_exp`, `float_ln`)
  - [x] Trigonometria (`trig_sin`, `trig_cos`, `trig_tan`, `trig_atan2`)
- [x] Fornecer utilitários de estatística simples (média, `clamp`, geração determinística de números aleatórios)
  - [x] `std.math.clamp`
  - [x] `std.math.mean`
  - [x] RNG determinístico (`rng_seed`, `rng_next`, `rng_next_range`, `rng_free`, `rng_free_all`)
  - [x] utilitários adicionais (`median`, `variance`, `std_dev`, `mode`)

## I/O e Sistema

- [x] Expor `print` e `flush` síncronos
- [x] Adicionar escrita/leitura direcionada (stdout/stderr, arquivos, buffers)
  - [x] Escrita em stderr (`std.io.print_err`)
  - [x] Escrita para buffer (`std.io.print_to_buffer`)
  - [x] Escrita em arquivo (`std.io.write_file`)
  - [x] Leitura de arquivo (`std.io.read_file`)
- [x] Planejar API de logging estruturado com níveis e destinos configuráveis
  - [x] Definir níveis padrão (TRACE/DEBUG/INFO/WARN/ERROR)
  - [x] Implementar sinks configuráveis iniciais (stdout, stderr, arquivo, buffer)
  - [x] Registrar host calls (`set_level`, `add_sink`, `clear_sinks`, `record`) e documentar uso
  - [x] Expandir suporte para campos estruturados/destinos personalizados (JSON/`key=value`, sink de lista)
- [x] Definir API de tempo (timestamp, `sleep`) com garantias multiplataforma
  - [x] Timestamp UTC (`std.time.now`)
  - [x] Relógio monotônico (`std.time.now_monotonic`)
  - [x] Suspensão controlada (`std.time.sleep`)

## Strings e Texto

- [x] Introduzir representações host para strings UTF-8
  - [x] Alocação/handles dedicados e conversão segura com listas
  - [x] Consulta de comprimento (contagem de escalares Unicode)
  - [x] Concatenação básica entre handles
  - [x] `substring`
- [x] Implementar conversões número↔string e parsing seguro
- [x] Adicionar utilitários de formatação (`format`, interpolação simples)

## Coleções e Estruturas de Dados

- [x] Oferecer listas de inteiros com `new`/`push`/`len`/`clear`/`free`
- [x] Expandir listas para suportar tipos genéricos via ponteiros/handles
- [x] Adicionar mapas/dicionários e conjuntos baseados em hash com ciclo de vida controlado
- [x] Criar iteradores/visões host para coleções (comprimento, `foreach`, slice)
- [x] Suportar operações de ordenação, busca e filtragem host-driven

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
