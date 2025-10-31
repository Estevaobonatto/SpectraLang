Desenvolva um plano detalhado para a criação da linguagem de programação SpectraLang do zero, incluindo os seguintes aspectos:

# use rust ou c++/c para o desenvolvimento

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

# A sisntaxe deve ser simples de se escrever, focando na usabilidade e praticidade, mas sem comprometer as fucnionalidades da linguagem

deve ter a implementaçã ode pelo menos 80% das tags:

[tag] - [linguagens que voce deve se inspirar]

if/else/elif - Python, Ruby, Bash
if/else if/else - JavaScript, C, C++, Java
switch/case - C, C++, Java, JavaScript, Go
match/case - Python 3.10+, Rust
cond - Lisp, Clojure
unless - Ruby, Perl

Loops:

for - Praticamente todas (C, Python, Java, JavaScript, etc.)
while - Praticamente todas
do-while - C, C++, Java, JavaScript
foreach - PHP, C#, Perl
for-in - JavaScript, Python
for-of - JavaScript
loop - Rust
repeat-until - Lua, Pascal

Controle de Fluxo:

break - Maioria das linguagens
continue - Maioria das linguagens
return - Todas
goto - C, C++, Go (limitado)
yield - Python, JavaScript, C#

Estruturas de Dados Primitivas
Tipos Básicos:

int/integer - Todas
float/double - Todas
char - C, C++, Java
string - Maioria (nativa em Python, Java, C#; biblioteca em C)
bool/boolean - Maioria das modernas
byte - Java, C#, Go

Estruturas de Dados Compostas
Arrays/Listas:

array - C, C++, Java, JavaScript, PHP
list - Python, Haskell
vector - C++, Rust
slice - Go
ArrayList - Java

Listas Encadeadas:

LinkedList - Java, C#
Implementação manual em C, C++
list - C++ STL

Dicionários/Mapas:

dict - Python
hash/Hash - Ruby, Perl
map/Map - JavaScript, Java, Go, C++
HashMap/TreeMap - Java
object - JavaScript (usado como dict)
associative array - PHP

Conjuntos:

set/Set - Python, JavaScript, Java, C++
HashSet/TreeSet - Java
unordered_set - C++

Tuplas:

tuple - Python, Swift, Rust
std::tuple - C++
Múltiplos valores de retorno - Go

Structs/Records:

struct - C, C++, Go, Rust
record - Pascal, Haskell
dataclass - Python 3.7+
class (como struct) - C++, Java, C#

Enumerações:

enum - C, C++, Java, Rust, TypeScript, Swift
Enum - Python

Estruturas Especializadas
Filas:

queue/Queue - Python, Java, C++
deque - Python, C++

Pilhas:

stack/Stack - Java, C++, Python (usando list)

Heaps:

heapq - Python
PriorityQueue - Java
heap - Go

Árvores:

TreeMap/TreeSet - Java
map (red-black tree) - C++
Implementação manual na maioria

Grafos:

Geralmente implementação manual ou bibliotecas especializadas

Estruturas de Programação
Funções:

function/def/func/fn - Todas as linguagens
lambda/arrow functions - Python, JavaScript, C++, Java
closure - JavaScript, Python, Ruby, Swift

Classes e Objetos:

class - Python, Java, C++, C#, Ruby, JavaScript
trait - Rust, Scala
interface - Java, Go, TypeScript, C#
protocol - Swift

Módulos/Namespaces:

module - Python, Ruby, Elixir
namespace - C++, C#, PHP
package - Java, Go

Ponteiros e Referências:

pointer (*) - C, C++, Go
reference (&) - C++, Rust
Referências implícitas - Java, Python (objetos)

Genéricos/Templates:

template - C++
Generic<T> - Java, C#, TypeScript
<T> - Rust, Swift