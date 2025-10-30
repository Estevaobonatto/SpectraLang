Desenvolva um plano detalhado para a criação da linguagem de programação SpectraLang do zero, incluindo os seguintes aspectos:

1. Definição de características técnicas:
   - Suporte a paradigmas: Orientação a Objetos (herança, polimorfismo, encapsulamento) + programação procedural + funcional
   - Sistema de tipos: Implementar tipagem forte com opção de modo fraco via diretivas especiais
   - Modelo de compilação: Compilador JIT (Just-In-Time) para rápida execução
   - Gerenciamento de memória: Coletor de lixo automático com opção de controle manual|
  - Suporte a todas as estruturas de dados
- deve ter uma documentação sólida com tudo que se pode usar na linguagem

2. Especificações da sintaxe:
   - Design limpo e intuitivo, balanceando simplicidade e expressividade
   - Palavras-chave em inglês reduzidas e consistentes
   - Suporte a metaprogramação controlada
   - Sistema de módulos integrado

3. Arquitetura do compilador:
   - Frontend: Analisador léxico, sintático e semântico
   - Middle-end: Otimizações independentes de plataforma
   - Backend: Geração de código para múltiplos alvos (x86, ARM, WASM)
   - Runtime: Biblioteca padrão mínima mas completa

4. Plano de desenvolvimento:
   - Fase 1: Protótipo do compilador básico (3 meses)
   - Fase 2: Implementação de recursos avançados (4 meses)
   - Fase 3: Otimização e polimento (2 meses)
   - Fase 4: Documentação e exemplos (1 mês)

5. Ferramentas auxiliares:
   - IDE com realce de sintaxe e autocompletar
   - Debugger integrado
   - Gerenciador de pacotes
   - Ferramenta de formatação de código

6. Critérios de qualidade:
   - Tempo de compilação < 500ms para projetos médios
   - Compatibilidade com 95% dos padrões POSIX
   - Curva de aprendizagem < 2 semanas para programadores experientes
   - Performance dentro de 15% das linguagens estabelecidas

7. Estratégia de testes:
   - Suíte de testes unitários para cada componente
   - Testes de integração entre módulos
   - Benchmarking contínuo
   - Validação cross-platform

8. Documentação:
   - Especificação formal da linguagem
   - Tutoriais passo-a-passo
   - Referência da API
   - Guias de melhores práticas

9. Ecossistema:
   - Comunidade aberta para contribuições
   - Repositório central de pacotes
   - Sistema de versão semântica
   - Canal de suporte técnico