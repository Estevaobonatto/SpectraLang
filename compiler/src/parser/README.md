# Parser da SpectraLang

Este diretório contém a implementação modular do parser da linguagem SpectraLang.

## Estrutura de Arquivos

### `mod.rs` - Parser Principal

- **Responsabilidade**: Gerenciamento geral do processo de parsing
- **Conteúdo**:
  - Estrutura `Parser` principal com estado (tokens, posição, erros)
  - Métodos de navegação de tokens (`current`, `peek`, `advance`, `is_at_end`)
  - Métodos de verificação de tokens (`check`, `check_keyword`, `check_symbol`, `check_identifier`)
  - Métodos de consumo de tokens (`consume_keyword`, `consume_symbol`, `consume_identifier`)
  - Gerenciamento de erros (`error`, `error_at`)
  - Sincronização de erros (`synchronize`)

### `module.rs` - Parser de Módulos

- **Responsabilidade**: Parse de declarações de módulos e imports
- **Sintaxe suportada**:
  - `module <nome>;` - Declaração de módulo
  - `import path.to.module;` - Importação de módulos
  - `import path.to.module as alias;` - Import com alias explícito
  - `pub import path.to.module;` - Import público para reexportar símbolos

### `item.rs` - Parser de Items

- **Responsabilidade**: Parse de declarações de alto nível (funções, classes, traits)
- **Sintaxe suportada**:
  - `fn <nome>(<params>) [-> tipo] { <corpo> }` - Declaração de função
  - `pub fn ...` - Função pública
  - Parâmetros de função com tipos opcionais
  - Tipos de retorno opcionais
  - Blocos de código

### `statement.rs` - Parser de Statements

- **Responsabilidade**: Parse de declarações dentro de funções
- **Sintaxe suportada**:
  - `let <nome> [: tipo] [= expr];` - Declaração de variável
  - `return [expr];` - Retorno de função
  - Expressões como statements

### `expression.rs` - Parser de Expressões

- **Responsabilidade**: Parse de expressões
- **Sintaxe suportada**:
  - Literais: números, strings
  - Identificadores
  - Chamadas de função: `func(arg1, arg2, ...)`
  - Expressões agrupadas: `(expr)`
  
### `type_annotation.rs` - Parser de Tipos

- **Responsabilidade**: Parse de anotações de tipo
- **Sintaxe suportada**:
  - Tipos simples: `i32`, `String`
  - Tipos qualificados: `std.collections.HashMap`

## Arquitetura

O parser utiliza uma arquitetura de **Recursive Descent Parser** com as seguintes características:

1. **Modularidade**: Cada tipo de construção sintática tem seu próprio arquivo
2. **Recuperação de Erros**: Sistema de sincronização para continuar parsing após erros
3. **Tipos Fortemente Tipados**: Usa a AST definida em `ast/mod.rs`
4. **Navegação de Tokens**: Métodos auxiliares para facilitar o parse

## Fluxo de Parsing

```text
Parser::parse()
  └─> parse_module()
       ├─> parse_import() (module.rs)
       └─> parse_item() (item.rs)
            └─> parse_function()
                 ├─> parse_function_params()
                 ├─> parse_type_annotation() (type_annotation.rs)
                 └─> parse_block()
                      └─> parse_statement() (statement.rs)
                           ├─> parse_let_statement()
                           ├─> parse_return_statement()
                           └─> parse_expression() (expression.rs)
                                ├─> parse_call_expression()
                                └─> parse_primary_expression()
```

## Tratamento de Erros

O parser coleta todos os erros encontrados durante o parsing e retorna uma lista de `ParseError`. O método `synchronize()` é usado para recuperar de erros e continuar o parsing.

## Extensibilidade

Para adicionar novos recursos ao parser:

1. **Novos tipos de expressões**: Adicione em `expression.rs`
2. **Novos statements**: Adicione em `statement.rs`
3. **Novos items (classes, traits)**: Adicione em `item.rs`
4. **Novos operadores**: Implemente precedência em `expression.rs`

## Exemplo de Uso

```rust
use spectra_compiler::{Lexer, Parser};

let source = r#"
module example;

fn main() {
    let x = 42;
    return x;
}
"#;

let lexer = Lexer::new(source);
let tokens = lexer.tokenize().unwrap();
let parser = Parser::new(tokens);
let module = parser.parse().unwrap();
```
