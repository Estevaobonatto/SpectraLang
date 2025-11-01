# 🗺️ SpectraLang - Roadmap de Desenvolvimento

## Status Atual (Nov 2024)

### ✅ Completamente Implementado
- **Frontend completo**: Lexer, Parser, AST, Semantic Analysis
- **Type System**: Inferência e validação de tipos
- **Backend**: IR generation (SSA) + Cranelift codegen
- **Structs**: Definições, literais, acesso a campos, lowering
- **Enums**: Unit variants, tuple variants (sem destructuring ainda)
- **Pattern Matching**: Match expressions, enum patterns, wildcard patterns

### 📊 Taxa de Sucesso dos Testes
- **16/20 testes passando (80%)**
- Nenhuma regressão introduzida por pattern matching

---

## 🎯 Fase 3: Aprimoramentos de Pattern Matching (PRÓXIMA)

### Prioridade ALTA - Essencial para usabilidade

#### 3.1 Tuple Variant Destructuring (Estimativa: 2-3h)
**Objetivo**: Permitir extrair dados de enum variants
```spectra
enum Option {
    Some(int),
    None
}

match opt {
    Option::Some(value) => value,  // Extrair value
    Option::None => 0
}
```

**Tarefas**:
- [ ] Atualizar `lower_pattern_check` para suportar data extraction
- [ ] Criar variáveis locais para valores extraídos
- [ ] Adicionar testes com tuple variants

**Arquivos afetados**:
- `midend/src/lowering.rs` (~50 linhas)
- `compiler/src/semantic/mod.rs` (~20 linhas)

---

#### 3.2 Identifier Bindings (Estimativa: 1-2h)
**Objetivo**: Permitir bind do scrutinee a uma variável
```spectra
match value {
    x => x + 1,  // x recebe o valor de value
    _ => 0
}
```

**Tarefas**:
- [ ] Implementar bind em `lower_pattern_check`
- [ ] Adicionar variável ao escopo do arm body
- [ ] Testes com identifier bindings

**Arquivos afetados**:
- `midend/src/lowering.rs` (~30 linhas)
- `compiler/src/semantic/mod.rs` (~15 linhas)

---

#### 3.3 Literal Patterns (Estimativa: 2h)
**Objetivo**: Permitir match em valores literais
```spectra
match x {
    1 => "one",
    2 => "two",
    _ => "other"
}
```

**Tarefas**:
- [ ] Implementar pattern check para literais
- [ ] Suportar int, bool, char literals
- [ ] Adicionar testes

**Arquivos afetados**:
- `midend/src/lowering.rs` (~40 linhas)

---

#### 3.4 Exhaustiveness Checking (Estimativa: 3-4h)
**Objetivo**: Avisar quando match não cobre todos os casos
```spectra
enum Color { Red, Green, Blue }

// WARNING: Missing pattern: Color::Blue
match c {
    Color::Red => 1,
    Color::Green => 2
}
```

**Tarefas**:
- [ ] Analisar patterns vs enum variants
- [ ] Detectar wildcards que tornam match exaustivo
- [ ] Emitir warnings para casos faltantes
- [ ] Adicionar testes de exhaustiveness

**Arquivos afetados**:
- `compiler/src/semantic/mod.rs` (~100 linhas)
- Nova função: `check_match_exhaustiveness`

---

#### 3.5 Type Checking de Match Arms (Estimativa: 1h)
**Objetivo**: Garantir que todos os arms retornam tipos compatíveis
```spectra
// ERROR: Type mismatch in match arms
match x {
    1 => "string",  // string
    2 => 42         // int - ERRO!
}
```

**Tarefas**:
- [ ] Verificar tipo de cada arm
- [ ] Garantir compatibilidade entre todos os arms
- [ ] Emitir erro se tipos incompatíveis

**Arquivos afetados**:
- `compiler/src/semantic/mod.rs` (~30 linhas)

---

## 🏗️ Fase 4: Methods e Impl Blocks (IMPORTANTE)

### Prioridade MÉDIA - Necessário para OOP

#### 4.1 Impl Blocks Básicos (Estimativa: 4-5h)
**Objetivo**: Permitir definir métodos em structs/enums
```spectra
struct Point {
    x: int,
    y: int
}

impl Point {
    fn new(x: int, y: int) -> Point {
        Point { x: x, y: y }
    }
    
    fn distance(&self) -> float {
        sqrt(self.x * self.x + self.y * self.y)
    }
}

let p = Point::new(3, 4);
let d = p.distance();
```

**Tarefas**:
- [ ] AST: Item::Impl, ImplBlock, ImplItem
- [ ] Parser: parse_impl_block()
- [ ] Semantic: associar métodos a tipos
- [ ] Lowering: name mangling (Type_method)
- [ ] Syntax: dot notation (obj.method())

**Arquivos afetados**:
- `compiler/src/ast/mod.rs` (~40 linhas)
- `compiler/src/parser/item.rs` (~80 linhas)
- `compiler/src/semantic/mod.rs` (~120 linhas)
- `midend/src/lowering.rs` (~60 linhas)

---

#### 4.2 Self Parameters (Estimativa: 2-3h)
**Objetivo**: Suportar self, &self, &mut self
```spectra
impl Point {
    fn move_by(&mut self, dx: int, dy: int) {
        self.x = self.x + dx;
        self.y = self.y + dy;
    }
}
```

**Tarefas**:
- [ ] Parser: reconhecer self parameters
- [ ] Semantic: validar uso de self
- [ ] Lowering: passar self como primeiro argumento

**Arquivos afetados**:
- `compiler/src/parser/item.rs` (~30 linhas)
- `compiler/src/semantic/mod.rs` (~50 linhas)
- `midend/src/lowering.rs` (~40 linhas)

---

## 📚 Fase 5: Coleções e Arrays (ÚTIL)

### Prioridade BAIXA - Pode esperar

#### 5.1 Arrays de Tamanho Fixo (Estimativa: 6-8h)
```spectra
let arr: [int; 5] = [1, 2, 3, 4, 5];
let first = arr[0];
arr[2] = 10;
```

**Tarefas**:
- [ ] Type: Array { element_type, size }
- [ ] Parser: array literals, index syntax
- [ ] Lowering: array allocation, GEP instructions
- [ ] Runtime: bounds checking

---

#### 5.2 Slices (Estimativa: 4-5h)
```spectra
fn sum(arr: &[int]) -> int {
    // ...
}
```

**Tarefas**:
- [ ] Type: Slice (fat pointer: ptr + len)
- [ ] Coerção array → slice
- [ ] Range indexing: arr[1..3]

---

## 🔧 Fase 6: Recursos Avançados (FUTURO)

### Prioridade MUITO BAIXA

#### 6.1 Genéricos (Estimativa: 15-20h)
```spectra
struct Box<T> {
    value: T
}

fn identity<T>(x: T) -> T {
    x
}
```

**Desafio**: Monomorphization vs type erasure

---

#### 6.2 Traits (Estimativa: 20-25h)
```spectra
trait Drawable {
    fn draw(&self);
}

impl Drawable for Circle {
    fn draw(&self) {
        // ...
    }
}
```

**Desafio**: Trait objects, vtables

---

#### 6.3 Closures (Estimativa: 12-15h)
```spectra
let add = |x, y| x + y;
let result = add(1, 2);
```

**Desafio**: Capture de variáveis, environment

---

## 🐛 Manutenção e Bugs

### Testes Falhando (4/20)

#### Bug 1: unless.spectra (Teste 10)
**Erro**: Value 6 not found
**Possível causa**: Verifier ou runtime issue
**Prioridade**: BAIXA

#### Bug 2: switch_case.spectra (Teste 11)
**Erro**: Feature não implementada
**Solução**: Implementar switch/case ou deprecar
**Prioridade**: BAIXA (match já cobre este caso)

#### Bug 3: scopes.spectra (Teste 18)
**Erro**: Value 10 not found
**Possível causa**: Escopo ou lifetime issue
**Prioridade**: MÉDIA

#### Bug 4: all_features.spectra (Teste 20)
**Erro**: Verifier errors
**Possível causa**: Combinação de features
**Prioridade**: BAIXA (teste muito complexo)

---

## 📅 Cronograma Sugerido

### Sprint 1 (1 semana) - Pattern Matching
- [x] Match básico (FEITO)
- [ ] Tuple destructuring
- [ ] Identifier bindings
- [ ] Literal patterns
- [ ] Exhaustiveness checking
- [ ] Type checking

### Sprint 2 (1 semana) - Methods
- [ ] Impl blocks básicos
- [ ] Associated functions
- [ ] Self parameters
- [ ] Dot notation

### Sprint 3 (1 semana) - Arrays
- [ ] Fixed-size arrays
- [ ] Array literals
- [ ] Index syntax
- [ ] Bounds checking

### Sprint 4 (2 semanas) - Polimento
- [ ] Corrigir bugs dos testes
- [ ] Otimizações de performance
- [ ] Melhorar mensagens de erro
- [ ] Documentação completa

---

## 🎯 Objetivos de Longo Prazo

### v0.1 (Atual) - MVP
✅ Compilador funcional
✅ Types básicos
✅ Control flow
✅ Functions
✅ Structs & Enums
✅ Pattern Matching básico

### v0.2 (Próxima) - Usabilidade
🔄 Pattern matching completo
⏳ Methods e impl blocks
⏳ Arrays básicos
⏳ Error messages melhores

### v0.3 (Futuro) - Avançado
⏳ Genéricos
⏳ Traits
⏳ Closures
⏳ Module system
⏳ Package manager

### v1.0 (Longo prazo) - Produção
⏳ Standard library completa
⏳ Async/await
⏳ FFI (C interop)
⏳ Debugger support
⏳ IDE integration

---

## 📊 Métricas de Progresso

### Atual
- **Completude**: ~40% das features planejadas
- **Estabilidade**: 80% dos testes passando
- **Performance**: Não otimizado (debug mode)
- **Documentação**: Boa (8 documentos)

### Meta v0.2
- **Completude**: ~60%
- **Estabilidade**: >90%
- **Performance**: Otimizações básicas
- **Documentação**: Excelente

---

## 🤝 Contribuindo

Se você quer ajudar, estas são as áreas prioritárias:

1. **Pattern Matching**: Tuple destructuring é a feature mais pedida
2. **Methods**: Essencial para código mais limpo
3. **Bug Fixes**: Ajude a chegar em 100% dos testes
4. **Documentação**: Exemplos e tutoriais
5. **Performance**: Profiling e otimizações

---

## 📝 Notas Técnicas

### Decisões de Design

**Por que match em vez de switch?**
- Mais expressivo e type-safe
- Permite destructuring
- Comum em linguagens modernas (Rust, Swift, Kotlin)

**Por que structs em vez de classes?**
- Mais simples e previsível
- Composition over inheritance
- Alinhado com sistemas modernos

**Por que SSA IR?**
- Facilita otimizações
- Cranelift já usa SSA
- Análise de data flow mais fácil

### Performance Considerations

- **Match**: Linear search (TODO: jump tables)
- **Structs**: Pass by value (TODO: referencias)
- **Enums**: Untagged unions (TODO: tagged)
- **Calls**: No inlining ainda

---

## 🔗 Links Úteis

- **Documentação**: `/docs`
- **Exemplos**: `/examples`
- **Testes**: `/tests`
- **Relatórios**: 
  - `pattern-matching-report.md`
  - `progress-report.md`
  - `type-system-implementation.md`

---

**Última atualização**: Nov 2024
**Versão do Roadmap**: 1.0
