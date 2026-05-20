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
| Beta 0.1   | Literals & Syntax | ✅ Implementado    | CharLiteral, f-string, lambda, `?`, `if let`  |
| Beta 0.2   | Stdlib & Errors   | 🔄 Em progresso   | Módulo std, Result/Option, error handling      |
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
- [x] Visibilidade `pub` / `internal` / privado
- [x] Sistema de módulos multi-arquivo (registry, ModuleExports, cross-module calls)
- [x] Módulos stdlib virtuais (`std.io`, `std.math`, `std.collections`)
- [x] `pub import` para re-exportação e `internal` para visibilidade de pacote
- [x] Configuração de projeto via `spectra.toml`

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
- **Status**: ✅ Implementado end-to-end — parsing, semântica, lowering e backend (JIT) funcionais. Pattern check com extração de tag, bindings de payload e loop com back-edge correto estão completos. Validado em `examples/test_beta_if_let.spectra` e `examples/test_if_let_correctness.spectra`

### 📦 Struct-style Enum Variants
```spectra
enum Shape {
    Circle { radius: float },
    Rectangle { width: float, height: float },
    Point,
}
```
- **Arquivo**: `compiler/src/ast/mod.rs`, `compiler/src/parser/item.rs`
- **Status**: ✅ Implementado end-to-end — parsing, semântica, lowering e backend (JIT) funcionais; validado em `examples/test_beta_struct_enum_variants.spectra`

### 🗂️ Import Avançado e Sistema de Módulos Multi-arquivo

Imports nomeados, aliased e re-exports funcionam end-to-end com stdlib virtual integrada.

```spectra
import std.io;                             // stdlib virtual — expõe print, println, etc.
import math.utils as utils;                // alias de módulo
import { sin, cos, tan } from math.trig;  // named imports
pub import path.to.Module;                // re-exportar para consumidores externos

internal fn helper() -> int { 42 }        // visível só no mesmo pacote (mesmo name em spectra.toml)
```

- **Arquivos**: `compiler/src/parser/module.rs`, `compiler/src/semantic/module_registry.rs`, `compiler/src/semantic/builtin_modules.rs`, `midend/src/lowering.rs`, `tools/spectra-cli/src/config.rs`, `tools/spectra-cli/src/discovery.rs`
- **Status**: ✅ Implementado end-to-end — multi-arquivo, cross-module calls, stdlib virtual, `pub import`, `internal`, `spectra.toml`, auto-descoberta de fontes e ordenação topológica. Validado com `examples/test_multi_a.spectra` + `test_multi_b.spectra` (`square(7)` → 49).

**Componentes do sistema de módulos:**

| Componente | Arquivo | Descrição |
|-----------|---------|----------|
| `ModuleRegistry` | `compiler/src/semantic/module_registry.rs` | Registro global de exports por módulo |
| `ModuleExports` | `compiler/src/semantic/module_registry.rs` | Funções/tipos exportados por um módulo |
| Stdlib virtual | `compiler/src/semantic/builtin_modules.rs` | `std.io`, `std.math`, `std.collections` pré-registrados |
| `internal` keyword | `compiler/src/token.rs` | Visibilidade de pacote (acesso somente por módulos do mesmo `[project].name`) |
| `pub import` | `compiler/src/parser/module.rs` | Re-exporta um módulo importado para consumidores externos |
| `spectra.toml` | `tools/spectra-cli/src/config.rs` | Configuração de projeto com `[project]` section |
| Auto-descoberta | `tools/spectra-cli/src/discovery.rs` | Coleta `.spectra`/`.spc` de `src_dirs` recursivamente |
| Ordenação | `tools/spectra-cli/src/project.rs` | Topological sort de módulos por grafo de dependências |
| CodeGenerator persistente | `tools/spectra-cli/src/compiler_integration.rs` | Reutilizado entre compilações para manter linking cross-module |

**Formato de `spectra.toml`:**

```toml
[project]
name = "meu-projeto"
version = "1.0.0"
src_dirs = ["src"]
entry = "main.spectra"
```

### Status de Validação Atual

- ✅ `examples/test_beta_char_literals.spectra`
- ✅ `examples/test_beta_fstrings.spectra`
- ✅ `examples/test_beta_closures.spectra` no modo `check`
- ✅ `examples/test_beta_if_let.spectra`
- ✅ `examples/test_if_let_correctness.spectra` — valida bindings, else branch, contagem de iterações e soma via binding
- ✅ `examples/test_beta_ranges.spectra`
- ✅ `examples/test_beta_struct_enum_variants.spectra` — compila e executa com JIT
- ✅ `examples/test_beta_imports.spectra` — `std.io` resolvido via módulos stdlib virtuais; `print` e `println` funcionais
- ✅ `examples/test_multi_a.spectra` + `examples/test_multi_b.spectra` — chamadas cross-module validadas (`square(7)` → 49)

### Notas pós-implementação

- Todos os enum variants (unit, tuple-style e struct-style) agora usam **representação uniforme por ponteiro** para tagged tuples no heap, eliminando a ambiguidade entre inteiro-raw e ponteiro.
- O backend agora despacha ops aritméticas (`+`, `-`, `*`, `/`) e comparações (`==`, `!=`, `<`, `<=`, `>`, `>=`) para Cranelift float (`fadd`, `fmul`, `fcmp`, …) quando os operandos são `F64`, corrigindo operações sobre campos `float` de enums e structs.
- O `InstructionKind::Load` agora carrega o tipo destino (`ty: IRType`) — o builder expõe `build_load_typed(ptr, ty)` além do `build_load(ptr)` que usa `Int` como default.
- **`if let` / `while let`** agora são completamente funcionais no lowering: o padrão é checado via `lower_pattern_check` (comparação de tag do enum), bindings de payload são extraídos com `lower_pattern_bindings`, e o `while let` gera um loop SSA com back-edge correto para o header. `find_assigned_variables` foi estendido para escanear recursivamente os corpos de `IfLet` e `WhileLet`, garantindo que variáveis mutáveis dentro desses blocos recebam slots de alloca no frame da função.

---

## Beta 0.2 — Stdlib & Error Handling

**Objetivo:** Biblioteca padrão funcional e tratamento de erros idiomático.

### ✅ Result<T, E> e Option<T> built-ins

`Option<T>` e `Result<T, E>` agora são tipos **built-in pré-registrados** — nenhuma declaração de enum necessária.

```spectra
fn divide(a: int, b: int) -> Result<int, string> {
    if b == 0 { Result::Err("cannot divide by zero") }
    else { Result::Ok(a / b) }
}

fn find_positive(x: int) -> Option<int> {
    if x > 0 { Option::Some(x) }
    else { Option::None }
}

fn main() -> int {
    if let Result::Ok(v) = divide(10, 2) { ... }
    if let Option::Some(n) = find_positive(7) { ... }
    while let Option::Some(n) = get_next() { ... }
    return 0;
}
```

**Componentes implementados:**
- `TypeAnnotationKind::Generic { name, type_args }` — novo variante no AST para preservar `<T>` em anotações de tipo
- Parser armazena type args em `Generic` variante; `looks_like_type_args_in_annotation()` evita ambiguidade com `<` aritmético
- `lower_type_annotation_with_map` para `Generic` resolve `Option<int>` → `IRType::Enum { name: "Option_int" }`
- `current_function_return_annotation` propaga tipo declarado `-> Result<int, string>` para preencher args inferidos como "unknown" em construções de variantes
- Redefinição do usuário de `Option`/`Result` silenciosamente ignorada (sem regressão em exemplos existentes)
- **Validado:** `examples/test_beta_option_result.spectra` — PASSA (`main() returned 0`)

### ✅ std.string — Módulo de manipulação de strings

Funções disponíveis via `import std.string;` (chamadas com prefixo qualificado `std.string.fn()` ou como nome simples `fn()`):

| Função | Assinatura | Descrição |
|--------|-----------|-----------|
| `len` | `(s: string) -> int` | Comprimento em bytes |
| `trim` | `(s: string) -> string` | Remove espaços das extremidades |
| `to_upper` | `(s: string) -> string` | Converte para maiúsculas |
| `to_lower` | `(s: string) -> string` | Converte para minúsculas |
| `contains` | `(s, sub: string) -> bool` | Verifica se sub-string está presente |
| `starts_with` | `(s, prefix: string) -> bool` | Verifica prefixo |
| `ends_with` | `(s, suffix: string) -> bool` | Verifica sufixo |
| `concat` | `(a, b: string) -> string` | Concatena duas strings |
| `repeat_str` | `(s: string, n: int) -> string` | Repete string n vezes |
| `char_at` | `(s: string, i: int) -> int` | Código do caractere na posição i |

- **Validado:** `examples/test_beta_stdlib_string.spectra` — PASSA (`main() returned 0`)

### ✅ std.convert — Módulo de conversão de tipos

Funções disponíveis via `import std.convert;`:

| Função | Assinatura | Descrição |
|--------|-----------|-----------|
| `int_to_string` | `(v: int) -> string` | Inteiro para string |
| `float_to_string` | `(v: float) -> string` | Float para string |
| `bool_to_string` | `(v: bool) -> string` | Bool para string |
| `string_to_int` | `(s: string) -> int` | String para inteiro (0 se inválido) |
| `string_to_float` | `(s: string) -> float` | String para float (0.0 se inválido) |
| `int_to_float` | `(v: int) -> float` | Inteiro para float |
| `float_to_int` | `(v: float) -> int` | Float para inteiro (trunca) |

- **Validado:** coberto em `examples/test_beta_stdlib_string.spectra`

### ✅ Chamadas qualificadas a módulos stdlib

Identificadores de namespace de módulo (`std`, `std.string`, etc.) são agora reconhecidos pelo analisador semântico quando o módulo foi importado. Chamadas qualificadas como `std.string.len(x)` são aceitas como equivalente a `len(x)` após `import std.string;`.

**Infraestrutura:**
- `module_namespaces: HashSet<String>` em `SemanticAnalyzer` — populado ao processar imports
- `ExpressionKind::MethodCall` no lowering verifica primeiro se é chamada stdlib qualificada (via path resolution)
- `lookup_std_host_function` estendida com todos os casos de `string.*` e `convert.*`
- Backend `codegen.rs` suporta argumentos `F64` em `HostCall` via `bitcast` para `I64`

### Módulos de stdlib planejados (visão completa) (visão completa)

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
[project]
name = "meu-projeto"
version = "1.0.0"
src_dirs = ["src"]
entry = "main.spectra"

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
| Char literals `'a'` | Syntax | Baixa | ✅ Implementado |
| F-strings `f"..."` | Syntax | Média | ✅ Implementado |
| Closures `|x| expr` | Syntax | Alta | 🟡 Parser/semântica OK; lowering parcial |
| Operador `?` | Syntax | Média | 🟡 Sintaxe + lowering básico; falta `Result`/stdlib |
| Range `..` / `..=` | Syntax | Média | ✅ Implementado |
| `if let` / `while let` | Syntax | Média | ✅ Implementado |
| Struct enum variants `Variant { field: T }` | Syntax | Média | ✅ Implementado |
| Import aliasing `import x as y` | Syntax | Baixa | ✅ Implementado |
| Named imports `import { a, b } from x` | Syntax | Baixa | ✅ Implementado |
| `pub import` / `internal` / multi-módulos | Syntax/Infra | Alta | ✅ Implementado |
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
| Módulos | Registry centralizado, 3 níveis de visibilidade | `pub` (global), `internal` (pacote), privado; stdlib como módulos virtuais pré-registrados |
| JIT multi-módulo | `CodeGenerator` persistido entre módulos | Mantém `function_map` com FuncIds para linking cross-module |

---

## Comparação com Linguagens Principais

| Feature | SpectraLang | Rust | Go | Kotlin | Python |
|---------|-------------|------|----|--------|--------|
| Tipagem | Estática | Estática | Estática | Estática | Dinâmica |
| GC | Puro | ❌ (ownership) | Puro | JVM GC | Puro (CPython) |
| Generics | ✅ | ✅ | ✅ (Go 1.18+) | ✅ | ✅ (typing) |
| Traits/Interfaces | ✅ | ✅ | ✅ (interfaces) | ✅ | ✅ (ABC) |
| Pattern matching | ✅ | ✅ | Parcial | ✅ | ✅ (3.10+) |
| Closures | � Parcial | ✅ | ✅ | ✅ | ✅ |
| async/await | 🔄 Beta 0.3 | ✅ | via goroutines | ✅ | ✅ |
| f-strings | ✅ | ❌ (format!) | ❌ (fmt.Sprintf) | ✅ (string templates) | ✅ |
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

# Executar arquivo .spectra único
.\target\debug\spectralang.exe run examples/basic.spectra

# Executar projeto multi-arquivo (ordem = dependências primeiro)
.\target\debug\spectralang.exe run examples/mathutils.spectra examples/main_app.spectra

# Executar projeto via spectra.toml (auto-descoberta de src_dirs)
.\target\debug\spectralang.exe build

# Criar novo projeto com spectra.toml
.\target\debug\spectralang.exe new meu-projeto

# Verificar erros sem executar
.\target\debug\spectralang.exe check examples/basic.spectra

# Rodar script de testes de regressão
.\run_tests.ps1
```

---

_Última atualização: março de 2026 — Beta 0.1 concluído; sistema multi-módulos implementado; Beta 0.2 em planejamento._
