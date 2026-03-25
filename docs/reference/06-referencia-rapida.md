# SpectraLang — Referência Rápida / Quick Reference

> **Nível / Level:** Todos / All  
> **Parte / Part:** 6 de 6  
> Este arquivo é um **cartão de consulta rápida** — ele não é didático. Para aprendizado, veja os arquivos 01–05.  
> This file is a **quick reference card** — it is not didactic. For learning, see files 01–05.

---

## Sumário / Table of Contents

1. [Palavras-Chave / Keywords](#1-palavras-chave--keywords)
2. [Operadores e Precedência / Operators & Precedence](#2-operadores-e-precedência--operators--precedence)
3. [Tipos / Types](#3-tipos--types)
4. [Sequências de Escape / Escape Sequences](#4-sequências-de-escape--escape-sequences)
5. [Visibilidade / Visibility](#5-visibilidade--visibility)
6. [Módulos — Formas de Importação / Import Forms](#6-módulos--formas-de-importação--import-forms)
7. [Estruturas de Controle — Resumo / Control Flow Summary](#7-estruturas-de-controle--resumo--control-flow-summary)
8. [Padrões em match / Patterns in match](#8-padrões-em-match--patterns-in-match)
9. [Erros Comuns / Common Errors](#9-erros-comuns--common-errors)
10. [Convenções de Nomenclatura / Naming Conventions](#10-convenções-de-nomenclatura--naming-conventions)
11. [Gramática Informal / Informal Grammar](#11-gramática-informal--informal-grammar)
12. [Módulos da Stdlib — Resumo / Stdlib Summary](#12-módulos-da-stdlib--resumo--stdlib-summary)
13. [Flags do CLI / CLI Flags](#13-flags-do-cli--cli-flags)

---

## 1. Palavras-Chave / Keywords

**39 palavras-chave reservadas / 39 reserved keywords**

| Palavra-chave | Status | Uso / Use |
|---------------|--------|-----------|
| `module` | ✅ Implementado | Declara o módulo do arquivo |
| `import` | ✅ Implementado | Importa módulos/símbolos |
| `export` | 🚧 Reservado | (futuro / future) |
| `pub` | ✅ Implementado | Visibilidade pública |
| `internal` | ✅ Implementado | Visibilidade interna ao módulo |
| `fn` | ✅ Implementado | Declara função |
| `struct` | ✅ Implementado | Declara struct |
| `enum` | ✅ Implementado | Declara enum |
| `impl` | ✅ Implementado | Bloco de implementação |
| `trait` | ✅ Implementado | Declara trait |
| `class` | 🚧 Reservado | (futuro / future) |
| `let` | ✅ Implementado | Declara variável |
| `mut` | ✅ Implementado | Mutabilidade |
| `Self` | ✅ Implementado | Tipo do impl atual |
| `self` | ✅ Implementado | Receptor do método |
| `if` | ✅ Implementado | Condicional |
| `elif` | ✅ Implementado | Senão-se |
| `elseif` | ✅ Implementado | Alias de `elif` |
| `else` | ✅ Implementado | Senão |
| `unless` | ✅ Implementado | Condicional negada |
| `while` | ✅ Implementado | Laço condicional |
| `do` | ✅ Implementado | Laço do-while (com `while`) |
| `for` | ✅ Implementado | Laço `for in` |
| `foreach` | 🚧 Reservado | (futuro / future) |
| `in` | ✅ Implementado | Em `for x in` |
| `of` | 🚧 Reservado | (futuro / future) |
| `loop` | ✅ Implementado | Laço infinito |
| `repeat` | 🚧 Reservado | (futuro / future) |
| `until` | 🚧 Reservado | (futuro / future) |
| `match` | ✅ Implementado | Pattern matching |
| `switch` | ✅ Implementado | Comparação por valor |
| `case` | ✅ Implementado | Braço de switch/match |
| `cond` | 🚧 Reservado | (futuro / future) |
| `return` | ✅ Implementado | Retorno explícito |
| `break` | ✅ Implementado | Sai do laço |
| `continue` | ✅ Implementado | Próxima iteração |
| `yield` | 🚧 Reservado | (futuro / future) |
| `goto` | 🚧 Reservado | (futuro / future) |
| `true` | ✅ Implementado | Literal booleano |
| `false` | ✅ Implementado | Literal booleano |

---

## 2. Operadores e Precedência / Operators & Precedence

**Do mais alto ao mais baixo / From highest to lowest:**

| Nível | Operador(es) | Tipo / Type | Associatividade |
|-------|-------------|-------------|-----------------|
| 1 (alto) | `(expr)`, `f()`, `x.y`, `x[i]` | Primário | Esq. / Left |
| 2 | `-x` (unário), `!x` | Unário | Direita / Right |
| 3 | `*`, `/`, `%` | Multiplicativo | Esq. / Left |
| 4 | `+`, `-` | Aditivo | Esq. / Left |
| 5 | `<`, `>`, `<=`, `>=` | Relacional | Esq. / Left |
| 6 | `==`, `!=` | Igualdade | Esq. / Left |
| 7 | `&&` | E lógico / Logical AND | Esq. / Left |
| 8 (baixo) | `\|\|` | Ou lógico / Logical OR | Esq. / Left |

**Operadores de atribuição / Assignment operators:**

| Operador | Equivalente a |
|----------|--------------|
| `=` | atribuição / assignment |
| `+=` | `x = x + rhs` |
| `-=` | `x = x - rhs` |
| `*=` | `x = x * rhs` |
| `/=` | `x = x / rhs` |
| `%=` | `x = x % rhs` |

**Nota:** Operadores de atribuição **não** são expressões — não retornam valor.  
**Note:** Assignment operators are **not** expressions — they don't return a value.

---

## 3. Tipos / Types

### Primitivos / Primitives

| Tipo | Tamanho / Size | Valores / Values | Exemplo |
|------|----------------|------------------|---------|
| `int` | 64-bit signed | -2⁶³ … 2⁶³-1 | `42`, `-7`, `0` |
| `float` | 64-bit IEEE 754 | ±1.8×10³⁰⁸ | `3.14`, `-0.5` |
| `bool` | 1-bit | `true`, `false` | `true` |
| `string` | Ponteiro UTF-8 | qualquer texto | `"hello"` |
| `char` | Unicode codepoint | caractere único | `'a'`, `'\n'` |
| `unit` | 0 bits | `()` | implícito |

### Compostos / Composite

| Tipo | Sintaxe | Exemplo |
|------|---------|---------|
| Array | `[T; N]` | `[1, 2, 3]` |
| Array dinâmico | `[T]` | `let a: [int] = [1, 2, 3]` |
| Tupla | `(T1, T2)` | `(1, "hi")` |
| Struct | `struct Nome { ... }` | `Ponto { x: 0, y: 0 }` |
| Enum | `enum Nome { ... }` | `Cor::Azul` |
| Option | `Option<T>` | `Option::Some(42)` |
| Result | `Result<T, E>` | `Result::Ok(v)` |

### Compatibilidade / Compatibility

| De / From | Para / To | Como / How |
|-----------|-----------|------------|
| `int` | `float` | `std.convert.int_to_float(x)` |
| `float` | `int` | `std.convert.float_to_int(x)` (trunca / truncates) |
| `int` | `string` | `std.convert.int_to_string(x)` |
| `float` | `string` | `std.convert.float_to_string(x)` |
| `bool` | `string` | `std.convert.bool_to_string(x)` |
| `bool` | `int` | `std.convert.bool_to_int(x)` → 0 ou 1 |
| `string` | `int` | `std.convert.string_to_int(x)` (0 se erro) |
| `string` | `float` | `std.convert.string_to_float(x)` (0.0 se erro) |
| `int`↔`float` | operação mista | **ERRO** — deve converter explicitamente |
| `int`↔`bool` | aritmética | **ERRO** — bool não é numérico |

---

## 4. Sequências de Escape / Escape Sequences

| Escape | Significado | Unicode |
|--------|-------------|---------|
| `\n` | Nova linha / Newline | U+000A |
| `\t` | Tabulação / Tab | U+0009 |
| `\r` | Retorno de carro / Carriage return | U+000D |
| `\\` | Barra invertida / Backslash | U+005C |
| `\"` | Aspas duplas / Double quote | U+0022 |
| `\'` | Aspas simples / Single quote | U+0027 |
| `\0` | Nulo / Null | U+0000 |

**Aplicam-se em / Apply in:** `string`, `char`, `f-string`

---

## 5. Visibilidade / Visibility

| Modificador | Acessível de / Accessible from |
|-------------|-------------------------------|
| `pub` | Qualquer código / Any code |
| `internal` | Mesmo módulo / Same module |
| *(sem/none)* | Apenas definindo / Only defining scope |

**Regras / Rules:**
- `pub fn main()` é o ponto de entrada obrigatório.
- Structs com campos `pub` precisam declarar cada campo com `pub`.
- Funções sem `pub` ou `internal` são privadas ao bloco de declaração.

---

## 6. Módulos — Formas de Importação / Import Forms

```spectra
// Forma 1: importar módulo completo / Import whole module
import std.math;
// Uso: std.math.sqrt_f(x)

// Forma 2: importar com alias / Import with alias
import std.math as m;
// Uso: m.sqrt_f(x)

// Forma 3: importar símbolos nomeados / Import named symbols
import { println, print } from std.io;
// Uso: println("hello")

// Módulo do arquivo / File module declaration
module meu_modulo;
```

---

## 7. Estruturas de Controle — Resumo / Control Flow Summary

```spectra
// if / elif / else
if condicao {
    // ...
} elif outra {
    // ...
} else {
    // ...
}

// unless (= if not)
unless condicao {
    // ...
}

// while
while condicao {
    // ...
}

// do-while
do {
    // ...
} while condicao;

// for com range / for with range
for i in 0..10 { /* 0 a 9 */ }
for i in 0..=10 { /* 0 a 10 */ }

// for com array / for with array
for item in arr { /* ... */ }

// loop infinito / infinite loop
loop {
    // ...
    break;    // necessário para sair / needed to exit
}

// switch (comparação por valor / value comparison)
switch valor {
    case 1 => println("um"),
    case 2 | 3 => println("dois ou três"),
    _ => println("outro")
}

// match (pattern matching)
match opcao {
    Option::Some(v) => println(f"Tem: {v}"),
    Option::None => println("Vazio")
}

// if let
if let Option::Some(v) = possivel_valor {
    // ...
}

// Control: break / continue
for i in 0..10 {
    if i == 3 { continue; }
    if i == 7 { break; }
}
```

---

## 8. Padrões em match / Patterns in match

| Padrão | Sintaxe | Exemplo |
|--------|---------|---------|
| Literal inteiro | `42` | `case 42 =>` |
| Literal string | `"hello"` | `case "ok" =>` |
| Literal bool | `true` / `false` | `case true =>` |
| Wildcard | `_` | `_ =>` |
| Binding | `nome` | `x => println(x)` |
| Enum unit | `Enum::Var` | `Cor::Azul =>` |
| Enum tuple | `Enum::Var(a, b)` | `Ponto::Par(x, y) =>` |
| Enum struct | `Enum::Var { campo }` | `Forma::Rect { w, h } =>` |
| Multi-padrão | `pat1 \| pat2` | `1 \| 2 =>` |
| Guard | `pat if cond` | Não implementado ainda |

**Regra de exhaustividade / Exhaustiveness rule:**  
Todo `match` deve cobrir **todos** os casos possíveis. Use `_` como wildcard.

---

## 9. Erros Comuns / Common Errors

| Erro | Causa | Solução |
|------|-------|---------|
| `type mismatch: int and float` | Operação mista sem conversão | Use `int_to_float()` ou `float_to_int()` |
| `cannot assign to immutable variable` | Reatribuição de `let` sem `mut` | Declare `let mut x = ...` |
| `non-exhaustive match` | Casos faltando no `match` | Adicione `_ =>` ou os casos faltantes |
| `return outside function` | `return` fora de função | Mova para dentro de uma função |
| `undefined variable` | Uso fora do escopo | Garanta que `let` está no escopo correto |
| `break/continue outside loop` | Fora de laço | Mova para dentro de `while`/`for`/`loop` |
| `module declaration missing` | Arquivo sem `module nome;` | Adicione `module nome;` no topo |
| `main not found` | Sem `pub fn main()` | Adicione `pub fn main() { ... }` |
| `cannot use bool in arithmetic` | `bool + int` | Use `bool_to_int()` explicitamente |
| `field not found` | Campo inexistente na struct | Verifique o nome do campo |

---

## 10. Convenções de Nomenclatura / Naming Conventions

| Construto | Convenção | Exemplo |
|-----------|-----------|---------|
| Variáveis | `snake_case` | `meu_valor`, `nome_usuario` |
| Funções | `snake_case` | `calcular_area()` |
| Parâmetros | `snake_case` | `fn f(valor_x: int)` |
| Structs | `PascalCase` | `Ponto`, `UsuarioCadastrado` |
| Enums | `PascalCase` | `Cor`, `ResultadoOp` |
| Variantes de Enum | `PascalCase` | `Cor::VermelhoEscuro` |
| Traits | `PascalCase` | `Exibivel`, `Calculavel` |
| Módulos | `snake_case` | `module minha_lib;` |
| Arquivos | `snake_case.spectra` | `minha_lib.spectra` |
| Constantes | `UPPER_SNAKE` (convenção, `let` não diferencia) | `let MAX_PONTOS = 100;` |

---

## 11. Gramática Informal / Informal Grammar

```ebnf
programa      = declaracao_modulo? import* declaracao* ;

declaracao_modulo = "module" IDENT ";" ;

import        = "import" caminho_modulo ";"
              | "import" caminho_modulo "as" IDENT ";"
              | "import" "{" IDENT ("," IDENT)* "}" "from" caminho_modulo ";" ;

declaracao    = decl_fn | decl_struct | decl_enum | decl_impl | decl_trait | decl_trait_impl ;

decl_fn       = visib? "fn" IDENT genericos? "(" params? ")" (":" tipo)? bloco ;

decl_struct   = visib? "struct" IDENT genericos? "{" campo* "}" ;

decl_enum     = visib? "enum" IDENT genericos? "{" variante* "}" ;

variante      = IDENT                                    (* unit *)
              | IDENT "(" tipo ("," tipo)* ")"           (* tupla *)
              | IDENT "{" campo* "}"                     (* struct *)
              ;

decl_impl     = "impl" genericos? IDENT ("{" metodo* "}")? ;

decl_trait    = visib? "trait" IDENT (":" IDENT)? "{" assinatura* "}" ;

bloco         = "{" stmt* expr? "}" ;

stmt          = decl_let | atribuicao | retorno | expr ";" | laço | condicional | match ;

decl_let      = "let" "mut"? IDENT (":" tipo)? "=" expr ";" ;

expr          = literal | IDENT | binario | unario | chamada | met_call | f_string
              | "if" expr bloco ("elif" expr bloco)* ("else" bloco)?
              | "unless" expr bloco ("else" bloco)?
              | "match" expr "{" braço* "}"
              | "|" params? "|" (expr | bloco)
              | "[" (expr ("," expr)*)? "]"
              | "(" expr ("," expr)+ ")"
              ;

tipo          = "int" | "float" | "bool" | "string" | "char"
              | IDENT                       (* tipo nomeado *)
              | IDENT "<" tipo ("," tipo)* ">"   (* genérico *)
              | "[" tipo "]"               (* array *)
              | "(" tipo ("," tipo)* ")"   (* tupla *)
              ;

visib         = "pub" | "internal" ;

genericos     = "<" IDENT (":" IDENT ("+" IDENT)*)? ("," ...)* ">" ;
```

---

## 12. Módulos da Stdlib — Resumo / Stdlib Summary

| Módulo | Funções Principais / Key Functions |
|--------|-----------------------------------|
| `std.io` | `print`, `println`, `eprint`, `eprintln`, `read_line`, `input`, `flush` |
| `std.string` | `len`, `contains`, `to_upper`, `to_lower`, `trim`, `substring`, `replace`, `split_first`, `split_last`, `index_of`, `concat`, `is_empty`, `reverse_str`, `pad_left`, `pad_right` |
| `std.math` | `abs`, `min`, `max`, `clamp`, `sign`, `gcd`, `lcm`, `sqrt_f`, `pow_f`, `floor_f`, `ceil_f`, `round_f`, `sin_f`, `cos_f`, `tan_f`, `log_f`, `pi`, `e_const` |
| `std.convert` | `int_to_string`, `float_to_string`, `bool_to_string`, `string_to_int`, `string_to_float`, `int_to_float`, `float_to_int`, `string_to_int_or`, `bool_to_int` |
| `std.collections` | `list_new`, `list_push`, `list_pop`, `list_len`, `list_get`, `list_set`, `list_sort`, `list_contains`, `list_index_of`, `list_free` |
| `std.random` | `random_seed`, `random_int`, `random_float`, `random_bool` |
| `std.fs` | `fs_read`, `fs_write`, `fs_append`, `fs_exists`, `fs_remove` |
| `std.env` | `env_get`, `env_set`, `env_args_count`, `env_arg` |
| `std.option` | `is_some`, `is_none`, `option_unwrap`, `option_unwrap_or` |
| `std.result` | `is_ok`, `is_err`, `result_unwrap`, `result_unwrap_or`, `result_unwrap_err` |
| `std.char` | `is_alpha`, `is_digit_char`, `is_whitespace_char`, `is_alphanumeric`, `to_upper_char`, `to_lower_char` |
| `std.time` | `time_now_millis`, `time_now_secs`, `sleep_ms` |

---

## 13. Flags do CLI / CLI Flags

```
spectra <comando> [flags] <arquivo>
```

| Comando | Descrição |
|---------|-----------|
| `run` | Compila e executa com JIT |
| `build` | Compila para binário AOT |
| `check` | Verifica erros sem executar |
| `fmt` | Formata o código (WIP) |
| `doc` | Gera documentação (WIP) |

| Flag | Descrição |
|------|-----------|
| `--emit-ir` | Exibe o IR intermediário |
| `--emit-ast` | Exibe a AST em formato de debug |
| `--emit-tokens` | Exibe os tokens do lexer |
| `--no-color` | Desativa cores no terminal |
| `--output <path>` | Define o caminho de saída (build) |
| `--verbose` | Saída detalhada |
| `--version` | Versão do compilador |

---

## Índice de Todos os Arquivos / Index of All Files

| Arquivo | Conteúdo |
|---------|---------|
| [01-introducao.md](01-introducao.md) | O que é SpectraLang, filosofia, pipeline, CLI, Hello World |
| [02-fundamentos.md](02-fundamentos.md) | Variáveis, tipos, operadores, controle de fluxo, funções |
| [03-tipos-compostos.md](03-tipos-compostos.md) | Arrays, tuplas, ranges, structs, enums, impl, traits, genéricos, closures |
| [04-avancado.md](04-avancado.md) | Pattern matching, if let, while let, Option, Result, ?, módulos, visibilidade |
| [05-stdlib.md](05-stdlib.md) | Referência completa dos 12 módulos da stdlib |
| **06-referencia-rapida.md** | **Este arquivo — consulta rápida** |

---

> **Fim da documentação SpectraLang Alpha**  
> **End of SpectraLang Alpha documentation**  
>
> Para contribuir ou reportar erros, veja o repositório do projeto.  
> To contribute or report errors, see the project repository.
