# Plano de Implementação - Features Faltantes

## 🎯 Meta: Atingir 80% (64/80 features)

**Status Atual**: 18/80 (22.5%)  
**Faltam**: 46 features  
**Tempo Estimado**: 7-10 semanas

---

## 📋 Fase 1: Estruturas de Dados Fundamentais (2-3 semanas)

### 1.1 Arrays (Prioridade MÁXIMA)
**Tempo**: 3-4 dias

#### Parser
- [ ] Literais de array: `[1, 2, 3, 4]`
- [ ] Tipo array: `array<int, 4>` ou `int[4]`
- [ ] Indexação: `arr[index]`
- [ ] Atribuição indexada: `arr[0] = 10`

#### AST
- [ ] `ArrayLiteral { elements: Vec<Expression>, ty: Type }`
- [ ] `ArrayType { element_type: Type, size: usize }`
- [ ] `IndexAccess { array: Box<Expression>, index: Box<Expression> }`

#### Semantic
- [ ] Type checking de arrays
- [ ] Validação de bounds (se possível em compile-time)
- [ ] Inferência de tipo de array

#### IR/Backend
- [ ] Alocação de arrays (stack)
- [ ] Acesso indexado
- [ ] Store indexado

#### Testes
- [ ] Arrays básicos
- [ ] Indexação
- [ ] Modificação de elementos
- [ ] Arrays aninhados

---

### 1.2 Strings Completas (Prioridade ALTA)
**Tempo**: 2-3 dias

#### Features
- [ ] Concatenação: `str1 + str2`
- [ ] Comparação: `str1 == str2`
- [ ] Interpolação: `"Hello {name}"`
- [ ] Escape sequences: `\n`, `\t`, `\\`, `\"`

#### Métodos Básicos
- [ ] `.len()` - comprimento
- [ ] `.chars()` - iterador de caracteres
- [ ] `.bytes()` - iterador de bytes

#### IR/Backend
- [ ] String como ponteiro + tamanho
- [ ] Alocação de strings
- [ ] Operações de string

---

### 1.3 Structs (Prioridade ALTA)
**Tempo**: 4-5 dias

#### Sintaxe
```spectra
struct Point {
    x: int,
    y: int
}

let p = Point { x: 10, y: 20 };
let x_val = p.x;
p.y = 30;
```

#### Parser
- [ ] Declaração de struct
- [ ] Literal de struct
- [ ] Acesso a campo: `struct.field`
- [ ] Atribuição a campo: `struct.field = value`

#### AST
- [ ] `StructDeclaration`
- [ ] `StructLiteral`
- [ ] `FieldAccess`

#### Semantic
- [ ] Registro de structs
- [ ] Validação de campos
- [ ] Type checking

#### IR/Backend
- [ ] Layout de memória de structs
- [ ] Acesso a campos (offset calculation)
- [ ] Construção de structs

---

### 1.4 Enums (Prioridade ALTA)
**Tempo**: 3-4 dias

#### Sintaxe
```spectra
enum Color {
    Red,
    Green,
    Blue
}

enum Option<T> {
    Some(T),
    None
}

let c = Color::Red;
```

#### Parser
- [ ] Declaração de enum
- [ ] Variantes simples
- [ ] Variantes com dados
- [ ] Path notation: `Enum::Variant`

#### AST
- [ ] `EnumDeclaration`
- [ ] `EnumVariant`
- [ ] `EnumLiteral`

#### Semantic
- [ ] Registro de enums
- [ ] Validação de variantes
- [ ] Type checking

#### IR/Backend
- [ ] Representação tagged union
- [ ] Discriminator
- [ ] Pattern matching (preparação)

---

### 1.5 Tuples (Prioridade MÉDIA)
**Tempo**: 2-3 dias

#### Sintaxe
```spectra
let tuple = (1, "hello", true);
let first = tuple.0;
let second = tuple.1;

fn return_multiple() -> (int, string) {
    return (42, "answer");
}
```

#### Parser
- [ ] Literais de tuple: `(val1, val2, ...)`
- [ ] Acesso por índice: `tuple.0`, `tuple.1`
- [ ] Tipos de tuple: `(int, string, bool)`

#### AST
- [ ] `TupleLiteral`
- [ ] `TupleAccess`
- [ ] `TupleType`

---

## 📋 Fase 2: Pattern Matching e Loops Avançados (2-3 semanas)

### 2.1 match/case (Prioridade ALTA)
**Tempo**: 5-6 dias

#### Sintaxe
```spectra
match value {
    1 => "one",
    2 => "two",
    3 | 4 | 5 => "few",
    x if x > 10 => "many",
    _ => "other"
}

match color {
    Color::Red => "red",
    Color::Green => "green",
    Color::Blue => "blue"
}
```

#### Parser
- [ ] `match` expression
- [ ] Pattern matching
- [ ] Guards: `if condition`
- [ ] Wildcard: `_`
- [ ] OR patterns: `|`

#### AST
- [ ] `MatchExpression`
- [ ] `Pattern` (vários tipos)
- [ ] `MatchArm { pattern, guard, body }`

#### Semantic
- [ ] Exhaustiveness checking
- [ ] Unreachable pattern detection
- [ ] Type checking de patterns

#### IR/Backend
- [ ] Lowering para switch ou if-else chain
- [ ] Destruturação de patterns

---

### 2.2 for loop C-style (Prioridade ALTA)
**Tempo**: 2 dias

#### Sintaxe
```spectra
for let i = 0; i < 10; i = i + 1 {
    // body
}
```

#### Parser
- [ ] `for` com init; condition; increment

#### Desugaring
- [ ] Transformar em:
  ```spectra
  {
      init;
      while condition {
          body
          increment
      }
  }
  ```

---

### 2.3 foreach (Prioridade MÉDIA)
**Tempo**: 2 dias

#### Sintaxe
```spectra
foreach item in array {
    // body
}

foreach key, value in dict {
    // body
}
```

#### Parser
- [ ] `foreach` keyword
- [ ] Pattern destructuring (key, value)

---

### 2.4 repeat-until (Prioridade BAIXA)
**Tempo**: 1 dia

#### Sintaxe
```spectra
repeat {
    // body
} until condition;
```

---

## 📋 Fase 3: Programação Funcional (2-3 semanas)

### 3.1 Lambdas (Prioridade ALTA)
**Tempo**: 4-5 dias

#### Sintaxe
```spectra
let add = |x, y| x + y;
let result = add(10, 20);

let double = |x: int| -> int { x * 2 };

// Como argumento
map(array, |x| x * 2);
```

#### Parser
- [ ] `|args| expr`
- [ ] `|args| { block }`
- [ ] Tipos opcionais

#### AST
- [ ] `Lambda { params, body, return_type }`

#### Semantic
- [ ] Type inference para lambdas
- [ ] Closure detection

---

### 3.2 Closures (Prioridade ALTA)
**Tempo**: 5-6 dias

#### Features
- [ ] Captura de variáveis por valor
- [ ] Captura de variáveis por referência
- [ ] Move semantics

#### IR/Backend
- [ ] Criar struct para closure
- [ ] Captured variables como campos
- [ ] Função + environment pointer

---

### 3.3 yield e Generators (Prioridade MÉDIA)
**Tempo**: 4-5 dias

#### Sintaxe
```spectra
fn fibonacci() yield int {
    let a = 0;
    let b = 1;
    
    loop {
        yield a;
        let temp = a + b;
        a = b;
        b = temp;
    }
}

for num in fibonacci() {
    if num > 100 {
        break;
    }
    print(num);
}
```

---

## 📋 Fase 4: Coleções Avançadas (1-2 semanas)

### 4.1 Vector (Prioridade ALTA)
**Tempo**: 3-4 dias

```spectra
let vec = vector<int>();
vec.push(10);
vec.push(20);
let val = vec.pop();
let len = vec.len();
```

---

### 4.2 Dict/Map (Prioridade ALTA)
**Tempo**: 4-5 dias

```spectra
let map = dict<string, int>();
map.set("key", 42);
let val = map.get("key");
map.remove("key");
```

---

### 4.3 Set (Prioridade MÉDIA)
**Tempo**: 3 dias

```spectra
let s = set<int>();
s.add(10);
s.contains(10); // true
s.remove(10);
```

---

### 4.4 Queue, Stack, LinkedList (Prioridade MÉDIA)
**Tempo**: 2 dias cada

Standard library implementations.

---

## 📋 Fase 5: Orientação a Objetos (3-4 semanas)

### 5.1 Classes Básicas (Prioridade ALTA)
**Tempo**: 6-7 dias

#### Sintaxe
```spectra
class Person {
    name: string;
    age: int;
    
    fn new(name: string, age: int) -> Person {
        return Person { name: name, age: age };
    }
    
    fn greet(self) {
        print("Hello, I'm " + self.name);
    }
    
    fn birthday(mut self) {
        self.age = self.age + 1;
    }
}

let p = Person::new("Alice", 30);
p.greet();
p.birthday();
```

#### Parser
- [ ] `class` keyword
- [ ] Fields
- [ ] Methods (self, mut self, &self)
- [ ] Associated functions (::)

---

### 5.2 Traits/Interfaces (Prioridade ALTA)
**Tempo**: 5-6 dias

#### Sintaxe
```spectra
trait Drawable {
    fn draw(self);
}

class Circle {
    radius: float;
}

impl Drawable for Circle {
    fn draw(self) {
        // desenha círculo
    }
}
```

---

### 5.3 Herança (Prioridade MÉDIA)
**Tempo**: 4-5 dias

```spectra
class Animal {
    name: string;
    
    fn speak(self) {
        print("Some sound");
    }
}

class Dog extends Animal {
    breed: string;
    
    fn speak(self) {
        print("Woof!");
    }
}
```

---

## 📋 Fase 6: Genéricos e Avançado (2-3 semanas)

### 6.1 Generics Básicos (Prioridade ALTA)
**Tempo**: 7-8 dias

```spectra
fn identity<T>(x: T) -> T {
    return x;
}

struct Box<T> {
    value: T
}

let int_box = Box<int> { value: 42 };
let str_box = Box<string> { value: "hello" };
```

---

### 6.2 Sistema de Módulos Completo (Prioridade ALTA)
**Tempo**: 4-5 dias

```spectra
// file: math.spectra
module math;

pub fn add(a: int, b: int) -> int {
    return a + b;
}

// file: main.spectra
module main;

import math;

pub fn main() {
    let result = math::add(10, 20);
}
```

---

## 📋 Fase 7: Standard Library (2-3 semanas)

### 7.1 Módulo io
- [ ] print, println
- [ ] read_line
- [ ] File I/O

### 7.2 Módulo collections
- [ ] Todas as estruturas de dados

### 7.3 Módulo string
- [ ] Operações de string

### 7.4 Módulo math
- [ ] Funções matemáticas

---

## 🎯 Resumo do Plano

| Fase | Features | Tempo | Prioridade |
|------|----------|-------|------------|
| 1 | Arrays, Strings, Structs, Enums, Tuples | 2-3 sem | ALTA |
| 2 | match, for C-style, foreach | 2-3 sem | ALTA |
| 3 | Lambdas, Closures, yield | 2-3 sem | ALTA |
| 4 | Vector, Dict, Set, etc | 1-2 sem | MÉDIA |
| 5 | Classes, Traits, Herança | 3-4 sem | MÉDIA |
| 6 | Generics, Módulos | 2-3 sem | ALTA |
| 7 | Standard Library | 2-3 sem | MÉDIA |

**Total**: 14-21 semanas para implementação completa (100%)

**Para 80% (64 features)**: 7-10 semanas focando nas fases 1, 2, 3 e 6.

---

## 🚀 Início Imediato

Vamos começar com **Arrays** agora mesmo!
