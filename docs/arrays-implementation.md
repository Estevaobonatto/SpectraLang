# Implementação de Arrays no SpectraLang

**Data**: 1 de Novembro de 2025  
**Status**: ✅ Completo e Funcional

## Visão Geral

Arrays são coleções homogêneas de elementos de tamanho fixo. A implementação suporta:
- Literais de array: `[1, 2, 3, 4, 5]`
- Indexação para leitura: `arr[i]`
- Indexação para escrita: `arr[i] = value`
- Inferência automática de tipos
- Arrays em qualquer contexto, incluindo loops

## Sintaxe

### Declaração e Inicialização
```spectra
// Literal de array
let numbers = [1, 2, 3, 4, 5];
let names = ["Alice", "Bob", "Charlie"];

// Tipo inferido automaticamente
// numbers: [int; 5]
// names: [string; 3]
```

### Acesso a Elementos
```spectra
let arr = [10, 20, 30, 40, 50];
let x = arr[0];  // x = 10
let y = arr[2];  // y = 30
```

### Modificação de Elementos
```spectra
let arr = [1, 2, 3];
arr[0] = 99;  // arr = [99, 2, 3]
arr[2] = 77;  // arr = [99, 2, 77]
```

### Arrays em Loops
```spectra
fn sum_array() -> int {
    let arr = [1, 2, 3, 4, 5];
    let sum = 0;
    let i = 0;
    
    while i < 5 {
        sum = sum + arr[i];
        i = i + 1;
    }
    
    return sum;  // 15
}
```

### Modificação em Loops
```spectra
fn fill_array() -> int {
    let arr = [0, 0, 0, 0, 0];
    let i = 0;
    
    // Preencher array
    while i < 5 {
        arr[i] = i * 10;
        i = i + 1;
    }
    
    // arr = [0, 10, 20, 30, 40]
    return arr[3];  // 30
}
```

## Implementação Técnica

### 1. Parser (`compiler/src/parser/expression.rs`)

#### Array Literals
```rust
fn parse_array_literal(&mut self) -> Result<Expression, String> {
    self.expect_symbol('[')?;
    let mut elements = Vec::new();
    
    while !self.check_symbol(']') {
        elements.push(self.parse_expression()?);
        if !self.check_symbol(']') {
            self.expect_symbol(',')?;
        }
    }
    
    self.expect_symbol(']')?;
    Ok(Expression::new(ExpressionKind::ArrayLiteral { elements }))
}
```

#### Array Indexing
```rust
// arr[index] é tratado como postfix expression
while self.check_symbol('[') {
    self.expect_symbol('[')?;
    let index = self.parse_expression()?;
    self.expect_symbol(']')?;
    expr = Expression::new(ExpressionKind::IndexAccess {
        target: Box::new(expr),
        index: Box::new(index),
    });
}
```

### 2. AST (`compiler/src/ast/mod.rs`)

#### Tipos
```rust
pub enum Type {
    Array {
        element_type: Box<Type>,
        size: Option<usize>,
    },
    // ... outros tipos
}
```

#### LValue para Assignments
```rust
pub enum LValue {
    Identifier(String),
    IndexAccess {
        target: String,
        index: Box<Expression>,
    },
}
```

#### Expressões
```rust
pub enum ExpressionKind {
    ArrayLiteral {
        elements: Vec<Expression>,
    },
    IndexAccess {
        target: Box<Expression>,
        index: Box<Expression>,
    },
    // ... outras expressões
}
```

### 3. Análise Semântica (`compiler/src/semantic/mod.rs`)

#### Type Inference de Arrays
```rust
ExpressionKind::ArrayLiteral { elements } => {
    let mut element_types = Vec::new();
    for elem in elements {
        element_types.push(self.infer_expression_type(elem)?);
    }
    
    // Verificar consistência
    if !element_types.is_empty() {
        let first = &element_types[0];
        for ty in &element_types[1..] {
            if ty != first {
                return Err("Array elements must have same type");
            }
        }
    }
    
    Type::Array {
        element_type: Box::new(element_types[0].clone()),
        size: Some(elements.len()),
    }
}
```

#### Validação de Index Access
```rust
ExpressionKind::IndexAccess { target, index } => {
    let target_type = self.infer_expression_type(target)?;
    let index_type = self.infer_expression_type(index)?;
    
    // Índice deve ser int
    if index_type != Type::Int {
        return Err("Array index must be int");
    }
    
    // Target deve ser array
    match target_type {
        Type::Array { element_type, .. } => Ok(*element_type),
        _ => Err("Cannot index non-array type"),
    }
}
```

### 4. IR Generation (`midend/src/lowering.rs`)

#### Array Literals
```rust
ExpressionKind::ArrayLiteral { elements } => {
    // Alocar espaço no stack
    let array_type = Type::Array {
        element_type: Box::new(elem_type),
        size: Some(elements.len()),
    };
    let alloca = self.builder.build_alloca(array_type);
    
    // Armazenar cada elemento
    for (i, elem) in elements.iter().enumerate() {
        let elem_val = self.lower_expression(elem)?;
        let index = self.builder.build_const_int(i as i64);
        let ptr = self.builder.build_getelementptr(
            alloca, index, elem_type
        );
        self.builder.build_store(ptr, elem_val);
    }
    
    // Registrar no array_map (não no alloca_map)
    self.array_map.insert(name.clone(), alloca);
    Ok(alloca)
}
```

#### Index Access (Load)
```rust
ExpressionKind::IndexAccess { target, index } => {
    let array_ptr = self.lower_expression(target)?;
    let index_val = self.lower_expression(index)?;
    
    let elem_ptr = self.builder.build_getelementptr(
        array_ptr, index_val, element_type
    );
    
    self.builder.build_load(elem_ptr)
}
```

#### Index Access (Store)
```rust
LValue::IndexAccess { target, index } => {
    let array_ptr = self.array_map.get(target)
        .ok_or("Array not found")?;
    let index_val = self.lower_expression(index)?;
    
    let elem_ptr = self.builder.build_getelementptr(
        *array_ptr, index_val, element_type
    );
    
    self.builder.build_store(elem_ptr, value);
}
```

### 5. Backend - Solução SSA (`backend/src/codegen.rs`)

#### Problema Original
Cranelift SSA verifier rejeitava código onde `stack_addr` values cruzavam block boundaries:
```
entry:
  v0 = stack_addr ss0, 0  // StackSlot para array
  jump while_header
  
while_body:
  v1 = load v0  // ❌ v0 não domina este uso!
```

#### Solução: StackSlot Storage
```rust
InstructionKind::Alloca { result, ty } => {
    let stack_slot = builder.create_sized_stack_slot(...);
    
    // APENAS arrays são armazenados no stack_slot_map
    if matches!(ty, IRType::Array { .. }) {
        stack_slot_map.insert(result.id, stack_slot);
    }
    
    // Gerar stack_addr para uso imediato
    let addr = builder.ins().stack_addr(types::I64, stack_slot, 0);
    value_map.insert(result.id, addr);
}
```

#### Regeneração de stack_addr no GetElementPtr
```rust
InstructionKind::GetElementPtr { ptr, index, ... } => {
    // Checar se ptr é de um array (precisa regeneração)
    let ptr_val = if let Some(&stack_slot) = stack_slot_map.get(&ptr.id) {
        // Regenerar stack_addr NO BLOCO ATUAL
        builder.ins().stack_addr(types::I64, stack_slot, 0)
    } else {
        get_value(ptr)?
    };
    
    // Calcular pointer arithmetic
    let offset = builder.ins().imul(index_val, elem_size_val);
    let result = builder.ins().iadd(ptr_val, offset);
    value_map.insert(result.id, result);
}
```

#### Por que funciona?
1. **StackSlot**: É function-scoped, válido em todos os blocos
2. **stack_addr**: É block-scoped, gerado localmente onde necessário
3. **Regeneração**: Cada bloco que precisa acessar o array gera seu próprio `stack_addr`
4. **SSA Dominance**: Satisfeita porque valores são gerados antes de serem usados no mesmo bloco

## Exemplos Completos

### Exemplo 1: Array Simples
```spectra
fn test() -> int {
    let arr = [1, 2, 3, 4, 5];
    return arr[2];  // 3
}
```

### Exemplo 2: Modificação
```spectra
fn test() -> int {
    let arr = [10, 20, 30];
    arr[1] = 99;
    return arr[1];  // 99
}
```

### Exemplo 3: Loop com Arrays
```spectra
fn test() -> int {
    let arr = [1, 2, 3, 4, 5];
    let sum = 0;
    let i = 0;
    
    while i < 5 {
        sum = sum + arr[i];
        i = i + 1;
    }
    
    return sum;  // 15
}
```

### Exemplo 4: Loops Aninhados
```spectra
fn test() -> int {
    let arr = [1, 2, 3, 4, 5];
    let sum = 0;
    let i = 0;
    
    while i < 5 {
        let j = 0;
        while j < i {
            sum = sum + arr[j];
            j = j + 1;
        }
        i = i + 1;
    }
    
    return sum;
}
```

### Exemplo 5: Preenchimento e Soma
```spectra
fn test() -> int {
    let arr = [0, 0, 0, 0, 0];
    let i = 0;
    
    // Preencher
    while i < 5 {
        arr[i] = i * 10;
        i = i + 1;
    }
    
    // Somar
    let sum = 0;
    i = 0;
    while i < 5 {
        sum = sum + arr[i];
        i = i + 1;
    }
    
    return sum;  // 100
}
```

## Limitações Atuais

1. **Tamanho Fixo**: Arrays têm tamanho fixo em tempo de compilação
2. **Bounds Checking**: Não há verificação de limites em tempo de execução
3. **Multi-dimensional**: Arrays multidimensionais não implementados ainda

## Próximos Passos para Arrays

1. **Dynamic Arrays**: Implementar Vec<T> com crescimento dinâmico
2. **Bounds Checking**: Adicionar verificação de limites opcional
3. **Array Methods**: `.len()`, `.push()`, `.pop()`, etc.
4. **Slices**: Suporte para `arr[0..3]`
5. **Multi-dimensional**: `[[1,2],[3,4]]`

## Arquivos Modificados

- `compiler/src/ast/mod.rs` - Type::Array, LValue, ExpressionKind
- `compiler/src/parser/expression.rs` - Array literal e indexing parsing
- `compiler/src/parser/statement.rs` - LValue assignment parsing
- `compiler/src/semantic/mod.rs` - Type inference e validation
- `midend/src/ir.rs` - GetElementPtr instruction, IRType::Array
- `midend/src/lowering.rs` - array_map, array literal lowering
- `midend/src/builder.rs` - build_getelementptr method
- `backend/src/codegen.rs` - stack_slot_map, StackSlot regeneration
- `midend/src/passes/dead_code_elimination.rs` - GetElementPtr usage tracking

## Testes

Todos os testes passam com sucesso:
- ✅ `examples/test_arrays.spectra` - 3 funções (simple, assignment, loop)
- ✅ `examples/test_loop_array.spectra` - Array em while loop
- ✅ `examples/test_complex_arrays.spectra` - Loops aninhados e modificações

## Conclusão

A implementação de arrays está **completa e funcional**, incluindo o caso complexo de arrays dentro de loops. A solução usando `StackSlot` storage e regeneração de `stack_addr` resolve elegantemente o problema de SSA dominance do Cranelift, permitindo que arrays sejam usados em qualquer contexto sem restrições.
