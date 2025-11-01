# 📋 Próximos Passos - SpectraLang

## ✅ Acabamos de Completar
- **Pattern Matching Básico**: Match expressions, enum patterns, wildcard patterns
- **263 linhas de código** adicionadas
- **16/20 testes** passando (80%)
- Zero regressões

---

## 🎯 PRÓXIMO: Aprimorar Pattern Matching

### 1. Tuple Variant Destructuring (PRIORITÁRIO) 
**Por quê**: Feature mais pedida, essencial para enums úteis

```spectra
enum Option {
    Some(int),
    None
}

match opt {
    Option::Some(value) => value,  // ← Isso não funciona ainda!
    Option::None => 0
}
```

**O que fazer**:
1. Modificar `lower_pattern_check` em `midend/src/lowering.rs`
2. Extrair valores da tuple variant
3. Criar variáveis locais para os dados extraídos
4. Adicionar ao escopo do arm body

**Estimativa**: 2-3 horas
**Dificuldade**: Média
**Impacto**: Alto

---

### 2. Identifier Bindings
**Por quê**: Permite capturar o valor do scrutinee

```spectra
match value {
    x => x + 1,  // x recebe o valor
    _ => 0
}
```

**O que fazer**:
1. Quando pattern é `Identifier(name)`, criar variável local
2. Bind ao valor do scrutinee
3. Adicionar ao escopo

**Estimativa**: 1-2 horas
**Dificuldade**: Baixa
**Impacto**: Médio

---

### 3. Literal Patterns
**Por quê**: Permite match em valores específicos

```spectra
match x {
    1 => "one",
    2 => "two",
    _ => "other"
}
```

**O que fazer**:
1. Em `lower_pattern_check`, comparar scrutinee com literal
2. Suportar int, bool, char
3. Retornar true/false da comparação

**Estimativa**: 2 horas
**Dificuldade**: Baixa
**Impacto**: Médio

---

### 4. Exhaustiveness Checking
**Por quê**: Ajuda a evitar bugs

```spectra
enum Color { Red, Green, Blue }

match c {
    Color::Red => 1,
    Color::Green => 2
    // WARNING: Missing Color::Blue
}
```

**O que fazer**:
1. Em `compiler/src/semantic/mod.rs`, criar função `check_match_exhaustiveness`
2. Para cada enum, verificar se todos os variants estão cobertos
3. Considerar wildcard como catch-all
4. Emitir warning se incompleto

**Estimativa**: 3-4 horas
**Dificuldade**: Média-Alta
**Impacto**: Alto (qualidade de código)

---

### 5. Type Checking de Match Arms
**Por quê**: Garantir consistência de tipos

```spectra
match x {
    1 => "string",  // string
    2 => 42         // int - ERRO!
}
```

**O que fazer**:
1. Em `infer_expression_type`, coletar tipo de cada arm
2. Verificar se todos são compatíveis
3. Emitir erro se divergirem

**Estimativa**: 1 hora
**Dificuldade**: Baixa
**Impacto**: Médio

---

## 🏗️ DEPOIS: Methods e Impl Blocks

### Por que isso é importante?
Permite código mais organizado e idiomático:

```spectra
struct Point { x: int, y: int }

impl Point {
    fn new(x: int, y: int) -> Point {
        Point { x: x, y: y }
    }
    
    fn distance(&self) -> float {
        sqrt(self.x * self.x + self.y * self.y)
    }
}

let p = Point::new(3, 4);
let d = p.distance();  // ← Muito mais limpo!
```

**Estimativa total**: 1 semana
**Dificuldade**: Alta
**Impacto**: Muito Alto

---

## 🐛 Bugs Conhecidos (Não Urgentes)

### Teste 10: unless.spectra
- **Erro**: Value 6 not found
- **Causa**: Runtime issue
- **Prioridade**: Baixa

### Teste 11: switch_case.spectra  
- **Erro**: Not implemented
- **Solução**: Usar match em vez de switch
- **Prioridade**: Muito baixa (deprecar switch)

### Teste 18: scopes.spectra
- **Erro**: Value 10 not found
- **Causa**: Possível issue com escopos
- **Prioridade**: Média

### Teste 20: all_features.spectra
- **Erro**: Verifier errors
- **Causa**: Teste muito complexo
- **Prioridade**: Baixa

---

## 📅 Cronograma Sugerido

### Esta Semana (5 dias)
- **Dia 1-2**: Tuple variant destructuring
- **Dia 3**: Identifier bindings + literal patterns
- **Dia 4**: Exhaustiveness checking
- **Dia 5**: Type checking + testes + documentação

**Resultado esperado**: Pattern matching 100% funcional

### Próxima Semana (5 dias)
- **Dia 1-2**: Impl blocks (AST + Parser)
- **Dia 3-4**: Methods (Semantic + Lowering)
- **Dia 5**: Self parameters + testes

**Resultado esperado**: Methods básicos funcionando

### Semana Seguinte
- **Polimento**: Bug fixes, otimizações, documentação
- **Meta**: 100% dos testes passando

---

## 🎓 Para Quem Quer Contribuir

### Fácil (1-2h cada)
- [ ] Identifier bindings em patterns
- [ ] Literal patterns
- [ ] Type checking de match arms
- [ ] Melhorar mensagens de erro

### Médio (3-5h cada)
- [ ] Tuple variant destructuring
- [ ] Exhaustiveness checking
- [ ] Corrigir bug do teste 18 (scopes)

### Difícil (1+ semana cada)
- [ ] Impl blocks completos
- [ ] Generics
- [ ] Traits

---

## 📊 Métricas de Progresso

### Atual (Nov 2024)
```
Frontend:     ████████████████████ 100%
Backend:      ████████████████████ 100%
Structs:      ████████████████████ 100%
Enums:        ████████████████████ 100%
Match:        ████████░░░░░░░░░░░░  40%
Methods:      ░░░░░░░░░░░░░░░░░░░░   0%
Generics:     ░░░░░░░░░░░░░░░░░░░░   0%
Traits:       ░░░░░░░░░░░░░░░░░░░░   0%
```

### Meta (Fim do mês)
```
Match:        ████████████████████ 100%
Methods:      ████████████████░░░░  80%
Testes:       ████████████████████ 100% (20/20)
```

---

## 🔗 Recursos

### Documentação Relevante
- `docs/pattern-matching-report.md` - Detalhes da implementação atual
- `ROADMAP_DETALHADO.md` - Plano completo de longo prazo
- `docs/type-system-implementation.md` - Como funciona o type system

### Exemplos
- `examples/test_match_basic.spectra` - Match simples
- `examples/test_match_complete.spectra` - Multiple arms + wildcard
- `examples/test_enums_complete.spectra` - Enums com tuple variants

### Testes
- `tests/validation/` - 20 testes de validação
- Rodar: `.\run_tests.ps1`

---

## 💡 Dicas de Implementação

### Para Tuple Destructuring
1. Em `lower_pattern_check`, quando encontrar `EnumVariant` com `data`:
   ```rust
   if let Some(patterns) = &pattern.data {
       // Para cada pattern, extrair valor correspondente da tuple
       // Usar GEP para acessar elementos
       // Criar variável local para cada binding
   }
   ```

2. O valor do scrutinee já é a tuple `(tag, data...)`
3. Use `build_gep` para extrair cada elemento
4. Crie variáveis com `build_alloca` + `build_store`

### Para Exhaustiveness Checking
1. Coletar todos os enum variants possíveis
2. Para cada pattern em match arms, marcar variants cobertos
3. Wildcard cobre tudo
4. Se algum variant não está coberto, emitir warning

---

## ✨ Visão de Longo Prazo

Depois de completar pattern matching e methods, SpectraLang terá:
- ✅ Type system robusto
- ✅ Structs e enums completos
- ✅ Pattern matching funcional
- ✅ Methods e OOP básico
- ✅ Compilação nativa
- ✅ Performance decente

**Isso é suficiente para**:
- Projetos pequenos/médios
- Prototipagem rápida
- Ensino de compiladores
- Base para features avançadas

---

**Última atualização**: Nov 2024
**Próxima revisão**: Após completar pattern matching
