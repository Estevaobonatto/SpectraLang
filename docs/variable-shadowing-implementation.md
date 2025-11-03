# Variable Shadowing Implementation

**Data**: 02 de Janeiro de 2025  
**Status**: ✅ 100% COMPLETO  
**Testes**: 45/45 passando (100%) 🎉

## 🎯 Objetivo

Implementar suporte completo para **variable shadowing** (sombreamento de variáveis), permitindo que variáveis declaradas em escopos internos "escondam" temporariamente variáveis de mesmo nome em escopos externos.

## 📋 Problema Original

### Sintoma
Testes 18 (`18_scopes.spectra`) e 20 (`20_all_features.spectra`) falhavam com erro:
```
Code generation failed: Value 10 not found
```

### Causa Raiz
O sistema de lowering usava um simples `HashMap<String, Value>` para mapear nomes de variáveis para valores SSA:

```rust
pub struct ASTLowering {
    value_map: HashMap<String, Value>,  // ❌ Problema!
    // ...
}
```

**Por que isso era problemático:**
- HashMap permite apenas **uma entrada por chave**
- Quando uma variável é redeclarada: `value_map.insert(name, new_value)` **sobrescreve** o valor antigo
- Não há como "voltar" ao valor anterior ao sair do escopo
- Código como este quebra:
  ```spectra
  let x = 10;
  if condition {
      let x = 20;  // Sobrescreve x globalmente!
  }
  return x;  // Tenta usar x=10, mas só existe x=20
  ```

## 🔧 Solução: Scope Stack

### Arquitetura

Substituímos o HashMap simples por uma **pilha de escopos** (stack of scopes):

```rust
#[derive(Clone)]
struct ScopeStack {
    scopes: Vec<HashMap<String, Value>>,
}
```

**Conceito:**
- Cada HashMap representa um **escopo léxico**
- `Vec` implementa a **pilha** de escopos aninhados
- Escopo mais interno = último elemento do Vec
- Lookup percorre do mais interno para o mais externo

### Métodos Implementados

#### 1. `new()` - Inicialização
```rust
fn new() -> Self {
    Self {
        scopes: vec![HashMap::new()],  // Começa com escopo global
    }
}
```

#### 2. `push_scope()` - Entrar em bloco
```rust
fn push_scope(&mut self) {
    self.scopes.push(HashMap::new());  // Novo escopo vazio
}
```

**Quando chamar:**
- Ao entrar em blocos `if`, `elif`, `else`
- Ao entrar em loops `while`, `for`, `do-while`, `loop`
- Ao entrar em `switch` cases
- Ao entrar em blocos `unless`

#### 3. `pop_scope()` - Sair de bloco
```rust
fn pop_scope(&mut self) {
    if self.scopes.len() > 1 {  // Mantém escopo global
        self.scopes.pop();
    }
}
```

**Quando chamar:**
- Ao sair de qualquer bloco que criou escopo

#### 4. `insert()` - Declarar variável
```rust
fn insert(&mut self, name: String, value: Value) {
    if let Some(scope) = self.scopes.last_mut() {
        scope.insert(name, value);  // Adiciona no escopo atual
    }
}
```

**Comportamento:**
- Sempre insere no **escopo mais interno**
- Pode "shadowing" variáveis de escopos externos
- Não modifica escopos anteriores

#### 5. `get()` - Buscar variável
```rust
fn get(&self, name: &str) -> Option<Value> {
    // Busca do mais interno para o mais externo
    for scope in self.scopes.iter().rev() {
        if let Some(value) = scope.get(name) {
            return Some(*value);
        }
    }
    None  // Variável não encontrada em nenhum escopo
}
```

**Estratégia de busca:**
1. Começa no escopo mais interno (último no Vec)
2. Se encontrar, retorna imediatamente
3. Se não encontrar, vai para escopo mais externo
4. Repete até escopo global
5. Retorna `None` se não encontrar em lugar nenhum

#### 6. `clear()` - Reset completo
```rust
fn clear(&mut self) {
    self.scopes.clear();
    self.scopes.push(HashMap::new());  // Recria escopo global
}
```

**Quando usar:**
- Ao começar a processar uma nova função
- Cada função tem seu próprio conjunto de escopos

## 🔄 Integração com Lowering

### Modificação do `lower_block`

Antes:
```rust
fn lower_block(&mut self, statements: &[Statement], ir_func: &mut IRFunction) {
    for stmt in statements {
        self.lower_statement(stmt, ir_func);
    }
}
```

Depois:
```rust
fn lower_block(&mut self, statements: &[Statement], ir_func: &mut IRFunction) {
    self.lower_block_with_scope(statements, ir_func, true);
}

fn lower_block_with_scope(
    &mut self, 
    statements: &[Statement], 
    ir_func: &mut IRFunction, 
    create_scope: bool
) {
    if create_scope {
        self.value_map.push_scope();  // 📌 Entra no escopo
    }
    
    for stmt in statements {
        self.lower_statement(stmt, ir_func);
    }
    
    if create_scope {
        self.value_map.pop_scope();   // 📌 Sai do escopo
    }
}
```

**Vantagens:**
- `lower_block()` mantém API antiga (cria escopo por padrão)
- `lower_block_with_scope()` permite controle fino
- Todos os blocos agora automaticamente criam escopos

### Onde os Escopos São Criados

1. **If Statements**
   ```rust
   self.lower_block(&then_block.statements, ir_func);  // ✅ Cria escopo
   ```

2. **While Loops**
   ```rust
   self.lower_block(&while_stmt.body.statements, ir_func);  // ✅ Cria escopo
   ```

3. **For Loops**
   ```rust
   self.lower_block(&for_stmt.body.statements, ir_func);  // ✅ Cria escopo
   ```

4. **Switch Cases**
   ```rust
   self.lower_block(&case.body.statements, ir_func);  // ✅ Cria escopo por case
   ```

5. **Unless Statements**
   ```rust
   self.lower_block(&then_block.statements, ir_func);  // ✅ Cria escopo
   ```

## ✅ Exemplo de Funcionamento

### Código SpectraLang
```spectra
fn test() -> int {
    let x = 10;          // Escopo 0 (global da função)
    
    if true {
        let x = 20;      // Escopo 1 (if block)
        let y = 30;      // Escopo 1
    }                    // pop_scope() - volta para escopo 0
    
    return x;            // Encontra x=10 no escopo 0
}
```

### Trace de Execução

1. **Início da função:**
   ```
   scopes: [{}]                    // Escopo 0 vazio
   ```

2. **`let x = 10;`:**
   ```
   scopes: [{x: Value(10)}]        // x no escopo 0
   ```

3. **Entra no `if` (push_scope):**
   ```
   scopes: [
       {x: Value(10)},              // Escopo 0
       {}                           // Escopo 1 vazio
   ]
   ```

4. **`let x = 20;` (dentro do if):**
   ```
   scopes: [
       {x: Value(10)},              // Escopo 0 (x antigo)
       {x: Value(20)}               // Escopo 1 (x novo, shadowing!)
   ]
   ```

5. **`let y = 30;`:**
   ```
   scopes: [
       {x: Value(10)},              
       {x: Value(20), y: Value(30)} // y também no escopo 1
   ]
   ```

6. **Busca `x` (dentro do if):**
   - Procura no escopo 1 primeiro → **Encontra x=20** ✅
   - Retorna Value(20)

7. **Sai do `if` (pop_scope):**
   ```
   scopes: [{x: Value(10)}]        // Escopo 1 descartado!
   ```

8. **`return x;`:**
   - Procura no escopo 0 → **Encontra x=10** ✅
   - Retorna Value(10)

## 📊 Impacto nos Testes

### Antes da Implementação
```
43/45 testes passando (95.56%)

❌ Test 18 (scopes): Value 10 not found
❌ Test 20 (all_features): Verifier errors
```

### Depois da Implementação
```
45/45 testes passando (100%) 🎉

✅ Test 18 (scopes): PASSA
✅ Test 20 (all_features): PASSA
```

### Teste 18 - Código Específico
```spectra
fn test_scopes() -> int {
    let outer = 100;
    
    if true {
        let inner = 200;
        let outer = 300;  // Shadowing!
        // Aqui outer = 300
    }
    
    // Aqui outer = 100 novamente
    return outer;
}
```

**Por que funcionava antes:** ❌
- HashMap sobrescrevia outer=100 com outer=300
- Ao sair do if, outer=100 estava perdido

**Por que funciona agora:** ✅
- Escopo 1 cria novo outer=300 (shadowing)
- pop_scope() descarta escopo 1
- Escopo 0 ainda tem outer=100 intacto

## 🎓 Lições Aprendidas

### 1. Scoping é Fundamental
- Scoping correto é **requisito básico** para qualquer linguagem
- Não pode ser "implementado depois"
- Afeta geração de IR e código nativo

### 2. Stack > HashMap para Escopos
- HashMap: bom para símbolos **globais únicos**
- Stack: necessário para símbolos **locais com shadowing**
- Estrutura de dados correta = problema simples

### 3. Testes Descobrem Bugs Arquiteturais
- Teste 18 expôs **falha arquitetural**
- Não era bug de código, era design errado
- Refatoração valeu 100% de cobertura

### 4. Separação de Responsabilidades
- `lower_block()` gerencia escopos automaticamente
- `lower_block_with_scope()` permite controle manual
- Flexibilidade sem complexidade

## 🔍 Complexidade

### Tempo
- **Insert**: O(1) - acessa último escopo diretamente
- **Get**: O(d) onde d = profundidade de aninhamento
  - Típico: d < 5 (if dentro de while dentro de função)
  - Pior caso: O(10-20) para código muito aninhado
- **Push/Pop**: O(1) - operações de Vec

### Espaço
- O(v × d) onde:
  - v = número de variáveis por escopo
  - d = profundidade máxima de aninhamento
- Típico: 5 variáveis × 3 níveis = 15 entradas totais
- Muito menor que alternativas (tabela de símbolos global)

## 📚 Referências

### Arquivos Modificados
- `midend/src/lowering.rs`: Implementação completa do ScopeStack
  - Linhas 12-58: Struct e métodos do ScopeStack
  - Linhas 66-68: Mudança de `HashMap` para `ScopeStack`
  - Linhas 95-97: Inicialização com `ScopeStack::new()`
  - Linhas 278-293: `lower_block_with_scope()` com push/pop
  - Linha 647: Correção de pattern matching (`Some(&value)` → `Some(value)`)

### Testes de Validação
- `test_shadow.spectra`: Teste simples de shadowing
- `tests/validation/18_scopes.spectra`: Teste complexo com escopos aninhados
- `tests/validation/20_all_features.spectra`: Teste de integração completo

### Commits
- `7c2f035`: feat: Implement variable shadowing with scope stack (100% tests passing!)

## 🎯 Conclusão

A implementação de variable shadowing através de um **Scope Stack** foi:
- ✅ **Essencial** para atingir 100% de testes
- ✅ **Elegante** em design e implementação
- ✅ **Eficiente** em tempo e espaço
- ✅ **Completa** em funcionalidade

**Resultado:** De 43/45 (95.56%) para **45/45 (100%)** em uma única implementação! 🎉

---

*SpectraLang agora tem suporte completo para scoping léxico e variable shadowing, atingindo paridade com linguagens modernas como Rust, JavaScript e Python.*
