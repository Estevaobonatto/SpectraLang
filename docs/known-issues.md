# Bug Report: While Loops com Assignments

## Status: ✅ RESOLVIDO

## Descrição

Loops `while` que contêm assignments de variáveis (`x = x + 1`) agora funcionam corretamente com a implementação de Memory SSA.

## Problema

O sistema atual usa uma abordagem simples de SSA (Static Single Assignment) onde:
- `let x = 10` mapeia `x` → Value{0}
- `x = x + 1` cria novo Value{1} e remapeia `x` → Value{1}

Porém, em loops, isso não funciona porque:
1. O valor inicial está fora do loop
2. O valor atualizado está dentro do loop
3. Precisamos de um **PHI node** no header do loop para escolher entre eles

## Exemplo que Falha

```spectra
fn factorial(n: int) -> int {
    let result = 1;  // result → Value{0}
    let i = 1;       // i → Value{1}
    
    while i <= n {
        result = result * i;  // Cria Value{2}, mas Value{0} não está disponível no loop!
        i = i + 1;            // Cria Value{3}, mas Value{1} não está disponível no loop!
    }
    
    return result;
}
```

## Solução Necessária

Implementar **Memory SSA** ou **PHI nodes para loops**:

### Opção 1: Memory SSA (Recomendado)
```rust
// Entry:
let result_ptr = alloca i64;
store 1, result_ptr;

// Loop header:
let result_val = load result_ptr;  // PHI implícito através da memória
// ... usar result_val ...

// Loop body:
let new_result = mul result_val, i;
store new_result, result_ptr;
```

### Opção 2: PHI Nodes Explícitos
```rust
// Loop header:
let result_phi = phi [Value{0} from entry], [Value{2} from body];
let i_phi = phi [Value{1} from entry], [Value{3} from body];
// ... condição ...

// Loop body:
let result_new = mul result_phi, i_phi;
let i_new = add i_phi, 1;
// ... atualizar PHI nodes ...
```

## Workaround Atual

Por enquanto, loops com assignments simples funcionam, mas após otimização o código pode ser incorreto.

**Recomendação**: Usar apenas loops com iteradores ou implementar Memory SSA antes de usar em produção.

## Arquivos Afetados

- `midend/src/lowering.rs` - função `lower_statement()` linhas 108-120
- `midend/src/builder.rs` - precisa suporte a Alloca/Load/Store
- `midend/src/passes/` - passes de otimização podem remover código necessário

## Prioridade

🔴 **ALTA** - Afeta funcionalidade básica de loops mutáveis

## Estimativa de Esforço

- Memory SSA: ~2-3 dias de desenvolvimento
- PHI nodes explícitos: ~1-2 dias de desenvolvimento
- Testes: ~1 dia

**Total**: 3-4 dias de trabalho

## Referências

- [LLVM Memory SSA](https://llvm.org/docs/MemorySSA.html)
- [SSA Construction](https://www.cs.cmu.edu/~fp/courses/15411-f13/lectures/03-ssa.pdf)
- [Cranelift SSA](https://docs.rs/cranelift-frontend/)

## Próximos Passos

1. ✅ Documentar o bug
2. ⏳ Implementar Alloca/Load/Store no IRBuilder
3. ⏳ Atualizar lowering para usar memória para variáveis mutáveis
4. ⏳ Ajustar passes de otimização para entender Store/Load
5. ⏳ Adicionar testes para loops mutáveis
