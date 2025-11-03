# Sistema de Tipos - SpectraLang

## Visão Geral

O SpectraLang possui um sistema de tipos estáticos com inferência automática. O analisador semântico realiza verificação de tipos em tempo de compilação para prevenir erros de tipo.

## Tipos Primitivos

### Tipos Básicos
- **int**: Números inteiros (ex: 42, -10, 0)
- **float**: Números de ponto flutuante (ex: 3.14, -0.5, 2.0)
- **bool**: Valores booleanos (true, false)
- **string**: Texto (ex: "Hello", "SpectraLang")
- **char**: Caractere único (ex: 'a', 'Z', '1')

### Tipos Especiais
- **Unit**: Tipo vazio, usado para funções sem retorno
- **Unknown**: Tipo desconhecido, usado durante inferência

## Inferência de Tipos

### Variáveis
O tipo de uma variável é inferido automaticamente do valor atribuído:

```spectra
let x = 42;         // int
let y = 3.14;       // float
let name = "Alice"; // string
let flag = true;    // bool
```

### Expressões
O tipo de expressões é inferido baseado nos operandos:

```spectra
let sum = 10 + 20;      // int (int + int = int)
let result = 5 > 3;     // bool (comparação retorna bool)
let product = 2.5 * 4;  // float (float * int = float)
```

### Funções
O tipo de retorno de funções pode ser declarado explicitamente:

```spectra
fn add(a: int, b: int) -> int {
    return a + b;
}

let result = add(10, 20);  // result tem tipo int
```

## Validação de Tipos

### Operações Aritméticas
Operadores aritméticos (+, -, *, /, %) requerem tipos numéricos (int ou float):

```spectra
let x = 10 + 5;      // ✓ Correto: int + int
let y = 3.14 * 2.0;  // ✓ Correto: float * float
let z = 10 + "hi";   // ✗ Erro: não pode somar int com string
```

### Operações de Comparação
Operadores de comparação (<, >, <=, >=) requerem tipos numéricos e retornam bool:

```spectra
let result = 10 > 5;      // ✓ Correto: retorna bool
let check = 3.14 <= 4.0;  // ✓ Correto: retorna bool
let bad = "a" < "b";      // ✗ Erro: não pode comparar strings
```

### Operações de Igualdade
Operadores de igualdade (==, !=) podem comparar qualquer tipo, mas os operandos devem ser do mesmo tipo:

```spectra
let same = 10 == 10;     // ✓ Correto: int == int
let diff = "a" != "b";   // ✓ Correto: string != string
let bad = 10 == "10";    // ✗ Erro: int != string
```

### Operações Lógicas
Operadores lógicos (&&, ||) requerem operandos booleanos:

```spectra
let result = true && false;   // ✓ Correto: bool && bool
let check = (x > 5) || (y < 3);  // ✓ Correto: bool || bool
let bad = 10 && 20;           // ✗ Erro: requer bool
```

### Chamadas de Função
Os tipos dos argumentos devem corresponder aos tipos dos parâmetros:

```spectra
fn greet(name: string, age: int) {
    // ...
}

greet("Alice", 25);    // ✓ Correto
greet("Bob", "30");    // ✗ Erro: segundo argumento deve ser int
greet(123);            // ✗ Erro: número errado de argumentos
```

## Mensagens de Erro

O compilador fornece mensagens de erro claras quando há incompatibilidade de tipos:

### Exemplo 1: Operação Aritmética Inválida
```
error: Right operand of arithmetic operation must be numeric, found String
  --> test.spectra:5:23
   |
5  |     let invalid = x + name;
   |                       ^^^^ expected numeric type, found String
```

### Exemplo 2: Argumento de Função Inválido
```
error: Argument 2 of function 'add' has type String, expected Int
  --> test.spectra:10:21
   |
10 |     let z = add(10, "hello");
   |                     ^^^^^^^ expected Int, found String
```

### Exemplo 3: Número Incorreto de Argumentos
```
error: Function 'add' expects 2 arguments, but 1 were provided
  --> test.spectra:15:13
   |
15 |     let y = add(10);
   |             ^^^^^^^ expected 2 arguments, found 1
```

## Tipos Compostos

### Arrays
Arrays de tamanho fixo ou dinâmico:

```spectra
let numbers = [1, 2, 3, 4, 5];
let first = numbers[0];  // Acesso por índice
```

### Tuplas
Coleções heterogêneas de valores:

```spectra
let person = ("Alice", 30, true);
let name = person.0;   // Acesso por índice
let age = person.1;
```

### Structs ✅
Tipos definidos pelo usuário com campos nomeados:

```spectra
struct Point {
    x: int,
    y: int,
}

let p = Point { x: 10, y: 20 };
let x_coord = p.x;
```

### Enums ✅
Tipos soma com variants:

```spectra
enum Color {
    Red,
    Green,
    Blue,
}

enum Option<T> {
    Some(T),
    None,
}

let color = Color::Red;
let maybe = Option::Some(42);
```

## Genéricos ✅

### Structs Genéricos ✅
Structs parametrizados por tipos:

```spectra
struct Point<T> {
    x: T,
    y: T,
}

// Inferência automática de tipos!
let p1 = Point { x: 10, y: 20 };        // Point<int>
let p2 = Point { x: 3.14, y: 2.71 };    // Point<float>

// Type arguments explícitos também funcionam
let p3 = Point<int> { x: 100, y: 200 };
```

### Múltiplos Parâmetros de Tipo ✅
```spectra
struct Pair<T, U> {
    first: T,
    second: U,
}

let pair = Pair { first: 42, second: "hello" };  // Pair<int, string>
```

### Enums Genéricos ✅
```spectra
enum Option<T> {
    Some(T),
    None,
}

enum Result<T, E> {
    Ok(T),
    Err(E),
}

let opt1 = Option<int>::Some(42);
let opt2 = Option<int>::None;
let res = Result<int, string>::Ok(100);
```

### Funções Genéricas ✅
```spectra
fn identity<T>(value: T) -> T {
    return value;
}

fn first<T>(a: T, b: T) -> T {
    return a;
}
```

### Trait Bounds ✅
Restrições de tipos em funções genéricas:

```spectra
trait Clone {
    fn clone(self: Self) -> Self;
}

fn duplicate<T: Clone>(value: T) -> T {
    return value.clone();
}
```

### Monomorphization ✅
O compilador gera código especializado para cada tipo concreto usado:

```
Point<int>   → Point_int   (especialização)
Point<float> → Point_float (especialização)
```

## Type Inference para Generics ✅

### Inferência Automática
O compilador infere type arguments de valores:

```spectra
// Não precisa especificar <int>
let p = Point { x: 10, y: 20 };  
// Compilador infere: Point<int>

let pair = Pair { first: 42, second: 1.5 };
// Compilador infere: Pair<int, float>
```

### Algoritmo de Unificação
1. Analisa tipos dos campos fornecidos
2. Mapeia parâmetros de tipo (T, U) para tipos concretos
3. Valida consistência das inferências
4. Preenche type arguments automaticamente

## Recursos Futuros

### Próximos Passos
- [ ] Inferência de tipos para enums: `Option::Some(42)` → `Option<int>`
- [ ] Inferência contextual: inferir de onde o valor é usado
- [ ] Generic methods em structs: `impl<T> Point<T> { ... }`
- [ ] Trait implementations para tipos genéricos
- [ ] Associated types em traits
- [ ] Higher-kinded types
- [ ] Conversão implícita entre tipos numéricos
- [ ] Type aliases: `type Vec<T> = Array<T>`
- [ ] Standard library types genéricos completos

### Em Planejamento
```spectra
// Generic methods
impl<T> Point<T> {
    fn new(x: T, y: T) -> Point<T> {
        return Point { x: x, y: y };
    }
}

// Trait implementations para generics
impl<T: Clone> Clone for Point<T> {
    fn clone(self: Self) -> Self {
        return Point { x: self.x.clone(), y: self.y.clone() };
    }
}

// Standard library
let vec = Vec::new();
vec.push(1);
vec.push(2);

let map = HashMap::new();
map.insert("key", 42);
```

## Testes

O sistema de tipos é validado através de testes extensivos em `tests/semantic/`:

- `type_inference.spectra`: Testa inferência básica de tipos
- `type_error.spectra`: Testa detecção de erros de tipo em operações
- `function_type_error.spectra`: Testa validação de argumentos de função
- `valid_code.spectra`: Código correto sem erros de tipo

Execute os testes com:
```bash
cargo run --bin spectra-cli tests/semantic/type_inference.spectra
```
