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

## Recursos Futuros

### Em Desenvolvimento
- [ ] Tipos compostos (arrays, tuplas)
- [ ] Tipos definidos pelo usuário (structs, enums)
- [ ] Genéricos
- [ ] Traits/Interfaces
- [ ] Conversão implícita entre tipos numéricos
- [ ] Nullable types / Option
- [ ] Type aliases

### Exemplos Futuros
```spectra
// Arrays
let numbers: [int] = [1, 2, 3, 4, 5];

// Structs
struct Point {
    x: int,
    y: int,
}

// Enums
enum Color {
    Red,
    Green,
    Blue,
}

// Genéricos
fn identity<T>(value: T) -> T {
    return value;
}
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
