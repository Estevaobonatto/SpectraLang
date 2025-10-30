# ADR 0003: Roadmap do analisador semântico e sistema de tipos

- Status: Aceito
- Data: 2025-10-30
- Autores: SpectraLang Core Team

## Contexto

A implementação atual do analisador semântico cobre escopos aninhados, redefinições, uso de símbolos, checagem básica de `return` e validação de operadores sobre tipos primitivos. Os próximos ciclos precisam evoluir essas capacidades para suportar resolução entre módulos, propagação de tipos em chamadas de função e preparar o compilador para análises mais ricas (tipagem estática completa, geração de SIR, otimizações).

Existem decisões arquiteturais que orientam essa evolução:

1. **Tabela de símbolos hierárquica** — deve sustentar exportação/importação entre módulos, diferenciar símbolos por categoria (funções, tipos, variáveis) e carregar metadados suficientes para futuras fases (mutabilidade, visibilidade, atributos).
2. **Resolução inter-módulo** — a linguagem prevê organização por módulos. Mesmo antes de implementar o gerenciador de pacotes, o compilador precisa aceitar múltiplos arquivos/fonte na mesma compilação e detectar usos antes de definição em fronteiras de módulo.
3. **Sistema de tipos incremental** — a tipagem forte com inferência local exige inferência para literais, operadores, chamadas, parâmetros e retorno. O design precisa ser extensível para composições futuras (structs, generics, traits).

## Decisão

- Continuaremos usando a estrutura de escopos em pilha, mas cada `ScopeFrame` passa a carregar metadados (categoria, tipo, flags) adequados para resolver nomes dentro e fora do arquivo.
- Adotaremos um tipo interno `SemanticType` (atual `Type`) com variantes para primitivos, funções e tipos desconhecidos. Essa enum será usada em todas as validações do front-end e propagada para o middle-end (SIR).
- Implementaremos, em fases, as seguintes capacidades:
  1. **Resolução inter-módulo**: coletar cabeçalhos de módulos, registrar símbolos exportados, permitir referência cruzada e detectar sombras/ausência. Isso inclui planejar o formato de AST/IR para `import` e como a CLI receberá múltiplos arquivos.
  2. **Chamadas e assinaturas**: estender a AST para chamadas (`CallExpr`), registrar tipo de retorno de função, validar argumentos vs. parâmetros e marcar uso de funções para relatórios (unused/privacidade).
  3. **Propagação de tipos composta**: introduzir variantes para arrays, ponteiros/refs, objetos estruturados e permitir inferência contextual em expressões mais complexas (condicionais, match, blocos).
  4. **Integração com SIR**: ao finalizar o front-end, os tipos validados alimentarão a geração de SIR em SSA, preservando informações necessárias para o JIT.
- Documentaremos cada incremento significativo com testes unitários/integração e atualizações nos planos (Plano detalhado, README, CLI help), garantindo transparência da evolução.

## Consequências

- O roadmap fica explicitado para orientar contribuições e evitar decisões ad-hoc sobre semântica/tipos.
- A equipe pode priorizar entregas iterativas (ex.: primeiro resolver múltiplos arquivos, depois chamadas) sem perder a visão global.
- Ao alinhar as decisões com o futuro SIR/JIT, reduzimos retrabalho nas fases subsequentes.
- O compilador ganhará diagnósticos mais precisos e manterá compatibilidade com o plano geral descrito na documentação principal.

## Próximos passos

1. Concluir o suporte a múltiplos arquivos na CLI/analisador com carregamento de símbolos exportados, permitindo resolução inter-módulo efetiva.
2. Definir regras de visibilidade (`export`/`pub`) e materializar símbolos importados nos escopos consumidores para habilitar referências cruzadas.
3. Expandir `Type` para abranger compostos simples (ex.: arrays) e conectar esses tipos às futuras construções do runtime/SIR.
4. Propagar tipos em fluxos de controle mais complexos (condicionais, `match`, blocos) preparando terreno para geração de SIR.
