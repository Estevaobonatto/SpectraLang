# ⚡ Quick Start Guide - SpectraLang

## 🎯 Para Você que Acabou de Chegar

**Tempo de leitura**: 5 minutos
**Objetivo**: Começar a contribuir o mais rápido possível

---

## 📖 O Que É SpectraLang?

Uma linguagem de programação compilada com:
- ✅ Type system robusto com inferência
- ✅ Structs e Enums
- ✅ Pattern matching
- ✅ Compilação para código nativo (via Cranelift)
- ✅ Sintaxe moderna inspirada em Rust/Swift

---

## 🚀 Setup (2 minutos)

### 1. Instalar Rust
```bash
# Windows (PowerShell como admin)
winget install Rustlang.Rust.MSVC

# Ou via rustup.rs
```

### 2. Clonar e Build
```bash
git clone <repo>
cd SpectraLang
cargo build
```

### 3. Testar
```bash
.\run_tests.ps1
# Esperado: 16/20 testes passando (80%)
```

### 4. Executar Exemplo
```bash
cargo run -- examples/test_match_complete.spectra
# Deve compilar com sucesso!
```

---

## 📚 Documentação Essencial

### Leia AGORA (10 min total)
1. **[PROXIMOS_PASSOS.md](PROXIMOS_PASSOS.md)** - O que fazer agora (3 min)
2. **[README_VISUAL.md](README_VISUAL.md)** - Visão geral (5 min)
3. **[docs/syntax-guide.md](docs/syntax-guide.md)** - Sintaxe (2 min)

### Leia DEPOIS (quando precisar)
- **[pattern-matching-report.md](docs/pattern-matching-report.md)** - Exemplo de feature completa
- **[ROADMAP_DETALHADO.md](ROADMAP_DETALHADO.md)** - Plano de longo prazo
- **[type-system-implementation.md](docs/type-system-implementation.md)** - Como funciona tipos

### Índice Completo
- **[INDICE_DOCUMENTACAO.md](INDICE_DOCUMENTACAO.md)** - Todos os documentos

---

## 🎯 Primeira Contribuição (Escolha Uma)

### 🟢 Fácil (1-2h cada)
Perfeito para começar!

#### 1. Identifier Bindings em Patterns
**O que é**: Permitir `x => x + 1` em match
**Onde**: `midend/src/lowering.rs` linha ~1170
**Como**: Ver seção "Identifier Bindings" em [PROXIMOS_PASSOS.md](PROXIMOS_PASSOS.md)

#### 2. Literal Patterns
**O que é**: Permitir `1 => "one"` em match  
**Onde**: `midend/src/lowering.rs` linha ~1170
**Como**: Ver seção "Literal Patterns" em [PROXIMOS_PASSOS.md](PROXIMOS_PASSOS.md)

#### 3. Type Checking de Match Arms
**O que é**: Garantir que todos arms retornam mesmo tipo
**Onde**: `compiler/src/semantic/mod.rs` linha ~486
**Como**: Ver seção "Type Checking" em [PROXIMOS_PASSOS.md](PROXIMOS_PASSOS.md)

---

### 🟡 Médio (3-5h cada)
Requer entender melhor o código

#### 4. Tuple Variant Destructuring
**O que é**: Permitir `Option::Some(value) => value`
**Onde**: `midend/src/lowering.rs` linha ~1170
**Impacto**: ALTO - feature muito pedida
**Como**: Ver [PROXIMOS_PASSOS.md](PROXIMOS_PASSOS.md)

#### 5. Exhaustiveness Checking
**O que é**: Avisar quando match não cobre todos os casos
**Onde**: `compiler/src/semantic/mod.rs` (nova função)
**Impacto**: ALTO - melhora qualidade do código
**Como**: Ver [PROXIMOS_PASSOS.md](PROXIMOS_PASSOS.md)

---

### 🔴 Difícil (1+ semana)
Para quem já conhece o projeto

#### 6. Impl Blocks
**O que é**: Permitir definir métodos em structs/enums
**Impacto**: MUITO ALTO - essencial para OOP
**Como**: Ver [ROADMAP_DETALHADO.md](ROADMAP_DETALHADO.md)

---

## 💡 Workflow Recomendado

### 1. Escolher Tarefa
Veja lista acima ou TODO list no VS Code

### 2. Ler Documentação Relevante
- Para pattern matching: `pattern-matching-report.md`
- Para types: `type-system-implementation.md`
- Para backend: `backend-implementation-complete.md`

### 3. Estudar Código Existente
Exemplo para patterns:
```bash
# Ver como enum variant pattern funciona
code compiler/src/parser/expression.rs:608
code midend/src/lowering.rs:1170
```

### 4. Implementar
- Escreva código limpo
- Adicione comentários
- Siga padrão existente

### 5. Testar
```bash
# Criar arquivo de teste
echo "..." > examples/test_minha_feature.spectra

# Compilar
cargo run -- examples/test_minha_feature.spectra

# Rodar suite completa
.\run_tests.ps1
```

### 6. Documentar
- Atualizar `PROXIMOS_PASSOS.md` se completou uma tarefa
- Criar relatório se feature grande (exemplo: `pattern-matching-report.md`)

---

## 🔍 Estrutura do Código

```
SpectraLang/
├── compiler/           # Frontend
│   ├── src/lexer/     # Tokenização
│   ├── src/parser/    # Parse (6 arquivos modulares)
│   ├── src/semantic/  # Type checking + validation
│   └── src/ast/       # AST definitions
├── midend/            # IR
│   └── src/           # SSA IR + lowering
├── backend/           # Codegen
│   └── src/           # Cranelift wrapper
├── runtime/           # Runtime support
├── examples/          # Código de exemplo
├── tests/             # Suite de testes
└── docs/              # Documentação técnica
```

---

## 🎓 Aprender Fazendo

### Tutorial 1: Adicionar Novo Operador (30 min)
1. Lexer: Reconhecer token
2. Parser: Parse expression
3. AST: Adicionar variant
4. Semantic: Type checking
5. Lowering: Gerar IR
6. Testar!

**Exemplo real**: Ver como `=>` (FatArrow) foi adicionado:
- `compiler/src/lexer/mod.rs` linha 164
- `compiler/src/token.rs` linha 138
- `compiler/src/parser/expression.rs` linha 579

### Tutorial 2: Adicionar Nova Feature (2-3h)
Ver `pattern-matching-report.md` - exemplo completo de:
- Design decisions
- Implementation steps
- Testing strategy
- Documentation

---

## 🐛 Encontrou um Bug?

### 1. Verificar se é Conhecido
Veja `known-issues.md`

### 2. Reproduzir
Criar exemplo mínimo em `examples/bug_report.spectra`

### 3. Diagnosticar
```bash
# Executar com verbose
cargo run -- examples/bug_report.spectra

# Ver IR gerado (se passar do parser)
cargo run -- examples/bug_report.spectra --emit-ir
```

### 4. Reportar
Abrir issue com:
- Código que reproduz
- Output esperado vs atual
- Versão do compilador

---

## 💬 Onde Buscar Ajuda?

1. **Documentação** (`INDICE_DOCUMENTACAO.md`)
2. **Exemplos** (`examples/`)
3. **Testes** (`tests/validation/`)
4. **Código similar** (grep/search)
5. **Issues/Discussões** (GitHub)

---

## ✨ Dicas de Ouro

### 🔥 Para Implementar Features
1. **Copie padrões existentes** - Veja como features similares foram feitas
2. **Teste incrementalmente** - Não implemente tudo de uma vez
3. **Documente enquanto codifica** - Será útil depois

### 🧠 Para Entender o Código
1. **Siga o fluxo de dados** - Lexer → Parser → Semantic → IR → Codegen
2. **Use exemplos** - Execute com breakpoints/prints
3. **Leia testes** - Mostram uso real

### 🚀 Para Ser Produtivo
1. **Configure VS Code** com Rust Analyzer
2. **Use `cargo check`** (mais rápido que `cargo build`)
3. **Rode testes frequentemente**

---

## 📊 Métricas de Sucesso

Você está indo bem quando:
- ✅ Consegue compilar código simples
- ✅ Entende o fluxo frontend → backend
- ✅ Consegue adicionar features simples
- ✅ Testes continuam passando
- ✅ Código está documentado

---

## 🎯 Metas de Curto Prazo

### Esta Semana
- [ ] Completar pattern matching (destructuring + bindings)
- [ ] Chegar em 18/20 testes passando

### Próxima Semana
- [ ] Impl blocks básicos
- [ ] Methods funcionando
- [ ] 100% dos testes passando

---

## 🔗 Links Rápidos

- **TODO List**: Ver no VS Code (Ctrl+Shift+P → "Todo Tree")
- **Próximos passos**: [PROXIMOS_PASSOS.md](PROXIMOS_PASSOS.md)
- **Roadmap completo**: [ROADMAP_DETALHADO.md](ROADMAP_DETALHADO.md)
- **Todos os docs**: [INDICE_DOCUMENTACAO.md](INDICE_DOCUMENTACAO.md)

---

## 🎉 Pronto Para Começar?

1. ✅ Setup feito?
2. ✅ Testes rodando?
3. ✅ Leu PROXIMOS_PASSOS.md?

**Então escolha uma tarefa e comece!** 🚀

Veja a lista de tarefas fáceis acima ou em [PROXIMOS_PASSOS.md](PROXIMOS_PASSOS.md).

**Boa sorte e bom código!** 💻✨

---

**Última atualização**: Nov 2024
**Para dúvidas**: Ver índice de documentação ou abrir issue
