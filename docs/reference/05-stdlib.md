# SpectraLang — Biblioteca Padrão / Standard Library

> **Nível / Level:** Intermediário–Avançado / Intermediate–Advanced  
> **Parte / Part:** 5 de 6

---

**PT-BR:**  
A Biblioteca Padrão (stdlib) do SpectraLang é implementada como funções hospedadas (*host functions*) que são registradas pelo runtime e chamadas pelo código JIT via FFI. Existem **12 módulos** com mais de **100 funções**.

**EN-US:**  
SpectraLang's Standard Library (stdlib) is implemented as host functions registered by the runtime and called from JIT code via FFI. There are **12 modules** with over **100 functions**.

---

## Sumário / Table of Contents

1. [std.io — Entrada e Saída / Input & Output](#1-stdio--entrada-e-saída--input--output)
2. [std.string — Manipulação de Strings / String Manipulation](#2-stdstring--manipulação-de-strings--string-manipulation)
3. [std.math — Matemática / Mathematics](#3-stdmath--matemática--mathematics)
4. [std.convert — Conversão de Tipos / Type Conversion](#4-stdconvert--conversão-de-tipos--type-conversion)
5. [std.collections — Coleções / Collections](#5-stdcollections--coleções--collections)
6. [std.random — Números Aleatórios / Random Numbers](#6-stdrandom--números-aleatórios--random-numbers)
7. [std.fs — Sistema de Arquivos / File System](#7-stdfs--sistema-de-arquivos--file-system)
8. [std.env — Ambiente / Environment](#8-stdenv--ambiente--environment)
9. [std.option — Operações em Option / Option Operations](#9-stdoption--operações-em-option--option-operations)
10. [std.result — Operações em Result / Result Operations](#10-stdresult--operações-em-result--result-operations)
11. [std.char — Operações em Caracteres / Character Operations](#11-stdchar--operações-em-caracteres--character-operations)
12. [std.time — Tempo / Time](#12-stdtime--tempo--time)

---

## 1. std.io — Entrada e Saída / Input & Output

**PT-BR:**  
Módulo para entrada e saída de texto. Este é o módulo mais frequentemente importado.

**EN-US:**  
Module for text input and output. This is the most frequently imported module.

```spectra
import std.io;
// ou / or
import { println, print, read_line } from std.io;
```

### Funções / Functions

#### `println(value: any) -> unit`

**PT-BR:** Imprime um valor seguido de uma nova linha na saída padrão.  
**EN-US:** Prints a value followed by a newline to standard output.

```spectra
std.io.println("Olá, mundo!");       // Olá, mundo!\n
std.io.println(42);                  // 42\n
std.io.println(3.14);                // 3.14\n
std.io.println(true);                // true\n
std.io.println(f"Valor: {100}");     // Valor: 100\n
```

#### `print(value: any) -> unit`

**PT-BR:** Imprime um valor **sem** nova linha.  
**EN-US:** Prints a value **without** a newline.

```spectra
std.io.print("Olá, ");
std.io.print("mundo");
std.io.println("!");    // Olá, mundo!
```

#### `eprint(value: any) -> unit`

**PT-BR:** Imprime na saída de erro padrão (stderr) sem nova linha.  
**EN-US:** Prints to standard error (stderr) without a newline.

```spectra
std.io.eprint("Aviso: ");
std.io.eprintln("arquivo não encontrado");
```

#### `eprintln(value: any) -> unit`

**PT-BR:** Imprime na saída de erro padrão com nova linha.  
**EN-US:** Prints to standard error with a newline.

```spectra
std.io.eprintln("Erro fatal: divisão por zero");
```

#### `flush() -> unit`

**PT-BR:** Esvazia o buffer de saída padrão.  
**EN-US:** Flushes the standard output buffer.

```spectra
std.io.print("Carregando...");
std.io.flush();    // Garante que o texto apareça antes de operação demorada
```

#### `read_line() -> string`

**PT-BR:** Lê uma linha da entrada padrão (aguarda Enter).  
**EN-US:** Reads a line from standard input (waits for Enter).

```spectra
std.io.print("Digite seu nome: ");
let nome = std.io.read_line();
std.io.println(f"Olá, {nome}!");
```

#### `input(prompt: string) -> string`

**PT-BR:** Exibe um prompt e lê uma linha de entrada.  
**EN-US:** Displays a prompt and reads a line of input.

```spectra
let nome = std.io.input("Digite seu nome: ");
let idade_str = std.io.input("Digite sua idade: ");
```

---

## 2. std.string — Manipulação de Strings / String Manipulation

```spectra
import std.string;
// ou / or
import { len, trim, contains } from std.string;
```

### Funções / Functions

#### `len(s: string) -> int`

**PT-BR:** Retorna o número de bytes da string (não necessariamente caracteres Unicode).  
**EN-US:** Returns the number of bytes in the string (not necessarily Unicode characters).

```spectra
let n = std.string.len("hello");        // 5
let n2 = std.string.len("");            // 0
let n3 = std.string.len("olá");         // pode variar com Unicode
```

#### `contains(s: string, sub: string) -> bool`

**PT-BR:** Verifica se a string contém a substring.  
**EN-US:** Checks whether the string contains the substring.

```spectra
let tem = std.string.contains("hello world", "world");  // true
let nao = std.string.contains("hello", "xyz");          // false
```

#### `to_upper(s: string) -> string`

**PT-BR:** Converte todos os caracteres ASCII para maiúsculo.  
**EN-US:** Converts all ASCII characters to uppercase.

```spectra
let upper = std.string.to_upper("hello");   // "HELLO"
let mixed = std.string.to_upper("Hello!");  // "HELLO!"
```

#### `to_lower(s: string) -> string`

**PT-BR:** Converte todos os caracteres ASCII para minúsculo.  
**EN-US:** Converts all ASCII characters to lowercase.

```spectra
let lower = std.string.to_lower("WORLD");   // "world"
```

#### `trim(s: string) -> string`

**PT-BR:** Remove espaços em branco do início e fim da string.  
**EN-US:** Removes whitespace from the beginning and end of the string.

```spectra
let limpa = std.string.trim("  hello  ");   // "hello"
let s2 = std.string.trim("\t texto \n");    // "texto"
```

#### `starts_with(s: string, prefix: string) -> bool`

```spectra
let sw = std.string.starts_with("hello world", "hello");  // true
let nao = std.string.starts_with("world", "hello");       // false
```

#### `ends_with(s: string, suffix: string) -> bool`

```spectra
let ew = std.string.ends_with("hello.spectra", ".spectra");  // true
```

#### `concat(a: string, b: string) -> string`

**PT-BR:** Concatena duas strings.  
**EN-US:** Concatenates two strings.

```spectra
let ab = std.string.concat("foo", "bar");    // "foobar"
// Nota: o operador + também concatena strings / Note: the + operator also concatenates strings
let ab2 = "foo" + "bar";    // "foobar"
```

#### `repeat_str(s: string, n: int) -> string`

**PT-BR:** Repete a string `n` vezes.  
**EN-US:** Repeats the string `n` times.

```spectra
let rep = std.string.repeat_str("ab", 3);    // "ababab"
let linha = std.string.repeat_str("-", 40);  // "----------------------------------------"
```

#### `char_at(s: string, index: int) -> int`

**PT-BR:** Retorna o código Unicode do caractere na posição `index`. Retorna `-1` se o índice estiver fora dos limites.  
**EN-US:** Returns the Unicode code point of the character at position `index`. Returns `-1` if the index is out of bounds.

```spectra
let c = std.string.char_at("hello", 0);     // 104 ('h')
let e = std.string.char_at("hello", 1);     // 101 ('e')
let oob = std.string.char_at("hi", 10);     // -1
```

#### `substring(s: string, start: int, end: int) -> string`

**PT-BR:** Extrai a substring de `start` até `end` (exclusivo).  
**EN-US:** Extracts substring from `start` to `end` (exclusive).

```spectra
let sub = std.string.substring("hello world", 0, 5);    // "hello"
let sub2 = std.string.substring("hello world", 6, 11);  // "world"
```

#### `replace(s: string, from: string, to: string) -> string`

**PT-BR:** Substitui todas as ocorrências de `from` por `to`.  
**EN-US:** Replaces all occurrences of `from` with `to`.

```spectra
let r = std.string.replace("hello world", "world", "SpectraLang");
// "hello SpectraLang"
```

#### `index_of(s: string, sub: string) -> int`

**PT-BR:** Retorna a posição (índice 0) da primeira ocorrência de `sub`, ou `-1` se não encontrada.  
**EN-US:** Returns the position (0-index) of the first occurrence of `sub`, or `-1` if not found.

```spectra
let pos = std.string.index_of("hello world", "world");  // 6
let nao = std.string.index_of("hello", "xyz");          // -1
```

#### `split_first(s: string, sep: string) -> string`

**PT-BR:** Retorna a parte antes do primeiro separador.  
**EN-US:** Returns the part before the first separator.

```spectra
let parte = std.string.split_first("nome:Alice:30", ":");  // "nome"
```

#### `split_last(s: string, sep: string) -> string`

**PT-BR:** Retorna a parte após o último separador.  
**EN-US:** Returns the part after the last separator.

```spectra
let ultima = std.string.split_last("nome:Alice:30", ":");  // "30"
```

#### `count_occurrences(s: string, sub: string) -> int`

```spectra
let count = std.string.count_occurrences("banana", "a");  // 3
```

#### `is_empty(s: string) -> bool`

```spectra
let vazio = std.string.is_empty("");        // true
let nao   = std.string.is_empty("hello");   // false
```

#### `pad_left(s: string, width: int, pad_char: int) -> string`

**PT-BR:** Preenche a string à esquerda com o caractere especificado até atingir `width`.  
**EN-US:** Left-pads the string with the specified character until reaching `width`.

```spectra
// pad_char é o código Unicode do caractere / pad_char is the Unicode code point
let padded = std.string.pad_left("42", 5, 48);   // "   42" (48 = '0')
// Nota: 48 é o código de '0', 32 é espaço / Note: 48 is code for '0', 32 is space
```

#### `pad_right(s: string, width: int, pad_char: int) -> string`

```spectra
let padded = std.string.pad_right("hello", 8, 32);  // "hello   " (32 = espaço/space)
```

#### `reverse_str(s: string) -> string`

```spectra
let rev = std.string.reverse_str("hello");  // "olleh"
```

---

## 3. std.math — Matemática / Mathematics

```spectra
import std.math;
// ou / or
import std.math as math;
```

### Funções Inteiras / Integer Functions

#### `abs(x: int) -> int`

```spectra
let v = std.math.abs(-42);     // 42
let v2 = std.math.abs(10);     // 10
```

#### `min(lhs: int, rhs: int) -> int`

```spectra
let menor = std.math.min(3, 7);    // 3
```

#### `max(lhs: int, rhs: int) -> int`

```spectra
let maior = std.math.max(3, 7);    // 7
```

#### `clamp(n: int, min: int, max: int) -> int`

**PT-BR:** Restringe `n` ao intervalo `[min, max]`.  
**EN-US:** Restricts `n` to the range `[min, max]`.

```spectra
let v = std.math.clamp(150, 0, 100);   // 100
let v2 = std.math.clamp(-5, 0, 100);   // 0
let v3 = std.math.clamp(50, 0, 100);   // 50
```

#### `sign(n: int) -> int`

**PT-BR:** Retorna `-1`, `0` ou `1`.  
**EN-US:** Returns `-1`, `0`, or `1`.

```spectra
let s1 = std.math.sign(-5);   // -1
let s2 = std.math.sign(0);    // 0
let s3 = std.math.sign(10);   // 1
```

#### `gcd(a: int, b: int) -> int`

**PT-BR:** Máximo divisor comum.  
**EN-US:** Greatest common divisor.

```spectra
let g = std.math.gcd(12, 8);    // 4
```

#### `lcm(a: int, b: int) -> int`

**PT-BR:** Mínimo múltiplo comum.  
**EN-US:** Least common multiple.

```spectra
let l = std.math.lcm(4, 6);    // 12
```

### Funções de Ponto Flutuante / Float Functions

#### `abs_f(x: float) -> float`

```spectra
let v = std.math.abs_f(-3.14);   // 3.14
```

#### `sqrt_f(x: float) -> float`

```spectra
let r = std.math.sqrt_f(16.0);   // 4.0
let r2 = std.math.sqrt_f(2.0);   // ~1.4142
```

#### `pow_f(base: float, exp: float) -> float`

```spectra
let p = std.math.pow_f(2.0, 10.0);   // 1024.0
let p2 = std.math.pow_f(3.0, 0.5);   // ~1.732 (raiz / sqrt)
```

#### `floor_f(x: float) -> float`

```spectra
let f = std.math.floor_f(3.7);    // 3.0
let f2 = std.math.floor_f(-1.2);  // -2.0
```

#### `ceil_f(x: float) -> float`

```spectra
let c = std.math.ceil_f(3.2);     // 4.0
let c2 = std.math.ceil_f(-1.8);   // -1.0
```

#### `round_f(x: float) -> float`

```spectra
let r = std.math.round_f(3.5);    // 4.0
let r2 = std.math.round_f(3.4);   // 3.0
```

#### `sin_f(x: float) -> float` / `cos_f(x: float) -> float` / `tan_f(x: float) -> float`

**PT-BR:** Funções trigonométricas. `x` em radianos.  
**EN-US:** Trigonometric functions. `x` in radians.

```spectra
import std.math as m;

let pi = m.pi();
let seno  = m.sin_f(pi / 2.0);    // ~1.0
let coss  = m.cos_f(0.0);          // 1.0
let tang  = m.tan_f(pi / 4.0);    // ~1.0
```

#### `log_f(x: float) -> float`

**PT-BR:** Logaritmo natural (base e).  
**EN-US:** Natural logarithm (base e).

```spectra
let ln_e = std.math.log_f(2.71828);   // ~1.0
```

#### `log2_f(x: float) -> float` / `log10_f(x: float) -> float`

```spectra
let l2  = std.math.log2_f(8.0);     // 3.0
let l10 = std.math.log10_f(1000.0); // 3.0
```

#### `atan2_f(y: float, x: float) -> float`

**PT-BR:** Arco-tangente de y/x, considerando o quadrante.  
**EN-US:** Arc-tangent of y/x, considering the quadrant.

```spectra
let angulo = std.math.atan2_f(1.0, 1.0);   // pi/4 (~0.785)
```

#### `is_nan_f(x: float) -> bool` / `is_infinite_f(x: float) -> bool`

```spectra
let nan_check = std.math.is_nan_f(0.0 / 0.0);        // true (comportamento impl-defined)
let inf_check = std.math.is_infinite_f(1.0 / 0.0);   // true
```

### Constantes / Constants

#### `pi() -> float`

```spectra
let pi = std.math.pi();    // ~3.14159265358979
```

#### `e_const() -> float`

```spectra
let e = std.math.e_const();    // ~2.71828182845905
```

### Exemplo Completo / Complete Example

```spectra
module matematica;

import std.math as m;
import { println } from std.io;

pub fn main() {
    let pi = m.pi();
    let raio = 5.0;
    let area = pi * m.pow_f(raio, 2.0);
    let circunferencia = 2.0 * pi * raio;

    println(f"Raio: {raio}");
    println(f"Área: {area}");
    println(f"Circunferência: {circunferencia}");

    // Teorema de Pitágoras / Pythagorean theorem
    let a = 3.0;
    let b = 4.0;
    let hipotenusa = m.sqrt_f(m.pow_f(a, 2.0) + m.pow_f(b, 2.0));
    println(f"Hipotenusa: {hipotenusa}");    // 5.0
}
```

---

## 4. std.convert — Conversão de Tipos / Type Conversion

```spectra
import std.convert;
```

### Funções / Functions

#### `int_to_string(val: int) -> string`

```spectra
let s = std.convert.int_to_string(42);      // "42"
let s2 = std.convert.int_to_string(-100);   // "-100"
```

#### `float_to_string(val: float) -> string`

```spectra
let s = std.convert.float_to_string(3.14);  // "3.14"
```

#### `bool_to_string(val: bool) -> string`

```spectra
let s1 = std.convert.bool_to_string(true);   // "true"
let s2 = std.convert.bool_to_string(false);  // "false"
```

#### `string_to_int(s: string) -> int`

**PT-BR:** Converte string para int. Retorna `0` em caso de erro.  
**EN-US:** Converts string to int. Returns `0` on error.

```spectra
let n = std.convert.string_to_int("123");   // 123
let e = std.convert.string_to_int("abc");   // 0 (erro / error)
```

#### `string_to_float(s: string) -> float`

**PT-BR:** Converte string para float. Retorna `0.0` em caso de erro.  
**EN-US:** Converts string to float. Returns `0.0` on error.

```spectra
let f = std.convert.string_to_float("3.14");    // 3.14
let e = std.convert.string_to_float("xyz");     // 0.0
```

#### `int_to_float(val: int) -> float`

```spectra
let f = std.convert.int_to_float(7);    // 7.0
```

#### `float_to_int(val: float) -> int`

**PT-BR:** Converte float para int truncando (não arredonda).  
**EN-US:** Converts float to int by truncating (not rounding).

```spectra
let i = std.convert.float_to_int(9.9);     // 9  (truncado / truncated)
let i2 = std.convert.float_to_int(-3.7);   // -3
```

#### `string_to_int_or(s: string, default: int) -> int`

**PT-BR:** Converte com valor padrão em caso de erro.  
**EN-US:** Converts with a default value on error.

```spectra
let n = std.convert.string_to_int_or("abc", -1);   // -1
let n2 = std.convert.string_to_int_or("42", -1);   // 42
```

#### `string_to_float_or(s: string, default: float) -> float`

```spectra
let f = std.convert.string_to_float_or("bad", 0.0);   // 0.0
```

#### `string_to_bool(s: string) -> bool`

**PT-BR:** Retorna `true` se a string for `"true"` (case-insensitive), `false` caso contrário.  
**EN-US:** Returns `true` if the string is `"true"` (case-insensitive), `false` otherwise.

```spectra
let b1 = std.convert.string_to_bool("true");    // true
let b2 = std.convert.string_to_bool("false");   // false
let b3 = std.convert.string_to_bool("1");       // false
```

#### `bool_to_int(b: bool) -> int`

```spectra
let i1 = std.convert.bool_to_int(true);    // 1
let i2 = std.convert.bool_to_int(false);   // 0
```

---

## 5. std.collections — Coleções / Collections

**PT-BR:**  
O módulo `std.collections` proVê listas dinâmicas via **handles** opacos (inteiros). Um handle é um identificador numérico para uma lista gerenciada pelo runtime. Não manipule handles diretamente.

**EN-US:**  
The `std.collections` module provides dynamic lists via opaque **handles** (integers). A handle is a numeric identifier for a runtime-managed list. Do not manipulate handles directly.

```spectra
import std.collections as col;
```

### Operações Básicas / Basic Operations

#### `list_new() -> int`

**PT-BR:** Cria uma nova lista vazia. Retorna o handle.  
**EN-US:** Creates a new empty list. Returns the handle.

```spectra
let lista = col.list_new();    // handle, ex: 1
```

#### `list_push(handle: int, value: int) -> unit`

```spectra
let lista = col.list_new();
col.list_push(lista, 10);
col.list_push(lista, 20);
col.list_push(lista, 30);
```

#### `list_len(handle: int) -> int`

```spectra
let n = col.list_len(lista);   // 3
```

#### `list_get(handle: int, index: int) -> int`

**PT-BR:** Retorna o elemento no índice. Retorna `-1` se fora dos limites.  
**EN-US:** Returns the element at the index. Returns `-1` if out of bounds.

```spectra
let v = col.list_get(lista, 0);    // 10
let oob = col.list_get(lista, 99); // -1
```

#### `list_set(handle: int, index: int, value: int) -> unit`

```spectra
col.list_set(lista, 0, 99);    // Substitui o elemento 0 por 99
```

#### `list_pop(handle: int) -> int`

**PT-BR:** Remove e retorna o último elemento. Retorna `-1` se vazia.  
**EN-US:** Removes and returns the last element. Returns `-1` if empty.

```spectra
let ultimo = col.list_pop(lista);    // 30
```

#### `list_pop_front(handle: int) -> int`

```spectra
let primeiro = col.list_pop_front(lista);    // 10
```

#### `list_insert_at(handle: int, index: int, value: int) -> unit`

```spectra
col.list_insert_at(lista, 1, 50);    // Insere 50 na posição 1
```

#### `list_remove_at(handle: int, index: int) -> int`

**PT-BR:** Remove o elemento no índice e o retorna. Retorna `-1` se inválido.  
**EN-US:** Removes the element at the index and returns it. Returns `-1` if invalid.

```spectra
let removido = col.list_remove_at(lista, 0);
```

#### `list_contains(handle: int, value: int) -> bool`

```spectra
let tem = col.list_contains(lista, 20);   // true/false
```

#### `list_index_of(handle: int, value: int) -> int`

**PT-BR:** Retorna o índice da primeira ocorrência ou `-1`.  
**EN-US:** Returns the index of the first occurrence or `-1`.

```spectra
let idx = col.list_index_of(lista, 20);   // índice ou -1
```

#### `list_sort(handle: int) -> unit`

**PT-BR:** Ordena a lista em ordem crescente in-place.  
**EN-US:** Sorts the list in ascending order in-place.

```spectra
col.list_sort(lista);
```

#### `list_clear(handle: int) -> unit`

```spectra
col.list_clear(lista);    // Remove todos os elementos
```

#### `list_free(handle: int) -> unit`

**PT-BR:** Libera a memória da lista. **Importante:** Chamar quando não precisar mais.  
**EN-US:** Frees the list's memory. **Important:** Call when no longer needed.

```spectra
col.list_free(lista);    // Libera recursos
```

#### `list_free_all() -> int`

**PT-BR:** Libera todas as listas alocadas. Retorna quantas foram liberadas.  
**EN-US:** Frees all allocated lists. Returns how many were freed.

```spectra
let liberadas = col.list_free_all();
```

### Funções de Alta Ordem / Higher-Order Functions

#### `list_map(handle: int, fn_ptr: int) -> int`

**PT-BR:** Cria uma nova lista aplicando a função a cada elemento.  
**EN-US:** Creates a new list by applying the function to each element.

> **Nota / Note:** `fn_ptr` é um ponteiro para função obtido via conversão. O uso direto com closures SpectraLang está em desenvolvimento.

#### `list_filter(handle: int, fn_ptr: int) -> int`

**PT-BR:** Cria uma nova lista com apenas os elementos que satisfazem o predicado.  
**EN-US:** Creates a new list with only elements satisfying the predicate.

#### `list_reduce(handle: int, initial: int, fn_ptr: int) -> int`

**PT-BR:** Reduz a lista a um único valor acumulando com a função.  
**EN-US:** Reduces the list to a single value by accumulating with the function.

#### `list_sort_by(handle: int, fn_ptr: int) -> unit`

**PT-BR:** Ordena com comparador customizado. A função comparador deve retornar `-1`, `0`, ou `1`.  
**EN-US:** Sorts with a custom comparator. The comparator function must return `-1`, `0`, or `1`.

### Exemplo Completo / Complete Example

```spectra
module usando_colecoes;

import std.collections as col;
import { println } from std.io;
import std.convert;

pub fn main() {
    // Criar lista / Create list
    let lista = col.list_new();

    // Adicionar elementos / Add elements
    col.list_push(lista, 5);
    col.list_push(lista, 3);
    col.list_push(lista, 8);
    col.list_push(lista, 1);
    col.list_push(lista, 9);
    col.list_push(lista, 2);

    println(f"Tamanho: {col.list_len(lista)}");    // 6

    // Ordenar / Sort
    col.list_sort(lista);

    // Imprimir todos / Print all
    let i = 0;
    while i < col.list_len(lista) {
        println(std.convert.int_to_string(col.list_get(lista, i)));
        i = i + 1;
    }
    // 1, 2, 3, 5, 8, 9

    // Verificar / Check
    println(f"Contém 5: {col.list_contains(lista, 5)}");   // true
    println(f"Índice de 8: {col.list_index_of(lista, 8)}"); // 4

    // Liberar / Free
    col.list_free(lista);
}
```

---

## 6. std.random — Números Aleatórios / Random Numbers

```spectra
import std.random;
```

#### `random_seed(seed: int) -> unit`

**PT-BR:** Define a semente do gerador de números aleatórios. Use para resultados reproduzíveis.  
**EN-US:** Sets the random number generator seed. Use for reproducible results.

```spectra
std.random.random_seed(42);
```

#### `random_int(min: int, max: int) -> int`

**PT-BR:** Retorna um inteiro aleatório em `[min, max]` (inclusivo).  
**EN-US:** Returns a random integer in `[min, max]` (inclusive).

```spectra
let dado = std.random.random_int(1, 6);    // 1 a 6
let moeda = std.random.random_int(0, 1);   // 0 ou 1
```

#### `random_float() -> float`

**PT-BR:** Retorna um float aleatório em `[0.0, 1.0)`.  
**EN-US:** Returns a random float in `[0.0, 1.0)`.

```spectra
let f = std.random.random_float();    // ex: 0.7351...
```

#### `random_bool() -> bool`

```spectra
let b = std.random.random_bool();    // true ou false
```

---

## 7. std.fs — Sistema de Arquivos / File System

```spectra
import std.fs;
```

#### `fs_read(path: string) -> string`

**PT-BR:** Lê o conteúdo completo de um arquivo. Retorna `""` em caso de erro.  
**EN-US:** Reads the full content of a file. Returns `""` on error.

```spectra
let conteudo = std.fs.fs_read("dados.txt");
if std.string.is_empty(conteudo) {
    println("Arquivo não encontrado ou vazio");
}
```

#### `fs_write(path: string, content: string) -> bool`

**PT-BR:** Escreve (substitui) o conteúdo de um arquivo. Retorna `true` em sucesso.  
**EN-US:** Writes (replaces) file content. Returns `true` on success.

```spectra
let ok = std.fs.fs_write("saida.txt", "Hello, World!\n");
```

#### `fs_append(path: string, content: string) -> bool`

**PT-BR:** Adiciona conteúdo ao final de um arquivo.  
**EN-US:** Appends content to the end of a file.

```spectra
std.fs.fs_append("log.txt", "Nova entrada de log\n");
```

#### `fs_exists(path: string) -> bool`

```spectra
if std.fs.fs_exists("config.txt") {
    let cfg = std.fs.fs_read("config.txt");
}
```

#### `fs_remove(path: string) -> bool`

```spectra
let removido = std.fs.fs_remove("temp.txt");
```

---

## 8. std.env — Ambiente / Environment

```spectra
import std.env;
```

#### `env_get(key: string) -> string`

**PT-BR:** Obtém uma variável de ambiente. Retorna `""` se não definida.  
**EN-US:** Gets an environment variable. Returns `""` if not set.

```spectra
let home = std.env.env_get("HOME");
let path = std.env.env_get("PATH");
```

#### `env_set(key: string, value: string) -> bool`

```spectra
let ok = std.env.env_set("MINHA_VAR", "valor");
```

#### `env_args_count() -> int`

**PT-BR:** Retorna o número de argumentos da linha de comando.  
**EN-US:** Returns the number of command-line arguments.

```spectra
let argc = std.env.env_args_count();
println(f"Argumentos: {argc}");
```

#### `env_arg(index: int) -> string`

**PT-BR:** Retorna o argumento na posição `index`. Retorna `""` se fora dos limites.  
**EN-US:** Returns the argument at position `index`. Returns `""` if out of bounds.

```spectra
let arg0 = std.env.env_arg(0);    // nome do programa / program name
let arg1 = std.env.env_arg(1);    // primeiro argumento / first argument

// Processando todos os argumentos / Processing all arguments
let n = std.env.env_args_count();
for i in 0..n {
    println(f"arg[{i}] = {std.env.env_arg(i)}");
}
```

---

## 9. std.option — Operações em Option / Option Operations

```spectra
import std.option;
```

#### `is_some(opt: unknown) -> bool`

```spectra
let opt = Option::Some(42);
let tem = std.option.is_some(opt);    // true
```

#### `is_none(opt: unknown) -> bool`

```spectra
let nada = Option::None;
let vazio = std.option.is_none(nada);    // true
```

#### `option_unwrap(opt: unknown) -> unknown`

**PT-BR:** Extrai o valor de `Some`. **Provoca panic** se for `None`.  
**EN-US:** Extracts the value from `Some`. **Panics** if `None`.

```spectra
let val = std.option.option_unwrap(Option::Some(42));   // 42
// std.option.option_unwrap(Option::None);  // PANIC!
```

#### `option_unwrap_or(opt: unknown, default: unknown) -> unknown`

**PT-BR:** Extrai o valor ou retorna o padrão se `None`.  
**EN-US:** Extracts the value or returns the default if `None`.

```spectra
let val = std.option.option_unwrap_or(Option::Some(42), 0);   // 42
let def = std.option.option_unwrap_or(Option::None, 99);      // 99
```

---

## 10. std.result — Operações em Result / Result Operations

```spectra
import std.result;
```

#### `is_ok(res: unknown) -> bool`

```spectra
let r = Result::Ok(100);
let ok = std.result.is_ok(r);      // true
```

#### `is_err(res: unknown) -> bool`

```spectra
let e = Result::Err("falha");
let err = std.result.is_err(e);    // true
```

#### `result_unwrap(res: unknown) -> unknown`

**PT-BR:** Extrai o valor de `Ok`. **Provoca panic** se for `Err`.  
**EN-US:** Extracts the value from `Ok`. **Panics** if `Err`.

```spectra
let val = std.result.result_unwrap(Result::Ok(42));    // 42
```

#### `result_unwrap_or(res: unknown, default: unknown) -> unknown`

```spectra
let val = std.result.result_unwrap_or(Result::Err("e"), 0);   // 0
```

#### `result_unwrap_err(res: unknown) -> unknown`

**PT-BR:** Extrai o valor de `Err`. **Provoca panic** se for `Ok`.  
**EN-US:** Extracts the value from `Err`. **Panics** if `Ok`.

```spectra
let msg = std.result.result_unwrap_err(Result::Err("algo errado"));   // "algo errado"
```

---

## 11. std.char — Operações em Caracteres / Character Operations

**PT-BR:**  
As funções de `std.char` operam sobre **códigos Unicode** (inteiros), o mesmo formato retornado por `std.string.char_at()`.

**EN-US:**  
Functions in `std.char` operate on **Unicode code points** (integers), the same format returned by `std.string.char_at()`.

```spectra
import std.char;
```

#### `is_alpha(c: int) -> bool`

```spectra
let sim = std.char.is_alpha(65);     // true ('A')
let nao = std.char.is_alpha(48);     // false ('0')
```

#### `is_digit_char(c: int) -> bool`

```spectra
let sim = std.char.is_digit_char(48);   // true ('0')
let nao = std.char.is_digit_char(65);   // false ('A')
```

#### `is_whitespace_char(c: int) -> bool`

```spectra
let sim = std.char.is_whitespace_char(32);   // true (espaço / space)
let sim2 = std.char.is_whitespace_char(9);   // true (tab)
```

#### `is_upper_char(c: int) -> bool` / `is_lower_char(c: int) -> bool`

```spectra
let upper = std.char.is_upper_char(65);   // true ('A')
let lower = std.char.is_lower_char(97);   // true ('a')
```

#### `is_alphanumeric(c: int) -> bool`

```spectra
let sim = std.char.is_alphanumeric(97);   // true ('a')
let sim2 = std.char.is_alphanumeric(48);  // true ('0')
let nao = std.char.is_alphanumeric(32);   // false (espaço)
```

#### `to_upper_char(c: int) -> int` / `to_lower_char(c: int) -> int`

```spectra
let A = std.char.to_upper_char(97);    // 65 ('A')
let a = std.char.to_lower_char(65);    // 97 ('a')
```

### Exemplo: Processamento de String Caractere a Caractere

```spectra
module analisar_string;

import std.string as s;
import std.char as c;
import { println } from std.io;
import std.convert;

fn contar_digitos(texto: string) -> int {
    let count = 0;
    let i = 0;
    let len = s.len(texto);
    while i < len {
        let codigo = s.char_at(texto, i);
        if c.is_digit_char(codigo) {
            count = count + 1;
        }
        i = i + 1;
    }
    return count;
}

pub fn main() {
    let texto = "abc123def456";
    let n = contar_digitos(texto);
    println(f"Dígitos em '{texto}': {n}");    // 6
}
```

---

## 12. std.time — Tempo / Time

```spectra
import std.time;
```

#### `time_now_millis() -> int`

**PT-BR:** Retorna os milissegundos desde a época Unix. Retorna `-1` em caso de erro.  
**EN-US:** Returns milliseconds since the Unix epoch. Returns `-1` on error.

```spectra
let inicio = std.time.time_now_millis();
// ... operação / operation ...
let fim = std.time.time_now_millis();
let duracao = fim - inicio;
println(f"Duração: {duracao}ms");
```

#### `time_now_secs() -> int`

**PT-BR:** Retorna os segundos desde a época Unix. Retorna `-1` em caso de erro.  
**EN-US:** Returns seconds since the Unix epoch. Returns `-1` on error.

```spectra
let agora = std.time.time_now_secs();
println(f"Timestamp: {agora}");
```

#### `sleep_ms(ms: int) -> unit`

**PT-BR:** Pausa a execução por `ms` milissegundos.  
**EN-US:** Pauses execution for `ms` milliseconds.

```spectra
println("Aguardando...");
std.time.sleep_ms(1000);    // Pausa 1 segundo / Pause 1 second
println("Pronto!");
```

### Exemplo: Benchmark Simples

```spectra
module benchmark;

import std.time;
import { println } from std.io;

fn operacao_pesada(n: int) -> int {
    let soma = 0;
    for i in 0..n {
        soma = soma + i;
    }
    return soma;
}

pub fn main() {
    let inicio = std.time.time_now_millis();
    let resultado = operacao_pesada(1000000);
    let fim = std.time.time_now_millis();

    println(f"Resultado: {resultado}");
    println(f"Tempo: {fim - inicio}ms");
}
```

---

> **Próximo / Next:** [06 — Referência Rápida / Quick Reference](06-referencia-rapida.md)  
> **Anterior / Previous:** [04 — Avançado / Advanced](04-avancado.md)
