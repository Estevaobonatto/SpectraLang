# Known Limitations

## 1. Unless/If Expression Values with Memory SSA

**Status**: Known Limitation 🟡
**Priority**: LOW
**Workaround**: Available

### Description

When using `unless` or `if` as statements (not expressions) that modify variables through assignments, the current implementation attempts to create PHI nodes for expression values that don't exist in all branches.

### Example of Problem

```spectra
fn problematic() -> int {
    let result = 0;
    
    unless x < 0 {
        result = x * 2;  // Modifies via Store
    }
    
    return result;  // Works, but IR has unnecessary PHI
}
```

### Technical Details

The lowering code tries to capture expression values from if/unless blocks:

```rust
if let Some(Statement { kind: StatementKind::Expression(expr), .. }) = 
    then_block.statements.last() 
{
    unless_value = Some(self.lower_expression(expr, ir_func));
}
```

But with Memory SSA, assignments use `Store` instructions, and the generated Value only exists in that specific block. When trying to create a PHI node in the merge block with values from both branches, it fails verification if one branch doesn't produce a value.

### Root Cause

Mixing two paradigms:
1. **Memory SSA**: Variables are Alloca'd, assignments use Store
2. **Expression Values**: If/Unless try to return last expression value via PHI

The problem occurs when:
- A block has assignments (which go through memory)
- The last statement is an expression (which tries to return a value)
- The value is only produced in one branch

### Workaround

Use if-else instead of unless when modifying variables:

```spectra
fn working() -> int {
    let result = 0;
    
    if x >= 0 {  // Negated condition
        result = x * 2;
    }
    
    return result;  // Works fine
}
```

Or use unless as a pure expression (without assignments):

```spectra
fn also_working(x: int) -> int {
    let result = unless x < 0 { x * 2 } else { 0 };
    return result;
}
```

### Potential Solutions

#### Option 1: Detect Statement vs Expression Context
Modify lowering to detect when if/unless is used as:
- **Statement**: Don't try to capture expression values
- **Expression**: Must have values in all branches

```rust
// Check if parent expects a value
let is_expression_context = ...; // Determine from AST traversal

if is_expression_context {
    // Must have value from all branches
    if unless_value.is_none() || unless_else_value.is_none() {
        return error("Unless expression must have values in all branches");
    }
} else {
    // Statement context - don't create PHI
    return ir_func.next_value(); // Dummy value
}
```

#### Option 2: Use Memory for All Conditional Results
When if/unless is used as expression, allocate memory and store results:

```rust
let result_ptr = alloca Int;
if condition {
    let then_val = ...;
    store then_val, result_ptr;
} else {
    let else_val = ...;
    store else_val, result_ptr;
}
let result = load result_ptr;
```

#### Option 3: Remove Expression Context for Unless
Make unless always a statement, never an expression. Users must use if-else for expression context.

### Recommendation

**Option 1** is cleanest but requires AST context tracking.
**Option 2** is consistent with Memory SSA philosophy.
**Option 3** is simplest but reduces language expressiveness.

For now, the workaround is sufficient since:
- If-else works correctly
- Unless as pure expression works
- Only Unless with assignments + expression value is problematic

### Impact

**LOW** - Affects rare edge case:
- Unless with assignments in one branch
- Unless used where expression value is captured
- Unless without else clause

Most code uses if-else for variable modifications, which works correctly.

## 2. For Loop (C-Style) Not Implemented

**Status**: Not Implemented 🔴
**Priority**: MEDIUM

### Description

Traditional C-style for loops with init/condition/increment are not yet implemented:

```spectra
// NOT SUPPORTED:
for let i = 0; i < 10; i = i + 1 {
    // body
}
```

### Workaround

Use while loops:

```spectra
let i = 0;
while i < 10 {
    // body
    i = i + 1;
}
```

### Implementation Status

Currently implemented:
- ✅ While loops
- ✅ Do-while loops
- ✅ Infinite loops
- ✅ For-in loops (over collections)
- ❌ C-style for loops

### Future Work

C-style for loops can be desugared to while loops during parsing:

```rust
for init; condition; increment {
    body
}

// Becomes:
{
    init;
    while condition {
        body
        increment
    }
}
```
