# SpectraLang - Próximos Passos

**Data**: 1 de Novembro de 2025  
**Fase Atual**: Fase 1 - Compilador Básico (70% completo)

## ✅ Recém Completado

### Arrays (100%)
- ✅ Sintaxe completa: literais, indexação, atribuição
- ✅ Type checking e inferência
- ✅ IR generation com GetElementPtr
- ✅ Backend com solução SSA para loops
- ✅ Testes completos incluindo loops aninhados

**Conquista Principal**: Arrays funcionam em qualquer contexto, incluindo loops complexos!

---

## 🎯 Próximos Passos - Fase 1 (Prioridades)

### 1. Strings Completas (Prioridade: ALTA)
**Status**: Parcial - Apenas literais básicos  
**Tempo Estimado**: 2-3 dias

#### Tarefas:
- [ ] **String Concatenation**: `"hello" + " world"`
- [ ] **String Interpolation**: `"x = {x}, y = {y}"`
- [ ] **String Methods**:
  - `.len()` - comprimento
  - `.chars()` - iterador de caracteres
  - `.split(delimiter)` - dividir em array
  - `.contains(substring)` - verificar substring
  - `.starts_with(prefix)` - verificar prefixo
  - `.ends_with(suffix)` - verificar sufixo
- [ ] **String Indexing**: `s[i]` retorna char
- [ ] **String Slicing**: `s[0..3]` retorna substring
- [ ] **Escape Sequences**: `\n`, `\t`, `\\`, `\"`

#### Implementação:
1. Parser: Interpolation syntax, escape sequences
2. AST: StringInterpolation, StringConcat nodes
3. Semantic: String method resolution
4. IR: String operations (concat, indexing)
5. Runtime: String allocation e manipulation

---

### 2. Structs (Prioridade: ALTA)
**Status**: Não iniciado  
**Tempo Estimado**: 3-4 dias

#### Sintaxe Proposta:
```spectra
// Declaração
struct Point {
    x: int,
    y: int
}

// Instanciação
let p = Point { x: 10, y: 20 };

// Acesso a campos
let x_val = p.x;
p.y = 30;

// Structs aninhados
struct Rectangle {
    top_left: Point,
    bottom_right: Point
}
```

#### Tarefas:
- [ ] **Parser**: Struct declarations, struct literals, field access
- [ ] **AST**: StructDeclaration, StructLiteral, FieldAccess
- [ ] **Semantic**: Struct type checking, field validation
- [ ] **IR**: Struct layout, field offset calculation
- [ ] **Backend**: Struct allocation, field access via GEP

#### Desafios:
- Memory layout (alignment, padding)
- Nested structs
- Struct copy vs move semantics

---

### 3. Enums e Pattern Matching Básico (Prioridade: MÉDIA)
**Status**: Não iniciado  
**Tempo Estimado**: 4-5 dias

#### Sintaxe Proposta:
```spectra
// Enum simples
enum Status {
    Success,
    Error,
    Pending
}

// Enum com dados
enum Option<T> {
    Some(T),
    None
}

// Pattern matching
match status {
    Status::Success => print("OK"),
    Status::Error => print("Failed"),
    Status::Pending => print("Waiting")
}
```

#### Tarefas:
- [ ] **Parser**: Enum declarations, match expressions
- [ ] **AST**: EnumDeclaration, MatchExpression, Pattern
- [ ] **Semantic**: Enum type checking, exhaustiveness checking
- [ ] **IR**: Enum representation (tag + payload)
- [ ] **Backend**: Enum layout, match code generation

---

### 4. Tuples (Prioridade: MÉDIA)
**Status**: Não iniciado  
**Tempo Estimado**: 2 dias

#### Sintaxe Proposta:
```spectra
// Declaração
let tuple = (1, "hello", true);

// Acesso indexado
let first = tuple.0;
let second = tuple.1;

// Destructuring
let (x, y, z) = tuple;

// Retorno múltiplo
fn divide(a: int, b: int) -> (int, int) {
    return (a / b, a % b);
}

let (quotient, remainder) = divide(10, 3);
```

#### Tarefas:
- [ ] **Parser**: Tuple literals, tuple indexing, destructuring
- [ ] **AST**: TupleLiteral, TupleAccess, TuplePattern
- [ ] **Semantic**: Tuple type inference, destructuring validation
- [ ] **IR**: Tuple as struct
- [ ] **Backend**: Tuple layout

---

### 5. Estruturas de Controle Restantes (Prioridade: BAIXA)
**Status**: Parcial (if/while/for já implementados)  
**Tempo Estimado**: 2-3 dias

#### Faltam:
- [ ] **switch/case**: Similar a match mas mais simples
- [ ] **loop**: Loop infinito com break
- [ ] **do while**: Executa pelo menos uma vez
- [ ] **unless**: Inverso de if

#### Sintaxe:
```spectra
// switch/case
switch x {
    case 1 => print("one"),
    case 2 => print("two"),
    default => print("other")
}

// loop
loop {
    if condition { break; }
}

// do while
do {
    // código
} while condition;

// unless
unless condition {
    // código se condição falsa
}
```

---

### 6. Implicit Returns (Prioridade: ALTA - Bug Fix)
**Status**: Bug conhecido  
**Tempo Estimado**: 1 dia

#### Problema:
Funções com loops não conseguem usar implicit return:
```spectra
fn test() -> int {
    let i = 0;
    while i < 3 {
        i = i + 1;
    }
    i  // ❌ Falha na compilação
}

fn test() -> int {
    let i = 0;
    while i < 3 {
        i = i + 1;
    }
    return i;  // ✅ Funciona
}
```

#### Solução:
- Verificar se último statement do bloco é expressão
- Usar valor dessa expressão como return implícito
- Mesmo após loops/condicionais

---

## 📊 Ordem Recomendada de Implementação

### Sprint 1 (3-4 dias): Fundações
1. **Fix Implicit Returns** (1 dia) - Remove limitação chata
2. **Strings Completas** (2-3 dias) - Feature essencial

### Sprint 2 (3-4 dias): Tipos Compostos
3. **Structs** (3-4 dias) - Base para OOP futuro

### Sprint 3 (4-5 dias): Pattern Matching
4. **Tuples** (2 dias) - Facilita múltiplos retornos
5. **Enums Básicos** (2-3 dias) - Sem pattern matching ainda

### Sprint 4 (2-3 dias): Controle de Fluxo
6. **Estruturas Restantes** (2-3 dias) - switch, loop, do-while, unless

### Sprint 5 (4-5 dias): Pattern Matching Completo
7. **Pattern Matching** (4-5 dias) - Integrar com enums

---

## 🔮 Futuro (Fase 2)

### Recursos Avançados (3-6 meses)
- [ ] Generics (`fn max<T>(a: T, b: T) -> T`)
- [ ] Traits/Interfaces
- [ ] Closures e First-class functions
- [ ] Standard Library
- [ ] Collections (Vec, HashMap, HashSet)
- [ ] IO (File, Network)
- [ ] Error Handling (Result<T,E>, try/catch)
- [ ] Async/Await
- [ ] Modules e Packages
- [ ] Macros

### Otimizações
- [ ] Constant folding
- [ ] Dead code elimination (expandir)
- [ ] Inline expansion
- [ ] Loop optimizations
- [ ] Tail call optimization

### Tooling
- [ ] LSP (Language Server Protocol)
- [ ] Debugger
- [ ] Package Manager
- [ ] Build System
- [ ] Formatter
- [ ] Linter

---

## 💡 Sugestões de Próximo Trabalho

### Opção 1: Quick Wins (Recomendado)
**Objetivo**: Resolver bugs e adicionar features essenciais
1. Fix implicit returns (1 dia)
2. String concatenation e methods (2 dias)
3. Tuples (2 dias)

**Total**: ~5 dias, 3 features importantes

### Opção 2: Feature Completa
**Objetivo**: Implementar structs completamente
1. Structs (3-4 dias)

**Total**: ~4 dias, 1 feature grande e poderosa

### Opção 3: Pattern Matching Path
**Objetivo**: Caminho para enums e pattern matching
1. Tuples (2 dias)
2. Enums básicos (3 dias)
3. Pattern matching (4 dias)

**Total**: ~9 dias, sistema de tipos muito mais poderoso

---

## 📈 Métricas de Progresso

### Fase 1 - Status Atual: 70%
- ✅ Parser: 100%
- ✅ Lexer: 100%
- ✅ AST: 100%
- ✅ Operadores: 100%
- ✅ Controle de Fluxo Básico: 50%
- ✅ Sistema de Tipos: 100%
- ✅ Arrays: 100%
- ⏳ Strings: 30%
- ⏳ Structs: 0%
- ⏳ Enums: 0%
- ⏳ Tuples: 0%
- ⏳ Pattern Matching: 0%

### Meta para 80%
Completar:
- Implicit returns fix
- Strings completas
- Structs
- Tuples

### Meta para 90%
Adicionar:
- Enums básicos
- Estruturas de controle restantes

### Meta para 100% (Fase 1)
Adicionar:
- Pattern matching
- Runtime completo
- Testes abrangentes

---

## 🚀 Decisão Recomendada

**Começar com Opção 1 (Quick Wins)**:

1. **Dia 1**: Fix implicit returns
   - Permite código mais limpo
   - Remove limitação irritante
   
2. **Dias 2-3**: String concatenation e methods
   - Feature essencial para qualquer linguagem
   - Base para IO futuro
   
3. **Dias 4-5**: Tuples
   - Útil para múltiplos retornos
   - Preparação para destructuring

Após isso, partir para **Structs** (Opção 2) ou continuar com **Enums** (Opção 3) dependendo das prioridades.

---

**Qual caminho você prefere seguir?** 🤔
