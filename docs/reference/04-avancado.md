# SpectraLang — Avançado / Advanced

> **Nível / Level:** Avançado / Advanced  
> **Parte / Part:** 4 de 6

---

## Sumário / Table of Contents

1. [Pattern Matching — match](#1-pattern-matching--match)
2. [if let](#2-if-let)
3. [while let](#3-while-let)
4. [Option\<T\> e Result\<T, E\>](#4-optiont-e-resultt-e)
5. [Operador de Propagação de Erro `?` / Error Propagation Operator `?`](#5-operador-de-propagação-de-erro---error-propagation-operator-)
6. [Sistema de Módulos / Module System](#6-sistema-de-módulos--module-system)
7. [Visibilidade / Visibility](#7-visibilidade--visibility)
8. [F-Strings Avançadas / Advanced F-Strings](#8-f-strings-avançadas--advanced-f-strings)
9. [Retorno Implícito de Blocos / Implicit Block Return](#9-retorno-implícito-de-blocos--implicit-block-return)

---

## 1. Pattern Matching — `match`

### Conceito / Concept

**PT-BR:**  
`match` é a construção de correspondência de padrões do SpectraLang. Ela compara um valor contra uma série de padrões e executa o corpo do primeiro padrão que corresponder. O compilador **verifica exaustividade** — todos os casos possíveis devem ser cobertos, ou um padrão curinga `_` deve ser incluído.

**EN-US:**  
`match` is SpectraLang's pattern matching construct. It compares a value against a series of patterns and executes the body of the first matching pattern. The compiler **checks exhaustiveness** — all possible cases must be covered, or a wildcard `_` pattern must be included.

### Sintaxe / Syntax

```spectra
match expressão {
    padrão1 => corpo1,
    padrão2 => corpo2,
    _ => corpo_padrão    // curinga / wildcard
}
```

### Padrões Literais / Literal Patterns

```spectra
module match_demo;

import { println } from std.io;

pub fn main() {
    let x = 5;

    let resultado = match x {
        1 => "um",
        2 => "dois",
        3 => "três",
        4 => "quatro",
        5 => "cinco",
        _ => "outro"
    };

    println(resultado);    // "cinco"
}
```

### Padrões de Identificador / Identifier Patterns (Binding)

**PT-BR:**  
Um identificador em um padrão **captura** o valor e o vincula a um nome para uso no corpo do braço.

**EN-US:**  
An identifier in a pattern **captures** the value and binds it to a name for use in the arm body.

```spectra
fn descrever(n: int) -> string {
    match n {
        0 => "zero",
        x => f"o número {x}"    // x captura o valor / x captures the value
    }
}
```

### Padrões de Variante de Enum / Enum Variant Patterns

#### Variantes Unitárias / Unit Variants

```spectra
enum Cor { Vermelho, Verde, Azul }

fn nome_da_cor(c: Cor) -> string {
    match c {
        Cor::Vermelho => "Vermelho",
        Cor::Verde    => "Verde",
        Cor::Azul     => "Azul"
    }
}
```

#### Variantes com Dados Tuple / Tuple Variant Patterns

```spectra
enum Mensagem {
    Sair,
    Mover(int, int),
    Texto(string)
}

fn processar(msg: Mensagem) {
    match msg {
        Mensagem::Sair          => println("Saindo..."),
        Mensagem::Mover(x, y)   => println(f"Movendo para ({x}, {y})"),
        Mensagem::Texto(t)      => println(f"Mensagem: {t}")
    }
}
```

#### Variantes com Campos Nomeados / Struct-Style Variant Patterns

```spectra
enum Forma {
    Circulo { raio: float },
    Retangulo { largura: float, altura: float }
}

fn calcular_area(f: Forma) -> float {
    match f {
        Forma::Circulo { raio }              => raio * raio * 3.14159,
        Forma::Retangulo { largura, altura } => largura * altura
    }
}
```

#### Renomeação de Campos / Field Renaming in Patterns

```spectra
enum Ponto3D {
    Cartesiano { x: float, y: float, z: float }
}

match ponto {
    Ponto3D::Cartesiano { x: a, y: b, z: c } => {
        // a, b, c são os valores de x, y, z
        println(f"({a}, {b}, {c})")
    }
}
```

### Padrão Curinga / Wildcard Pattern `_`

**PT-BR:**  
`_` captura qualquer valor e o descarta. É obrigatório quando não são listados todos os casos.

**EN-US:**  
`_` captures any value and discards it. It is required when not all cases are listed.

```spectra
let c = Cor::Azul;

match c {
    Cor::Vermelho => println("Vermelho!"),
    _ => println("Não é vermelho.")    // captura Verde e Azul / captures Green and Blue
}
```

### Corpo de Bloco / Block Body

```spectra
fn processar_forma(f: Forma) -> float {
    match f {
        Forma::Circulo { raio } => {
            let area = raio * raio * 3.14159;
            println(f"Círculo com raio {raio}");
            area    // retorno implícito do bloco / implicit block return
        }
        Forma::Retangulo { largura, altura } => {
            let area = largura * altura;
            println(f"Retângulo {largura}x{altura}");
            area
        }
    }
}
```

### `match` como Expression vs Statement

```spectra
// Como expressão: retorna valor / As expression: returns value
let descricao = match x {
    0 => "zero",
    _ => "não-zero"
};

// Como statement: apenas efeitos colaterais / As statement: only side effects
match x {
    0 => println("zero"),
    _ => println("não-zero")
}
```

### Exaustividade / Exhaustiveness

**PT-BR:**  
O compilador verifica se todos os casos de um enum são cobertos. Se um caso estiver faltando, um erro de compilação é emitido. Use `_` para cobrir os casos restantes.

**EN-US:**  
The compiler checks whether all cases of an enum are covered. If a case is missing, a compilation error is emitted. Use `_` to cover remaining cases.

```spectra
enum Status { Ativo, Inativo, Pendente }

// ERRO: faltando Pendente / ERROR: missing Pendente
// match s {
//     Status::Ativo   => ...,
//     Status::Inativo => ...
// }

// OK: todos os casos / OK: all cases
match s {
    Status::Ativo    => "ativo",
    Status::Inativo  => "inativo",
    Status::Pendente => "pendente"
}

// OK: com curinga / OK: with wildcard
match s {
    Status::Ativo => "ativo",
    _ => "não ativo"
}
```

---

## 2. if let

**PT-BR:**  
`if let` é um atalho para aplicar um único padrão de correspondência. É especialmente útil para desestruturar `Option` e `Result` sem escrever um `match` completo.

**EN-US:**  
`if let` is a shortcut for applying a single pattern match. It is especially useful for destructuring `Option` and `Result` without writing a full `match`.

### Sintaxe / Syntax

```spectra
if let Padrão = expressão {
    // executado quando o padrão corresponde / executed when pattern matches
} else {
    // executado quando não corresponde / executed when it doesn't match
}
```

### Exemplos com Option / Examples with Option

```spectra
module if_let_demo;

import { println } from std.io;

pub fn main() {
    let talvez: Option<int> = Option::Some(42);
    let nada: Option<int> = Option::None;

    // Desestruturando Some / Destructuring Some
    if let Option::Some(valor) = talvez {
        println(f"Tenho um valor: {valor}");    // "Tenho um valor: 42"
    } else {
        println("Nenhum valor");
    }

    // Se for None, cai no else / If None, falls to else
    if let Option::Some(v) = nada {
        println(f"Valor: {v}");
    } else {
        println("Sem valor");    // Imprime isso / Prints this
    }
}
```

### Exemplos com Result / Examples with Result

```spectra
let resultado: Result<int, string> = Result::Ok(100);
let erro: Result<int, string> = Result::Err("não encontrado");

if let Result::Ok(valor) = resultado {
    println(f"Sucesso: {valor}");    // "Sucesso: 100"
}

if let Result::Err(msg) = erro {
    println(f"Erro: {msg}");        // "Erro: não encontrado"
}
```

### Exemplos com Enums Customizados / Custom Enum Examples

```spectra
enum Forma {
    Circulo { raio: float },
    Retangulo { largura: float, altura: float },
    Ponto
}

fn processar(f: Forma) {
    if let Forma::Circulo { raio } = f {
        println(f"É um círculo com raio {raio}");
    } else {
        println("Não é um círculo");
    }
}
```

### if let Encadeados / Chained if let

```spectra
fn obter_nome_usuario(id: int) -> Option<string> {
    if id == 1 {
        return Option::Some("Alice");
    }
    return Option::None;
}

fn obter_email(nome: string) -> Option<string> {
    if nome == "Alice" {
        return Option::Some("alice@exemplo.com");
    }
    return Option::None;
}

pub fn main() {
    let id = 1;

    if let Option::Some(nome) = obter_nome_usuario(id) {
        if let Option::Some(email) = obter_email(nome) {
            println(f"Email do usuário: {email}");
        } else {
            println("Usuário sem email");
        }
    } else {
        println("Usuário não encontrado");
    }
}
```

---

## 3. while let

**PT-BR:**  
`while let` combina um padrão com um loop: continua executando enquanto o padrão corresponder. Quando o padrão não corresponder mais, o loop termina.

**EN-US:**  
`while let` combines a pattern with a loop: it keeps executing as long as the pattern matches. When the pattern no longer matches, the loop terminates.

```spectra
module while_let_demo;

import { println } from std.io;

fn encontrar_positivo(n: int) -> Option<int> {
    if n > 0 {
        return Option::Some(n);
    }
    return Option::None;
}

pub fn main() {
    let contador = 5;

    while let Option::Some(n) = encontrar_positivo(contador) {
        println(f"Valor positivo: {n}");
        contador = contador - 1;
    }
    // Imprime 5, 4, 3, 2, 1 e para quando contador == 0

    // Processando itens de uma fila / Processing queue items
    let itens = [10, 20, 30];
    let idx = 0;

    while let Option::Some(item) = obter_item(itens, idx, 3) {
        println(f"Processando: {item}");
        idx = idx + 1;
    }
}

fn obter_item(arr: [int], idx: int, tamanho: int) -> Option<int> {
    if idx < tamanho {
        return Option::Some(arr[idx]);
    }
    return Option::None;
}
```

---

## 4. Option\<T\> e Result\<T, E\>

**PT-BR:**  
`Option<T>` e `Result<T, E>` são os tipos centrais para lidar com ausência de valores e erros de forma segura, sem exceções.

**EN-US:**  
`Option<T>` and `Result<T, E>` are the central types for handling absence of values and errors safely, without exceptions.

### Option\<T\>

**PT-BR:**  
`Option<T>` representa um valor que pode ou não existir:
- `Option::Some(valor)` — há um valor
- `Option::None` — não há valor

**EN-US:**  
`Option<T>` represents a value that may or may not exist:
- `Option::Some(value)` — there is a value
- `Option::None` — there is no value

```spectra
fn dividir_seguro(a: int, b: int) -> Option<int> {
    if b == 0 {
        return Option::None;
    }
    return Option::Some(a / b);
}

fn obter_primeiro(arr: [int], n: int) -> Option<int> {
    if n == 0 {
        return Option::None;
    }
    return Option::Some(arr[0]);
}

pub fn main() {
    // Usando match / Using match
    let resultado = dividir_seguro(10, 2);
    match resultado {
        Option::Some(v) => println(f"Resultado: {v}"),  // "Resultado: 5"
        Option::None    => println("Divisão por zero!")
    }

    // Usando if let / Using if let
    if let Option::Some(n) = dividir_seguro(10, 0) {
        println(f"Valor: {n}");
    } else {
        println("Sem resultado");    // Imprime isso
    }

    // Funções da stdlib para Option / Stdlib functions for Option
    import std.option;

    let opt = Option::Some(42);
    let tem = std.option.is_some(opt);      // true
    let val = std.option.option_unwrap(opt);// 42 (panic se None)
    let ou  = std.option.option_unwrap_or(Option::None, 0);  // 0 (padrão)
}
```

### Result\<T, E\>

**PT-BR:**  
`Result<T, E>` representa uma operação que pode ter sucesso ou falhar:
- `Result::Ok(valor)` — operação bem-sucedida com um valor
- `Result::Err(erro)` — operação falhou com informação de erro

**EN-US:**  
`Result<T, E>` represents an operation that can succeed or fail:
- `Result::Ok(value)` — operation succeeded with a value
- `Result::Err(error)` — operation failed with error information

```spectra
import std.convert;

fn analisar_inteiro(s: string) -> Result<int, string> {
    let n = std.convert.string_to_int(s);
    if n == 0 && s != "0" {
        return Result::Err(f"'{s}' não é um inteiro válido");
    }
    return Result::Ok(n);
}

fn dividir(a: int, b: int) -> Result<int, string> {
    if b == 0 {
        return Result::Err("divisão por zero");
    }
    return Result::Ok(a / b);
}

pub fn main() {
    // Correspondência exaustiva / Exhaustive matching
    match analisar_inteiro("42") {
        Result::Ok(n)    => println(f"Sucesso: {n}"),
        Result::Err(msg) => println(f"Erro: {msg}")
    }

    match analisar_inteiro("abc") {
        Result::Ok(n)    => println(f"Sucesso: {n}"),
        Result::Err(msg) => println(f"Erro: {msg}")    // Imprime isso
    }

    // Encadeamento de operações / Chaining operations
    if let Result::Ok(n) = analisar_inteiro("10") {
        if let Result::Ok(resultado) = dividir(n, 2) {
            println(f"10 / 2 = {resultado}");    // 5
        }
    }

    // Funções da stdlib / Stdlib functions
    import std.result;

    let r = Result::Ok(100);
    let ok     = std.result.is_ok(r);               // true
    let val    = std.result.result_unwrap(r);        // 100
    let padrao = std.result.result_unwrap_or(Result::Err("e"), 0); // 0
}
```

### Convertendo entre Option e Result

```spectra
// Option → Result
fn opcao_para_resultado(opt: Option<int>, msg_erro: string) -> Result<int, string> {
    match opt {
        Option::Some(v) => Result::Ok(v),
        Option::None    => Result::Err(msg_erro)
    }
}

// Result → Option (descartando o erro)
fn resultado_para_opcao(res: Result<int, string>) -> Option<int> {
    match res {
        Result::Ok(v)  => Option::Some(v),
        Result::Err(_) => Option::None
    }
}
```

---

## 5. Operador de Propagação de Erro `?` / Error Propagation Operator `?`

**PT-BR:**  
O operador `?` é uma forma concisa de propagar erros. Quando aplicado a um `Result` ou `Option`, ele desembrulha o valor se for `Ok`/`Some`, ou retorna antecipadamente da função com o `Err`/`None` se for o caso.

**EN-US:**  
The `?` operator is a concise way to propagate errors. When applied to a `Result` or `Option`, it unwraps the value if `Ok`/`Some`, or early-returns from the function with the `Err`/`None` otherwise.

```spectra
fn processar_entrada(entrada: string) -> Result<int, string> {
    // Sem o operador ? / Without the ? operator
    let r = analisar_inteiro(entrada);
    let n = match r {
        Result::Ok(v)    => v,
        Result::Err(msg) => return Result::Err(msg)    // Propagação manual
    };
    return Result::Ok(n * 2);
}

fn processar_entrada_conciso(entrada: string) -> Result<int, string> {
    // Com o operador ? / With the ? operator
    let n = analisar_inteiro(entrada)?;    // Propaga o erro automaticamente
    return Result::Ok(n * 2);
}
```

> **Nota / Note:** A função que usa `?` deve ter retorno compatível com o tipo sendo propagado (`Result<T, E>` → `Result<U, E>`).

```spectra
// Encadeamento elegante com ? / Elegant chaining with ?
fn calcular_pipeline(a: string, b: string) -> Result<int, string> {
    let x = analisar_inteiro(a)?;
    let y = analisar_inteiro(b)?;
    let resultado = dividir(x, y)?;
    return Result::Ok(resultado);
}

pub fn main() {
    match calcular_pipeline("10", "2") {
        Result::Ok(v)  => println(f"Resultado: {v}"),   // "Resultado: 5"
        Result::Err(e) => println(f"Erro: {e}")
    }

    match calcular_pipeline("10", "0") {
        Result::Ok(v)  => println(f"Resultado: {v}"),
        Result::Err(e) => println(f"Erro: {e}")          // "Erro: divisão por zero"
    }
}
```

---

## 6. Sistema de Módulos / Module System

### Declaração de Módulo / Module Declaration

**PT-BR:**  
Todo arquivo SpectraLang começa com uma declaração `module`. O caminho do módulo usa pontos como separadores.

**EN-US:**  
Every SpectraLang file starts with a `module` declaration. The module path uses dots as separators.

```spectra
module app;
module app.utils;
module app.controladores.usuarios;
module biblioteca.colecoes;
```

### Importações / Imports

**PT-BR:**  
Existem três formas de importar módulos:

**EN-US:**  
There are three ways to import modules:

#### 1. Importação Qualificada / Qualified Import

```spectra
// Importar o módulo completo e usar com prefixo / Import whole module and use with prefix
import std.io;
import std.math as math;

pub fn main() {
    std.io.println("Olá");            // prefixo completo / full prefix
    let raiz = math.sqrt_f(16.0);    // prefixo de alias / alias prefix
}
```

#### 2. Importação por Nome / Named Import

```spectra
// Importar nomes específicos diretamente / Import specific names directly
import { println, print } from std.io;
import { abs, sqrt_f as sqrt } from std.math;

pub fn main() {
    println("Sem prefixo!");    // sem qualificação / no qualification
    let r = sqrt(25.0);         // usando alias local / using local alias
}
```

#### 3. Re-exportação Pública / Public Re-export

```spectra
// Tornar uma importação pública (re-exportar) / Make an import public (re-export)
pub import { println } from std.io;

// Outros módulos podem importar println deste módulo / Other modules can import println from this module
```

### Restrições do Sistema de Módulos / Module System Restrictions

**PT-BR:**  
Na versão alpha, as seguintes limitações se aplicam:
- Cada arquivo é compilado **independentemente**
- Importações são **reconhecidas estruturalmente** mas arquivos externos não são carregados automaticamente
- Não há **detecção de módulos duplicados** dentro da mesma sessão de compilação
- **Módulos stdlib** (`std.*`) são resolvidos internamente pelo compilador

**EN-US:**  
In the alpha version, the following limitations apply:
- Each file is compiled **independently**
- Imports are **structurally recognized** but external files are not loaded automatically
- There is no **duplicate module detection** within the same compilation session
- **Stdlib modules** (`std.*`) are resolved internally by the compiler

### Módulos Stdlib Disponíveis / Available Stdlib Modules

| Módulo / Module | Conteúdo PT-BR | Content EN-US |
|---|---|---|
| `std.io` | Entrada e saída | Input/Output |
| `std.string` | Manipulação de strings | String manipulation |
| `std.math` | Funções matemáticas | Mathematical functions |
| `std.convert` | Conversão de tipos | Type conversion |
| `std.collections` | Listas e coleções | Lists and collections |
| `std.random` | Números aleatórios | Random numbers |
| `std.fs` | Sistema de arquivos | File system |
| `std.env` | Variáveis de ambiente | Environment variables |
| `std.option` | Operações em Option | Option operations |
| `std.result` | Operações em Result | Result operations |
| `std.char` | Operações em caracteres | Character operations |
| `std.time` | Operações de tempo | Time operations |

---

## 7. Visibilidade / Visibility

**PT-BR:**  
SpectraLang tem três níveis de visibilidade:

**EN-US:**  
SpectraLang has three visibility levels:

| Modificador / Modifier | Escopo PT-BR | Scope EN-US |
|---|---|---|
| (padrão / default) | Privado — apenas no módulo | Private — current module only |
| `pub` | Público — acessível de outros módulos | Public — accessible from other modules |
| `internal` | Interno — acessível dentro do pacote | Internal — accessible within the package |

```spectra
module minha.biblioteca;

// Pública — qualquer módulo pode usar / Public — any module can use
pub struct Ponto {
    pub x: int,    // campo público / public field
    pub y: int
}

// Interna — apenas no pacote / Internal — package only
internal fn utilitario_interno() -> int {
    return 42;
}

// Privada — apenas neste módulo / Private — this module only
fn helper() -> int {
    return utilitario_interno();
}

// Pública com impl público / Public with public impl
pub impl Ponto {
    pub fn novo(x: int, y: int) -> Ponto {
        Ponto { x: x, y: y }
    }
}
```

### Regras de Visibilidade / Visibility Rules

**PT-BR:**  
- Funções públicas **não podem** expor tipos privados nas assinaturas
- Structs públicas **não podem** expor tipos privados nos campos
- Enums públicos **não podem** expor tipos privados nas variantes
- Parâmetros genéricos e tipos built-in (`int`, `float`, etc.) são exceções

**EN-US:**  
- Public functions **cannot** expose private types in their signatures
- Public structs **cannot** expose private types in fields
- Public enums **cannot** expose private types in variants
- Generic type parameters and built-in types (`int`, `float`, etc.) are exceptions

```spectra
// Tipo privado / Private type
struct Interno {
    dados: int
}

// ERRO: função pública expõe tipo privado / ERROR: public function exposes private type
// pub fn obter_interno() -> Interno { ... }

// OK: usa tipo built-in / OK: uses builtin type
pub fn calcular() -> int { return 42; }

// OK: tipo público / OK: public type
pub struct Publico {
    valor: int
}
pub fn obter_publico() -> Publico { Publico { valor: 1 } }
```

---

## 8. F-Strings Avançadas / Advanced F-Strings

**PT-BR:**  
F-strings (`f"..."`) suportam expressões arbitrárias dentro de `{...}`. Praticamente qualquer expressão válida em SpectraLang pode ser interpolada.

**EN-US:**  
F-strings (`f"..."`) support arbitrary expressions inside `{...}`. Virtually any valid SpectraLang expression can be interpolated.

```spectra
module fstrings_avancado;

import { println } from std.io;

pub fn main() {
    let nome = "Alice";
    let pontos = 850;
    let nivel = 5;

    // Variáveis simples / Simple variables
    println(f"Jogador: {nome}");

    // Expressões aritméticas / Arithmetic expressions
    println(f"Próximo nível: {nivel + 1}");
    println(f"Pontos necessários: {(nivel + 1) * 200 - pontos}");

    // Chamadas de função / Function calls
    println(f"Dobro dos pontos: {dobrar(pontos)}");

    // Acesso a campos / Field access
    let p = Ponto { x: 10, y: 20 };
    println(f"Posição: ({p.x}, {p.y})");

    // Comparações / Comparisons
    println(f"Nível máximo: {nivel == 10}");

    // Literais / Literals
    println(f"Total: {pontos + 150}");

    // Expressões complexas / Complex expressions
    let media = (80 + 90 + 75) / 3;
    println(f"Média: {media}");

    // Interpolação dentro de interpolação (cuidado com clareza)
    // Interpolation within interpolation (be careful with clarity)
    let lista = [1, 2, 3];
    println(f"Primeiro: {lista[0]}, Último: {lista[2]}");
}

fn dobrar(x: int) -> int { x * 2 }
struct Ponto { x: int, y: int }
```

### Casos Especiais / Special Cases

```spectra
// Chaves literais — não suportadas diretamente / Literal braces — not directly supported
// Use uma variável intermediária / Use an intermediate variable
let chave_esq = "{";
let chave_dir = "}";
println(f"Objeto JS: {chave_esq}valor{chave_dir}");

// Strings vazias / Empty strings
let vazio = "";
println(f"Campo: '{vazio}'");    // Campo: ''

// Valores de retorno void (unit) não devem ser interpolados
// Void (unit) return values should not be interpolated
```

---

## 9. Retorno Implícito de Blocos / Implicit Block Return

**PT-BR:**  
Em SpectraLang, **blocos são expressões**. O valor de um bloco é a última expressão que ele contém (sem ponto e vírgula). Isso vale para funções, if/else, match, e blocos anônimos.

**EN-US:**  
In SpectraLang, **blocks are expressions**. The value of a block is its last expression (without a semicolon). This applies to functions, if/else, match, and anonymous blocks.

```spectra
module blocos;

pub fn main() {
    // Bloco como expressão / Block as expression
    let resultado = {
        let a = 10;
        let b = 20;
        a + b    // Retorno implícito do bloco: 30 / Implicit block return: 30
    };
    // resultado == 30

    // if como expressão / if as expression
    let max = if 10 > 5 { 10 } else { 5 };    // max == 10

    // match como expressão / match as expression
    let descricao = match resultado {
        0..=10  => "baixo",
        11..=50 => "médio",
        _       => "alto"
    };
    // descricao == "médio"

    // Função com retorno implícito / Function with implicit return
    println(calcular(5, 3));    // 8
}

// Retorno implícito na última linha / Implicit return on last line
fn calcular(a: int, b: int) -> int {
    let soma = a + b;
    soma    // Retorna soma / Returns soma
}

// Retorno implícito em ramo if / Implicit return in if branch
fn classificar(n: int) -> string {
    if n < 0 {
        "negativo"
    } else if n == 0 {
        "zero"
    } else {
        "positivo"
    }
}
```

### Regras Importantes / Important Rules

**PT-BR:**  
- Expressão **sem** `;` ao final do bloco → retorno implícito
- Expressão **com** `;` ao final do bloco → descartada, bloco retorna `unit`
- `return expr;` funciona em qualquer ponto da função (inclui retorno antecipado)

**EN-US:**  
- Expression **without** `;` at block end → implicit return
- Expression **with** `;` at block end → discarded, block returns `unit`
- `return expr;` works anywhere in a function (includes early return)

```spectra
fn exemplo1() -> int {
    42          // Retorna 42 / Returns 42
}

fn exemplo2() -> int {
    42;         // 42 descartado, retorna unit — ERRO DE TIPO / 42 discarded, returns unit — TYPE ERROR
    // O compilador lançará um erro pois a assinatura diz -> int mas o bloco retorna unit
}

fn exemplo3() -> int {
    let x = 42;
    return x;   // Retorno explícito / Explicit return
    // O código após return nunca será executado / Code after return is never reached
}
```

---

> **Próximo / Next:** [05 — Biblioteca Padrão / Standard Library](05-stdlib.md)  
> **Anterior / Previous:** [03 — Tipos Compostos / Composite Types](03-tipos-compostos.md)
