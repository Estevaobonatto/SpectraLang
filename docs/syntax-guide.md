# SpectraLang - Guia de Sintaxe Rápido

## Estrutura Básica

```spectra
// Declaração de módulo
module nome_do_modulo;

// Imports
import std.io;
import std.collections;

// Função pública principal
pub fn main() {
    // Seu código aqui
    return;
}
```

## Declarações de Variáveis

```spectra
// Tipo inferido
let x = 42;
let name = "SpectraLang";
let flag = true;

// Tipo explícito
let age: int = 25;
let pi: float = 3.14;
let active: bool = true;
```

## Tipos Primitivos

- `int` - Inteiro
- `float` - Ponto flutuante
- `bool` - Booleano (`true` ou `false`)
- `string` - String de texto
- `char` - Caractere único

## Operadores

### Aritméticos
```spectra
let sum = a + b;        // Adição
let diff = a - b;       // Subtração
let product = a * b;    // Multiplicação
let quotient = a / b;   // Divisão
let remainder = a % b;  // Módulo
```

### Comparação
```spectra
let equal = a == b;          // Igual
let not_equal = a != b;      // Diferente
let less = a < b;            // Menor
let greater = a > b;         // Maior
let less_eq = a <= b;        // Menor ou igual
let greater_eq = a >= b;     // Maior ou igual
```

### Lógicos
```spectra
let and = a && b;       // E lógico
let or = a || b;        // OU lógico
let not = !a;           // NÃO lógico
```

### Unários
```spectra
let negative = -x;      // Negação aritmética
let inverted = !flag;   // Negação lógica
```

## Funções

### Função Básica
```spectra
fn nome(parametro: tipo) {
    // corpo
    return;
}
```

### Função com Retorno
```spectra
fn soma(a: int, b: int) -> int {
    return a + b;
}
```

### Função Pública
```spectra
pub fn publica() {
    return;
}
```

### Parâmetros sem Tipo (inferido)
```spectra
fn flexivel(x, y) {
    return;
}
```

## Condicionais

### If/Else Simples
```spectra
if condicao {
    // código
} else {
    // código alternativo
}
```

### If/Elif/Else
```spectra
if x > 100 {
    // código
} elif x > 50 {
    // código
} else {
    // código
}
```

### If como Expressão
```spectra
let result = if x > 0 {
    // retorna algo
} else {
    // retorna algo
};
```

## Loops

### While
```spectra
while condicao {
    // código
    contador = contador + 1;
}
```

### For...In
```spectra
for item in colecao {
    // processar item
}
```

### For...Of
```spectra
for elemento of array {
    // processar elemento
}
```

### Break e Continue
```spectra
while true {
    if condicao_saida {
        break;
    }
    
    if pular_iteracao {
        continue;
    }
    
    // código normal
}
```

## Chamadas de Função

```spectra
// Sem argumentos
funcao();

// Com argumentos
resultado = funcao(arg1, arg2);

// Aninhadas
x = externa(interna(valor));
```

## Expressões Complexas

```spectra
// Precedência de operadores
let resultado = (a + b) * c - d / e;

// Expressões booleanas
let pode = (x > 0) && (y < 100) || (z == 50);

// Expressões aninhadas
let complexo = funcao(a + b, c * d);
```

## Comentários

```spectra
// Comentário de linha única

// Você pode ter múltiplos comentários
// em linhas separadas
```

## Exemplos Completos

### Exemplo 1: Função Simples
```spectra
module exemplo1;

pub fn main() {
    let resultado = adicionar(5, 3);
    return;
}

fn adicionar(a: int, b: int) -> int {
    return a + b;
}
```

### Exemplo 2: Loop e Condicional
```spectra
module exemplo2;

pub fn main() {
    let contador = 0;
    
    while contador < 10 {
        let eh_par = contador % 2 == 0;
        contador = contador + 1;
    }
    
    return;
}
```

### Exemplo 3: Função com Lógica
```spectra
module exemplo3;

fn maximo(a: int, b: int) -> int {
    let resultado = a;
    return resultado;
}

fn eh_positivo(n: int) -> bool {
    return n > 0;
}
```

## Boas Práticas

### 1. Nomenclatura
- Módulos: `snake_case`
- Funções: `snake_case`
- Variáveis: `snake_case`
- Constantes: `UPPER_CASE` (futuro)

### 2. Indentação
- Use 4 espaços (não tabs)
- Blocos sempre entre chaves `{ }`

### 3. Tipos
- Use tipos explícitos em funções públicas
- Tipos podem ser inferidos em variáveis locais

### 4. Comentários
- Comente o "porquê", não o "o quê"
- Use comentários para funções complexas

### 5. Estrutura
```spectra
module nome;

// 1. Imports primeiro
import std.io;

// 2. Funções públicas
pub fn main() {
    return;
}

// 3. Funções privadas
fn helper() {
    return;
}
```

## Métodos (Programação Orientada a Objetos)

### Definindo Métodos com `impl`

```spectra
struct Point {
    x: int,
    y: int
}

impl Point {
    // Método com self (acessa o objeto)
    fn get_x(&self) -> int {
        return 42;
    }
    
    // Método com múltiplos parâmetros
    fn add(&self, a: int, b: int) -> int {
        return a + b;
    }
}
```

### Chamando Métodos

```spectra
fn main() -> int {
    // Criar instância
    let p = Point { x: 10, y: 20 };
    
    // Chamar método
    let x = p.get_x();
    
    // Método com argumentos
    let sum = p.add(5, 3);
    
    return x;
}
```

### Características dos Métodos

- **Parâmetro `&self`**: Primeiro parâmetro especial que representa o objeto
- **Sintaxe OOP**: Use `objeto.metodo(args)` em vez de `Type_metodo(objeto, args)`
- **Validação Automática**: 
  - Verifica se o método existe para o tipo
  - Valida número de argumentos (descontando `self`)
  - Valida tipos de argumentos
- **Inferência de Tipos**: O compilador infere o tipo do objeto automaticamente

### Exemplo Completo: Calculator

```spectra
struct Calculator {
    value: int
}

impl Calculator {
    fn add(&self, x: int, y: int) -> int {
        return x + y;
    }
    
    fn multiply(&self, x: int) -> int {
        return x * 2;
    }
}

fn main() -> int {
    let calc = Calculator { value: 0 };
    
    let sum = calc.add(5, 3);       // 8
    let product = calc.multiply(7);  // 14
    
    return sum + product;  // 22
}
```

## Recursos Futuros (Em Desenvolvimento)

- ⏳ `match/case` - Pattern matching (80% completo)
- ⏳ Self field access (`self.x`)
- ⏳ Method chaining (`obj.m1().m2()`)
- ⏳ Static methods / Constructors (`Type::new()`)
- ⏳ `switch/case` - Switch statements
- ⏳ `loop` - Loop infinito
- ⏳ `do while` - Loop com condição no final
- ⏳ Traits (interfaces)
- ⏳ Generics
- ⏳ Macros

## Dicas

1. **Ponto-e-vírgula**: Necessário após expressões e statements
2. **Tipos de retorno**: Use `->` para especificar tipo de retorno
3. **Blocos**: Sempre use `{ }` para delimitar blocos
4. **Return**: Sempre termine funções com `return`
5. **Operadores**: Respeitam precedência matemática padrão

## Ajuda e Recursos

- Documentação completa: `docs/`
- Exemplos: `examples/`
- Plano de desenvolvimento: `docs/development-plan.md`
- Relatório de progresso: `docs/progress-report.md`

---

**SpectraLang** - Uma linguagem simples, mas poderosa! 🚀
