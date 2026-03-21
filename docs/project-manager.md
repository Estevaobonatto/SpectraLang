# SpectraLang — Project Manager

> Repositório central de planejamento, roadmap, backlog e decisões arquiteturais da linguagem SpectraLang.

---

## Visão Geral

**SpectraLang** é uma linguagem de programação de propósito geral, de tipagem estática e compilada JIT, projetada para ser expressiva, segura e de alta performance. Inspirada em Rust, Kotlin, Swift e Python, combina sintaxe moderna com semântica robusta.

| Atributo           | Valor                                    |
|--------------------|------------------------------------------|
| Paradigma          | Multi-paradigma (funcional + imperativo) |
| Sistema de tipos   | Estático, inferência automática          |
| Gerenciamento de memória | GC puro (estilo Go/JVM)           |
| Backend            | Cranelift JIT                            |
| Runtime            | Nativo (Rust)                            |
| Estado atual       | **Alpha congelado** → Beta 0.1 em desenvolvimento |

---

## Milestones

| Versão     | Nome              | Status         | Foco principal                                |
|------------|-------------------|----------------|-----------------------------------------------|
| Alpha      | Core Language     | ✅ Congelado    | Estruturas básicas, tipos, loops, match        |
| Beta 0.1   | Literals & Syntax | 🔄 Em progresso | CharLiteral, f-string, lambda, `?`, `if let`  |
| Beta 0.2   | Stdlib & Errors   | 📋 Planejado    | Módulo std, Result/Option, error handling      |
| Beta 0.3   | Concorrência      | 📋 Planejado    | async/await, channels, green threads           |
| Beta 0.4   | Metaprogramação   | 📋 Planejado    | Macros, derive, code generation                |
| 1.0        | Stable Release    | 📋 Futuro       | Tooling, package manager, LSP completo         |

---

## Alpha — Recap do que está funcionando

- [x] Lexer com escape sequences (`\n`, `\t`, `\r`, `\\`, `\"`, `\0`)
- [x] Parser completo (expressões, statements, items)
- [x] Sistema de tipos: Int, Float, Bool, String, Char, Unit, Array, Tuple, Struct, Enum
- [x] Funções com parâmetros tipados e retorno
- [x] Generics e polimorfismo paramétrico (`fn foo<T>()`)
- [x] Traits e implementações (`trait Printable`, `impl Printable for Type`)
- [x] Herança de traits (trait hierarquias)
- [x] Match exaustivo com padrões (Wildcard, Literal, Identifier, EnumVariant)
- [x] Loops: `while`, `do-while`, `for-in`, `loop`
- [x] `if`, `elif`, `else`, `unless`
- [x] Structs com fields e métodos (`impl`)
- [x] Enums com variantes unit e tuple
- [x] Tuplas e destructuring básico
- [x] Arrays com acesso por índice
- [x] `break`, `continue`, `return`
- [x] `switch/case` (feature-gated)
- [x] Inferência de tipos em let bindings
- [x] Análise semântica completa com erros descritivos
- [x] Lowering para SSA IR (midend)
- [x] Backend Cranelift JIT funcional
- [x] Lint básico (variáveis não usadas, etc.)
- [x] Monomorphization de generics
- [x] Visibilidade `pub` / privado
- [x] Módulos e imports básicos

---

## Beta 0.1 — Literals, Syntax & Expressividade

**Objetivo:** Tornar a linguagem mais expressiva na sintaxe quotidiana. Features comparáveis a linguagens mainstream.

### 🔤 Char Literals
```spectra
let c: char = 'a';
let newline = '\n';
let tab = '\t';
```
- **Arquivo**: `compiler/src/token.rs`, `compiler/src/lexer/mod.rs`, `compiler/src/ast/mod.rs`
- **Status**: ✅ Front-end concluído e validado em exemplo dedicado

### 📝 String Interpolation (f-strings)
```spectra
let name = "World";
let msg = f"Hello, {name}!";
let calc = f"2 + 2 = {2 + 2}";
```
- **Arquivo**: `compiler/src/token.rs`, `compiler/src/lexer/mod.rs`, `compiler/src/parser/expression.rs`
- **Status**: ✅ Front-end concluído e validado em exemplo dedicado

### λ Closures/Lambdas
```spectra
let double = |x: int| x * 2;
let add = |a, b| a + b;
let greet = || println("Hello!");

// Em argumentos de função:
let nums = [1, 2, 3];
// (quando stdlib estiver pronta)
// let doubled = nums.map(|x| x * 2);
```
- **Arquivo**: `compiler/src/ast/mod.rs`, `compiler/src/parser/expression.rs`
- **Status**: 🟡 Parser, AST e análise semântica implementados; lowering/backend ainda parcial para chamadas via valor de função

### ❓ Operador `?` (Try/Propagate)
```spectra
fn read_config() -> Result<Config, Error> {
    let data = read_file("config.json")?;
    let config = parse_json(data)?;
    Ok(config)
}
```
- **Arquivo**: `compiler/src/ast/mod.rs`, `compiler/src/parser/expression.rs`, `midend/src/lowering.rs`
- **Status**: 🟡 Sintaxe e lowering básico implementados; falta validação completa de fluxo com `Result`/stdlib

### 🔀 Range Operator
```spectra
let r = 0..10;      // exclusivo: 0, 1, ..., 9
let r2 = 0..=10;    // inclusivo: 0, 1, ..., 10
for i in 0..5 { println(i); }
```
- **Arquivo**: `compiler/src/ast/mod.rs`, `compiler/src/parser/expression.rs`
- **Status**: ✅ Implementado e validado em exemplo dedicado

### 🔍 if let / while let
```spectra
if let Option::Some(value) = maybe_value {
    println(value);
}

while let Option::Some(next) = iterator.next() {
    process(next);
}
```
- **Arquivo**: `compiler/src/ast/mod.rs`, `compiler/src/parser/statement.rs`
- **Status**: 🟡 `if let` validado; `while let` ainda sem exemplo de regressão consolidado no workspace

### 📦 Struct-style Enum Variants
```spectra
enum Shape {
    Circle { radius: float },
    Rectangle { width: float, height: float },
    Point,
}
```
- **Arquivo**: `compiler/src/ast/mod.rs`, `compiler/src/parser/item.rs`
- **Status**: 🟡 Declaração, parsing de patterns e validação semântica implementados; backend ainda falha em enums com payloads heterogêneos/mistos

### 🗂️ Import Avançado (alias e named imports)
```spectra
import math.utils as utils;
import { sin, cos, tan } from math.trig;
import path.to.MyStruct;
```
- **Arquivo**: `compiler/src/ast/mod.rs`, `compiler/src/parser/module.rs`
- **Status**: 🟡 Sintaxe disponível; integração com símbolos esperados de `std.io` ainda pendente

### Status de Validação Atual

- ✅ `examples/test_beta_char_literals.spectra`
- ✅ `examples/test_beta_fstrings.spectra`
- ✅ `examples/test_beta_closures.spectra` no modo `check`
- ✅ `examples/test_beta_if_let.spectra`
- ✅ `examples/test_beta_ranges.spectra`
- 🟡 `examples/test_beta_struct_enum_variants.spectra` chega ao backend, mas ainda falha por representação IR inconsistente para enums com variantes mistas
- 🟡 `examples/test_beta_imports.spectra` ainda depende de resolução real de `std.io::{print, println}`

### Limitação Atual de Backend

- O frontend já aceita variants tuple-style e struct-style em enums.
- O backend ainda não possui uma representação única de tagged union para enums com combinações de:
    - variantes unit
    - variantes tuple-style com aridades diferentes
    - variantes struct-style com payload nomeado
- Enquanto isso não for resolvido, o comando `check` ainda pode falhar em exemplos que chegam à geração de código, mesmo com parsing e semântica corretos.
- **Status**: 🔄 Implementando

---

## Beta 0.2 — Stdlib & Error Handling

**Objetivo:** Biblioteca padrão funcional e tratamento de erros idiomático.

### Result<T, E> e Option<T> built-ins
```spectra
fn divide(a: int, b: int) -> Result<int, string> {
    if b == 0 {
        Err("cannot divide by zero")
    } else {
        Ok(a / b)
    }
}

let x: Option<int> = Some(42);
let y: Option<int> = None;
```

### Módulos de stdlib planejados

| Módulo         | Conteúdo                                            |
|----------------|-----------------------------------------------------|
| `std.io`       | `println`, `print`, `eprintln`, `read_line`         |
| `std.math`     | `abs`, `sqrt`, `pow`, `min`, `max`, `floor`, `ceil` |
| `std.string`   | `len`, `trim`, `split`, `join`, `contains`, `replace`, `to_upper`, `to_lower` |
| `std.array`    | `len`, `push`, `pop`, `map`, `filter`, `reduce`, `sort`, `contains` |
| `std.option`   | `unwrap`, `unwrap_or`, `map`, `and_then`, `is_some`, `is_none` |
| `std.result`   | `unwrap`, `unwrap_or`, `map`, `map_err`, `and_then`, `is_ok`, `is_err` |
| `std.convert`  | `to_int`, `to_float`, `to_string`, `to_bool`        |
| `std.fs`       | `read_file`, `write_file`, `exists`, `remove`       |
| `std.env`      | `args`, `get_env`, `set_env`                        |

### Error Types

```spectra
// Hierarquia de erros nativa
trait Error {
    fn message(self) -> string;
    fn cause(self) -> Option<Error>;
}

// Erros customizados
struct ParseError {
    message: string,
    line: int,
}
impl Error for ParseError {
    fn message(self) -> string { self.message }
    fn cause(self) -> Option<Error> { None }
}
```

---

## Beta 0.3 — Concorrência

**Objetivo:** Suporte a programação assíncrona e paralela com modelo seguro.

### async/await
```spectra
async fn fetch_data(url: string) -> string {
    // Simulates async I/O
    await read_url(url)
}

fn main() {
    let result = await fetch_data("https://example.com");
    println(result);
}
```

### Channels (CSP style, inspirado em Go)
```spectra
let (sender, receiver) = channel<int>();

spawn {
    sender.send(42);
};

let value = receiver.recv()?;
println(value);
```

### Primitivas de Sync
```spectra
let mutex = Mutex::new(0);
let guard = mutex.lock();
*guard = 42;
// Released when guard goes out of scope (via GC hooks)
```

---

## Beta 0.4 — Metaprogramação

**Objetivo:** Macros procedurais e `derive` automático para reduzir boilerplate.

### Macros de expressão
```spectra
macro_rules! vec {
    ($($x:expr),*) => {
        {
            let mut v = [];
            $(v.push($x);)*
            v
        }
    };
}

let nums = vec![1, 2, 3, 4, 5];
```

### Derive
```spectra
#[derive(Debug, Clone, Eq, Hash)]
struct Point {
    x: int,
    y: int,
}
```

### Reflexão básica (em avaliação)
```spectra
let type_name = typeof(value);
let fields = reflect::fields<MyStruct>();
```

---

## 1.0 — Stable Release

**Objetivo:** Linguagem production-ready com ecossistema completo de tooling.

### Tooling
- [ ] **spectra** CLI completo (build, run, test, bench, doc, fmt)
- [x] **spectra-fmt** formatter (infraestrutura existe, precisa completar)
- [ ] **spectra-lsp** Language Server Protocol para editores
- [ ] **spectra-pkg** Package manager (integrado no CLI)
- [ ] **VSCode extension** completa (syntax highlighting, LSP, debugging)

### Package Manager
```toml
# spectra.toml
[package]
name = "meu-projeto"
version = "1.0.0"
edition = "2025"

[dependencies]
spectra-json = "1.0"
spectra-http = "0.5"
```

### Compilação AOT
- Suporte a compilação antecipada (ahead-of-time) além do JIT atual
- Target: Linux x86_64, Windows x86_64, macOS arm64/x86_64, WebAssembly

---

## Feature Backlog Completo

### Prioridade Alta (Beta 0.1–0.2)

| Feature | Tipo | Complexidade | Notas |
|---------|------|--------------|-------|
| Char literals `'a'` | Syntax | Baixa | Lexer + AST |
| F-strings `f"..."` | Syntax | Média | Lexer + parser + lowering |
| Closures `|x| expr` | Syntax | Alta | AST + lowering + JIT |
| Operador `?` | Syntax | Média | Try desugar em lowering |
| Range `..` / `..=` | Syntax | Média | Range type + for-in |
| `if let` / `while let` | Syntax | Média | AST desugar = match |
| Struct enum variants `Variant { field: T }` | Syntax | Média | AST + parser + lowering |
| Import aliasing `import x as y` | Syntax | Baixa | Parser |
| Named imports `import { a, b } from x` | Syntax | Baixa | Parser |
| `Result<T,E>` built-in | Stdlib | Alta | Enum + methods |
| `Option<T>` built-in | Stdlib | Média | Enum + methods |
| `std.string` métodos | Stdlib | Média | Runtime FFI |
| `std.array` métodos | Stdlib | Média | Runtime FFI |
| Conversões de tipo (`to_int`, etc.) | Stdlib | Baixa | Runtime FFI |

### Prioridade Média (Beta 0.3–0.4)

| Feature | Tipo | Complexidade |
|---------|------|--------------|
| async/await | Runtime | Muito Alta |
| Channels | Runtime | Alto |
| Spawn/threads | Runtime | Alto |
| Macros | Metaprog | Muito Alta |
| `#[derive]` | Metaprog | Alta |
| Pattern matching exaustivo em struct variants | Semantic | Média |
| Walrus operator `:=` | Syntax | Baixa |
| String slice `s[0..5]` | Syntax | Média |
| Destructuring em let `let (a, b) = tuple;` | Syntax | Média |
| Multiple assignment `a, b = b, a` | Syntax | Média |

### Prioridade Baixa (1.0+)

| Feature | Tipo | Complexidade |
|---------|------|--------------|
| Reflexão em runtime | Runtime | Alta |
| AOT compilation | Compiler | Muito Alta |
| Plugins / extensões | Compiler | Alta |
| FFI com C/Rust | Runtime | Alta |
| WebAssembly target | Backend | Alta |
| REPL interativo | Tooling | Média |
| Debugger integrado | Tooling | Alta |
| Profiling | Tooling | Alta |

---

## Decisões Arquiteturais

| Decisão | Escolha | Justificativa |
|---------|---------|---------------|
| Runtime | GC puro | Elimina ownership/borrow checker, mais acessível a iniciantes |
| Backend JIT | Cranelift | Zero dependência de LLVM, compila Rust nativo |
| IR | SSA custom (midend) | Controle total sobre otimizações |
| Generics | Monomorphization | Performance máxima, sem boxing |
| Traits | Estáticos (vtable-free) | Resolvidos em compile-time, sem overhead |
| Strings | UTF-8 heap-allocated | Compatibilidade universal, boas primitivas |
| Números | `int` = i64, `float` = f64 | Simplificação deliberada (sem u8, i32, etc.) |

---

## Comparação com Linguagens Principais

| Feature | SpectraLang | Rust | Go | Kotlin | Python |
|---------|-------------|------|----|--------|--------|
| Tipagem | Estática | Estática | Estática | Estática | Dinâmica |
| GC | Puro | ❌ (ownership) | Puro | JVM GC | Puro (CPython) |
| Generics | ✅ | ✅ | ✅ (Go 1.18+) | ✅ | ✅ (typing) |
| Traits/Interfaces | ✅ | ✅ | ✅ (interfaces) | ✅ | ✅ (ABC) |
| Pattern matching | ✅ | ✅ | Parcial | ✅ | ✅ (3.10+) |
| Closures | 🔄 Beta 0.1 | ✅ | ✅ | ✅ | ✅ |
| async/await | 🔄 Beta 0.3 | ✅ | via goroutines | ✅ | ✅ |
| f-strings | 🔄 Beta 0.1 | ❌ (format!) | ❌ (fmt.Sprintf) | ✅ (string templates) | ✅ |
| Operador `?` | 🔄 Beta 0.1 | ✅ | ❌ | ❌ | ❌ |
| JIT | ✅ | ❌ | ❌ | JVM JIT | PyPy |
| Macros | 🔄 Beta 0.4 | ✅ | ❌ | ❌ | ❌ |
| Syntax sugar | ✅ | Médio | Mínimo | Alto | Alto |

---

## Notas de Desenvolvimento

### Convenções de Código (Compilador em Rust)

- **Lexer**: `compiler/src/lexer/mod.rs` — tokenizar → `Vec<Token>`
- **Parser**: `compiler/src/parser/` — tokens → AST
- **Semantic**: `compiler/src/semantic/mod.rs` — AST → tipos/erros
- **Lowering**: `midend/src/lowering.rs` — AST → IR (SSA)
- **Backend**: `backend/src/codegen.rs` — IR → Cranelift → machine code
- **Runtime**: `runtime/src/` — funções host expostas ao JIT

### Regra de Extensão de AST

Toda vez que um novo `ExpressionKind` ou `StatementKind` é adicionado a `compiler/src/ast/mod.rs`, **obrigatoriamente** devem ser atualizados:
1. `midend/src/lowering.rs` → `lower_expression` / `lower_statement`
2. `midend/src/lowering.rs` → `infer_expr_ir_type` (para expressões)
3. `compiler/src/semantic/mod.rs` → `analyze_expression` / `analyze_statement`

Handlers podem ser stubs (`todo!()` ou `eprintln!("TODO")`) enquanto a feature está sendo desenvolvida.

### Comandos Úteis

```powershell
# Build completo
cargo build

# Executar testes
cargo test

# Executar arquivo .spectra específico
cargo run --bin spectra-cli -- run examples/basic.spectra

# Rodar script de testes de regressão
.\run_tests.ps1
```

---

_Última atualização: 2025 — Beta 0.1 em progresso ativo._
