# SpectraLang - Próximos Passos

**Data**: Janeiro de 2025  
**Fase Atual**: Fase 5 - Sistema de Tipos Avançados (100% completo) 🎉

## ✅ Recém Completado

### Sistema de Traits Avançado (100%) ✅
- ✅ **Trait Inheritance** (100%): Multi-nível com múltiplos parents
- ✅ **Default Implementations** (100%): Métodos opcionais com corpo em traits
- ✅ **Self Type** (100%): Keyword para referenciar tipo implementador
- ✅ **Generics** (100%): Parser completo com bounds (`<T: Trait>`)
- ✅ **Variable Shadowing** (100%): Scope stack com push/pop automático
- ✅ **Standard Library** (100%): Traits Clone, Debug, Default, Eq
- ✅ **Memory SSA** (100%): Alloca/Load/Store para variáveis mutáveis em loops

**Conquista Principal**: 45/45 testes passando (100%)! 🏆🎯🎉

---

## 🎯 Próximos Passos - Fase 6 (Features Avançadas)

### 🔥 Prioridade Máxima (Próximas 1-2 Semanas)

#### 1. Correção de Erros de Compilação (URGENTE) 🚨
**Tempo Estimado**: 1-2 dias  
**Dificuldade**: ⭐⭐

**Problema Atual**: Erros de importação no código (TypeAnnotation, Statement, Expression, etc.)

**Ações Necessárias**:
1. Verificar imports em todos os módulos
2. Corrigir dependências entre crates
3. Garantir que todos os testes compilem novamente
4. Validar que funcionalidades não foram quebradas

#### 2. Monomorphization - Especialização de Genéricos (50% → 100%)
**Tempo Estimado**: 4-5 dias  
**Dificuldade**: ⭐⭐⭐⭐

**Status Atual**: Parser funciona, mas sem codegen completo.

**Solução**:
1. Implementar substituição de tipos genéricos por concretos
2. Gerar versões especializadas de funções genéricas
3. Name mangling para evitar conflitos (`process_int`, `process_Point`)
4. Validação de trait bounds em tempo de compilação

```rust
// Exemplo de monomorphization
fn process<T: Clone>(item: T) -> T {
    return item.clone();
}

// Compilador gera automaticamente:
// process_int(item: int) -> int { ... }
// process_Point(item: Point) -> Point { ... }
```

**Implementação**:
1. **Type Substitution**: Substituir `T` por tipos concretos
2. **Monomorphization Pass**: Gerar versões especializadas
3. **Name Mangling**: `compare<int>` vs `compare<string>`
4. **Trait Bounds Validation**: Verificar que `T` implementa constraints

---

#### 3. Trait Objects - Dynamic Dispatch (0% → 100%)
**Tempo Estimado**: 6-8 dias  
**Dificuldade**: ⭐⭐⭐⭐⭐

**Objetivo**: Permitir polimorfismo em runtime com vtables

```spectra
trait Drawable {
    fn draw(&self);
}

fn render(shape: dyn Drawable) {
    shape.draw();  // Dynamic dispatch
}
```

**Requisitos**:
1. Fat pointers (data_ptr, vtable_ptr)
2. VTable generation automática
3. Dynamic method dispatch
4. Trait object safety validation

---

### ⭐ Curto Prazo (Próximas 2-3 Semanas)

#### 4. Associated Types
**Tempo Estimado**: 3-4 dias  
**Dificuldade**: ⭐⭐⭐⭐

**Objetivo**: Tipos associados a traits (alternativa mais limpa para alguns generics)

```spectra
trait Iterator {
    type Item;  // Associated type
    
    fn next(&self) -> Option<Self::Item>;
}

impl Iterator for Range {
    type Item = int;
    
    fn next(&self) -> Option<int> {
        // ...
    }
}
```

**Diferença vs Generics**:
- **Generic**: `trait Iterator<Item>` - múltiplas implementações por tipo
- **Associated**: `trait Iterator { type Item; }` - uma implementação por tipo

---

#### 5. Automatic Derivation (#[derive])
**Tempo Estimado**: 5-6 dias  
**Dificuldade**: ⭐⭐⭐⭐

**Objetivo**: Gerar implementações automáticas de traits comuns

```spectra
// Parser já reconhece isso:
trait Comparable<T> {
    func compare(other: T) -> int;
}

impl<T: Comparable<T>> Comparable<T> for Option<T> {
    func compare(other: Option<T>) -> int {
        // ...
    }
}
```

**Implementação**:
1. **Type Substitution**: Substituir `T` por tipos concretos
2. **Monomorphization Pass**: Gerar versões especializadas de funções genéricas
3. **Name Mangling**: `compare<int>` vs `compare<string>`
4. **Trait Bounds Validation**: Verificar que `T` implementa constraints

```rust
// Em midend/passes/monomorphization.rs (NOVO)
struct MonomorphizationPass {
    specializations: HashMap<(String, Vec<Type>), String>, // (generic_name, types) -> mangled_name
}

impl MonomorphizationPass {
    fn specialize_function(&mut self, func: &Function, type_args: &[Type]) -> String {
        let mangled = self.mangle_name(&func.name, type_args);
        
        if !self.specializations.contains_key(&(func.name.clone(), type_args.to_vec())) {
            // Gerar nova versão especializada
            let specialized = func.clone();
            self.substitute_types(&mut specialized, type_args);
            self.add_specialization(mangled.clone(), specialized);
        }
        
        mangled
    }
    
    fn substitute_types(&self, func: &mut Function, type_args: &[Type]) {
        // Substituir TypeParameter por tipos concretos
    }
}
```

**Teste Afetado**: 45

---

#### 5. Trait Bounds Validation
**Tempo Estimado**: 2 dias  
**Dificuldade**: ⭐⭐⭐

Validar constraints em tempo de compilação:

```spectra
func max<T: Comparable<T>>(a: T, b: T) -> T {
    if a.compare(b) > 0 {
        return a;
    }
    return b;
}

max(5, 10);        // ✅ int implementa Comparable
max("a", "b");     // ❌ string não implementa Comparable (ainda)
```

---

### 📚 Médio Prazo (2-4 Semanas)

#### 6. Standard Library Expansion
**Tempo Estimado**: 5-7 dias  
**Dificuldade**: ⭐⭐

Expandir stdlib com traits essenciais:

```spectra
// Comparação
trait Eq {
    func eq(other: Self) -> bool;
    func ne(other: Self) -> bool { return !self.eq(other); }  // Default
}

trait Ord: Eq {
    func cmp(other: Self) -> Ordering;
    func lt(other: Self) -> bool { return self.cmp(other) == Ordering::Less; }
    func gt(other: Self) -> bool { return self.cmp(other) == Ordering::Greater; }
}

enum Ordering {
    Less,
    Equal,
    Greater
}

// Iteradores
trait Iterator {
    type Item;  // Associated type (Fase 6)
    
    func next() -> Option<Self::Item>;
    
    func collect() -> Array<Self::Item> { /* default */ }
    func map<B, F: Fn(Self::Item) -> B>(f: F) -> Map<Self, F> { /* default */ }
    func filter<F: Fn(&Self::Item) -> bool>(f: F) -> Filter<Self, F> { /* default */ }
}

// Conversão
trait From<T> {
    func from(value: T) -> Self;
}

trait Into<T> {
    func into(self) -> T;
}

// Display
trait Display {
    func fmt() -> string;
}
```

---

### 🚀 Longo Prazo (1-3 Meses)

#### 7. Trait Objects (Dynamic Dispatch)
**Tempo Estimado**: 6-8 dias  
**Dificuldade**: ⭐⭐⭐⭐⭐

Permitir polimorfismo em runtime com vtables:

```spectra
trait Drawable {
    func draw();
}

struct Circle { radius: float }
struct Square { side: float }

impl Drawable for Circle {
    func draw() { println("Drawing circle"); }
}

impl Drawable for Square {
    func draw() { println("Drawing square"); }
}

// NOVO: dyn keyword para trait objects
func render(shape: dyn Drawable) {
    shape.draw();  // Dynamic dispatch via vtable
}

let c = Circle { radius: 5.0 };
let s = Square { side: 10.0 };
render(c);  // ✅
render(s);  // ✅
```

**Implementação**:
1. **Fat Pointers**: `(data_ptr, vtable_ptr)`
2. **VTable Generation**: Tabela de ponteiros para métodos
3. **Upcast**: Converter tipo concreto para trait object
4. **Runtime**: Dispatch via vtable lookup

```rust
// VTable structure
struct VTable {
    type_id: usize,
    destructor: fn(*mut u8),
    methods: Vec<fn(*mut u8, ...)>,  // Ponteiros para implementações
}

// Fat pointer representation
struct TraitObject {
    data: *mut u8,
    vtable: *const VTable,
}
```

---

#### 8. Automatic Derivation (#[derive])
**Tempo Estimado**: 5-6 dias  
**Dificuldade**: ⭐⭐⭐⭐

Gerar implementações automáticas de traits comuns:

```spectra
#[derive(Clone, Debug, Default, Eq)]
struct Point {
    x: int,
    y: int
}

// Compiler gera automaticamente:
impl Clone for Point {
    func clone() -> Self {
        return Point { x: self.x, y: self.y };
    }
}

impl Debug for Point {
    func fmt() -> string {
        return "Point { x: " + self.x.to_string() + ", y: " + self.y.to_string() + " }";
    }
}

impl Default for Point {
    func default() -> Self {
        return Point { x: 0, y: 0 };
    }
}

impl Eq for Point {
    func eq(other: Self) -> bool {
        return self.x == other.x && self.y == other.y;
    }
}
```

**Implementação**:
1. **Attribute Parser**: Reconhecer `#[derive(...)]`
2. **Trait Deriver**: Gerar AST para cada trait derivável
3. **Validation**: Verificar que todos os campos implementam o trait
4. **Codegen**: Inserir implementações geradas no AST

---

#### 9. Associated Types
**Tempo Estimado**: 4-5 dias  
**Dificuldade**: ⭐⭐⭐⭐

Tipos associados a traits (mais limpo que generics em alguns casos):

```spectra
trait Iterator {
    type Item;  // Associated type
    
    func next() -> Option<Self::Item>;
}

impl Iterator for Range {
    type Item = int;  // Concretização
    
    func next() -> Option<int> {
        // ...
    }
}

func sum<I: Iterator>(iter: I) -> I::Item 
    where I::Item: Add<I::Item> 
{
    let total = I::Item::default();
    while let Some(val) = iter.next() {
        total = total + val;
    }
    return total;
}
```

**Diferença vs Generics**:
- **Generic**: `trait Iterator<Item>` - múltiplas implementações por tipo
- **Associated**: `trait Iterator { type Item; }` - uma implementação por tipo

---

#### 10. Lifetimes e Borrowing (Fase 6)
**Tempo Estimado**: 12+ dias  
**Dificuldade**: ⭐⭐⭐⭐⭐

Sistema de ownership completo inspirado em Rust:

```spectra
func longest<'a>(x: &'a str, y: &'a str) -> &'a str {
    if x.len() > y.len() {
        return x;
    }
    return y;
}

struct Foo<'a> {
    data: &'a int
}
```

---

## 📊 Roadmap Visual

```
✅ COMPLETO (Fase 5)
├── ✅ Trait Inheritance              100%
├── ✅ Default Implementations         100%
├── ✅ Self Type                       100%
├── ✅ Generics (Parser + Bounds)     100%
├── ✅ Variable Shadowing              100%
├── ✅ Memory SSA                      100%
└── ✅ Standard Library Básica         100%

🚨 URGENTE (Semana 1)
└── 🔧 Correção de Erros de Compilação

Curto Prazo (Semana 2-3)
├── Monomorphization Completo         ⭐⭐⭐⭐
└── Trait Objects (dyn Trait)         ⭐⭐⭐⭐⭐

Médio Prazo (Mês 2)
├── Associated Types                  ⭐⭐⭐⭐
├── Automatic Derivation              ⭐⭐⭐⭐
└── Standard Library Expansion        ⭐⭐

Longo Prazo (Mês 3+)
├── Lifetimes/Borrowing               ⭐⭐⭐⭐⭐
├── Async/Await                       ⭐⭐⭐⭐⭐
└── Macros                            ⭐⭐⭐⭐⭐
```

---

## 🎯 Status de Testes

| Milestone | Testes Passando | Percentual | Status |
|-----------|-----------------|------------|--------|
| Fase 5 Completa | 45/45 | 100.00% | ✅ Alcançado |
| Após Correções | 45/45 | 100.00% | 🎯 Meta |
| Com Monomorphization | 50+/50+ | 100.00% | 🔮 Futuro |
| Sistema Completo | 100+/100+ | 100.00% | 🚀 Visão |

---

## 📝 Notas de Implementação

### Arquivos Principais a Modificar

**Para Default Implementations Codegen**:
- `compiler/src/semantic/mod.rs`: Adicionar IR generation para default bodies
- `midend/src/lowering.rs`: Copiar IR de defaults para implementações

**Para Métodos Estáticos**:
- `compiler/src/token.rs`: Keyword::Static
- `compiler/src/parser/item.rs`: Parsing de static func
- `compiler/src/ast/mod.rs`: TraitMethod.is_static
- `compiler/src/semantic/mod.rs`: Validação (sem self)

**Para Monomorphization**:
- `midend/src/passes/monomorphization.rs`: Novo arquivo
- `midend/src/lib.rs`: Registrar pass
- `midend/src/ir.rs`: Suporte para tipos especializados

**Para Trait Objects**:
- `compiler/src/ast/mod.rs`: Type::TraitObject
- `compiler/src/semantic/mod.rs`: Upcast validation
- `midend/src/lowering.rs`: VTable generation
- `runtime/src/lib.rs`: Fat pointer support

---

## 🎓 Recursos de Aprendizado

### Referências Técnicas
- **Rust Reference - Traits**: https://doc.rust-lang.org/reference/items/traits.html
- **Trait Objects**: https://doc.rust-lang.org/book/ch17-02-trait-objects.html
- **Monomorphization**: https://rustc-dev-guide.rust-lang.org/backend/monomorph.html
- **VTable Layout**: https://gankra.github.io/blah/tower-of-weakenings/

### Papers
- "Traits: Composable Units of Behaviour" (Schärli et al., 2003)
- "System F with Type Equality Coercions" (Weirich et al., 2011)

---

**Status**: 🟢 Sistema de traits robusto e bem documentado! Próximos passos claros
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
