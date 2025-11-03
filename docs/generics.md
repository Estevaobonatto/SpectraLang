# Generics - SpectraLang

## Visão Geral

SpectraLang possui um sistema completo de genéricos (generics) com monomorphization, trait bounds e inferência automática de tipos. Esta documentação detalha a implementação e uso de generics na linguagem.

## Status da Implementação

### ✅ Implementado
- [x] Structs genéricos com múltiplos parâmetros de tipo
- [x] Enums genéricos com múltiplos parâmetros de tipo
- [x] Funções genéricas com trait bounds
- [x] Monomorphization (geração de código especializado)
- [x] Inferência automática de type arguments
- [x] Validação de trait bounds em tempo de compilação
- [x] Pattern matching com tipos genéricos

### 🚧 Em Desenvolvimento
- [ ] Inferência de tipos para enum variants
- [ ] Generic methods em structs/enums
- [ ] Trait implementations para tipos genéricos

### 📋 Planejado
- [ ] Associated types
- [ ] Higher-kinded types
- [ ] Standard library completa com tipos genéricos

## Sintaxe

### Definição de Structs Genéricos

```spectra
// Single type parameter
struct Box<T> {
    value: T,
}

// Multiple type parameters
struct Pair<T, U> {
    first: T,
    second: U,
}

// Com campos variados
struct Container<T> {
    data: T,
    count: int,
    active: bool,
}
```

### Definição de Enums Genéricos

```spectra
// Option type
enum Option<T> {
    Some(T),
    None,
}

// Result type
enum Result<T, E> {
    Ok(T),
    Err(E),
}

// Multiple variants with data
enum Either<L, R> {
    Left(L),
    Right(R),
}
```

### Definição de Funções Genéricas

```spectra
// Simple generic function
fn identity<T>(x: T) -> T {
    return x;
}

// Multiple type parameters
fn pair<T, U>(first: T, second: U) -> Pair<T, U> {
    return Pair { first: first, second: second };
}

// With trait bounds
fn clone_value<T: Clone>(x: T) -> T {
    return x.clone();
}
```

## Instanciação

### Com Type Arguments Explícitos

```spectra
// Structs
let p1 = Point<int> { x: 10, y: 20 };
let p2 = Point<float> { x: 3.14, y: 2.71 };
let pair = Pair<int, string> { first: 42, second: "hello" };

// Enums
let opt1 = Option<int>::Some(42);
let opt2 = Option<string>::None;
let res = Result<int, string>::Ok(100);

// Functions
let x = identity<int>(42);
let y = pair<int, float>(10, 3.14);
```

### Com Inferência de Tipos ✨

```spectra
// Structs - type arguments inferidos automaticamente!
let p1 = Point { x: 10, y: 20 };           // Infere Point<int>
let p2 = Point { x: 3.14, y: 2.71 };       // Infere Point<float>
let pair = Pair { first: 42, second: 1.5 }; // Infere Pair<int, float>

// Type arguments explícitos ainda funcionam
let p3 = Point<int> { x: 100, y: 200 };    // OK!
```

**Como funciona a inferência:**
1. O compilador analisa os tipos dos valores dos campos
2. Mapeia cada parâmetro de tipo (T, U) para um tipo concreto
3. Valida a consistência das inferências
4. Preenche automaticamente os type arguments

## Monomorphization

SpectraLang usa **monomorphization** para implementar generics, gerando código especializado para cada tipo concreto usado:

### Exemplo de Monomorphization

```spectra
struct Point<T> {
    x: T,
    y: T,
}

fn main() -> int {
    let p1 = Point { x: 10, y: 20 };        // Point<int>
    let p2 = Point { x: 3.14, y: 2.71 };    // Point<float>
    return 0;
}
```

**Código gerado internamente:**
```
Point<int> → Point_int   // Especialização com campos int
Point<float> → Point_float // Especialização com campos float
```

**Logs de compilação:**
```
Info: Stored generic struct 'Point' for monomorphization
Info: Inferred type arguments for struct 'Point': [int]
Info: Specialized struct 'Point' as 'Point_int'
Info: Inferred type arguments for struct 'Point': [float]
Info: Specialized struct 'Point' as 'Point_float'
```

### Vantagens da Monomorphization
- ✅ Zero overhead em runtime
- ✅ Performance equivalente a código não-genérico
- ✅ Melhor otimização pelo compilador
- ⚠️ Aumenta o tamanho do binário (uma cópia por tipo)

## Trait Bounds

### Sintaxe

```spectra
// Single trait bound
fn duplicate<T: Clone>(value: T) -> T {
    return value.clone();
}

// Multiple trait bounds (futuro)
fn compare<T: Ord + Debug>(a: T, b: T) -> bool {
    return a < b;
}
```

### Traits da Standard Library

```spectra
// Clone trait
trait Clone {
    fn clone(self: Self) -> Self;
}

// Debug trait
trait Debug {
    fn debug(self: Self) -> string;
}

// Default trait
trait Default {
    fn default() -> Self;
}
```

### Validação de Bounds

O compilador valida trait bounds em tempo de compilação:

```spectra
fn needs_clone<T: Clone>(x: T) -> T {
    return x.clone();
}

struct Point { x: int, y: int }

// Sem implementação de Clone
let p = Point { x: 10, y: 20 };
let p2 = needs_clone(p);  // ❌ ERRO: Point não implementa Clone

// Com implementação
impl Clone for Point {
    fn clone(self: Self) -> Self {
        return Point { x: self.x, y: self.y };
    }
}

let p3 = needs_clone(p);  // ✅ OK agora!
```

## Pattern Matching com Generics

```spectra
enum Option<T> {
    Some(T),
    None,
}

fn unwrap_or<T>(opt: Option<T>, default: T) -> T {
    match opt {
        Option<T>::Some(x) => return x,
        Option<T>::None => return default,
    }
}
```

## Exemplos Práticos

### Container Genérico

```spectra
struct Container<T> {
    value: T,
    count: int,
}

fn main() -> int {
    let c1 = Container { value: 42, count: 1 };      // Container<int>
    let c2 = Container { value: "hello", count: 2 }; // Container<string>
    
    return c1.value + c2.count;
}
```

### Lista Ligada (Futuro)

```spectra
enum List<T> {
    Cons(T, Box<List<T>>),
    Nil,
}

fn length<T>(list: List<T>) -> int {
    match list {
        List::Cons(_, tail) => return 1 + length(*tail),
        List::Nil => return 0,
    }
}
```

### Option Type

```spectra
enum Option<T> {
    Some(T),
    None,
}

fn map<T, U>(opt: Option<T>, f: fn(T) -> U) -> Option<U> {
    match opt {
        Option::Some(x) => return Option::Some(f(x)),
        Option::None => return Option::None,
    }
}
```

## Limitações Atuais

### Não Suportado (Ainda)
1. **Generic Methods**: Métodos em tipos genéricos
   ```spectra
   impl<T> Point<T> {
       fn new(x: T, y: T) -> Point<T> { ... }  // Futuro
   }
   ```

2. **Trait Implementations para Generics**: 
   ```spectra
   impl<T: Clone> Clone for Point<T> { ... }  // Futuro
   ```

3. **Associated Types**:
   ```spectra
   trait Iterator {
       type Item;  // Futuro
       fn next(self: Self) -> Option<Self::Item>;
   }
   ```

4. **Where Clauses**:
   ```spectra
   fn complex<T, U>(x: T, y: U) -> bool
       where T: Clone, U: Debug { ... }  // Futuro
   ```

### Workarounds

Para algumas limitações, existem workarounds temporários:

```spectra
// Em vez de generic methods:
// impl<T> Point<T> { fn new(...) }

// Use funções livres:
fn new_point<T>(x: T, y: T) -> Point<T> {
    return Point { x: x, y: y };
}
```

## Desempenho

### Características
- **Zero overhead**: Código genérico tem a mesma performance que código especializado manualmente
- **Compile-time**: Toda a resolução acontece em tempo de compilação
- **Binary size**: Cada tipo concreto gera código adicional

### Comparação

```spectra
// Código genérico
struct Point<T> { x: T, y: T }
let p1 = Point { x: 10, y: 20 };      // ~100 bytes no binário
let p2 = Point { x: 3.14, y: 2.71 };  // +100 bytes no binário

// Código manual equivalente
struct PointInt { x: int, y: int }
struct PointFloat { x: float, y: float }
let p1 = PointInt { x: 10, y: 20 };   // ~100 bytes no binário
let p2 = PointFloat { x: 3.14, y: 2.71 }; // +100 bytes no binário
```

**Performance**: Idêntica ✅  
**Ergonomia**: Generics são muito melhores! ✨

## Próximos Passos

### Curto Prazo
1. **Enum Type Inference**: `Option::Some(42)` → inferir `Option<int>`
2. **Generic Methods**: `impl<T> Point<T> { fn new(...) }`
3. **Contextual Inference**: Inferir tipos do uso

### Médio Prazo
4. **Trait Implementations para Generics**
5. **Associated Types**
6. **Standard Library Completa**: Vec<T>, HashMap<K,V>, etc.

### Longo Prazo
7. **Higher-Kinded Types**
8. **Generic Const Parameters**: `Array<T, N: const int>`
9. **Variadic Generics**

## Referências

- [Rust Generics](https://doc.rust-lang.org/book/ch10-00-generics.html) - Inspiração principal
- [C++ Templates](https://en.cppreference.com/w/cpp/language/templates) - Conceitos de monomorphization
- [Haskell Type Classes](https://www.haskell.org/tutorial/classes.html) - Conceitos de traits

## Testes

Testes localizados em:
- `test_generic_instantiate.spectra` - Instanciação de structs e enums
- `test_generic_enum.spectra` - Enums genéricos
- `test_type_inference.spectra` - Inferência automática de tipos
- `tests/validation/56_monomorphization.spectra` - Validação completa

Execute:
```bash
./target/release/spectra-cli test_type_inference.spectra
```
