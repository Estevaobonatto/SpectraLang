# Console I/O Quick Recipes

SpectraLang console projects now expose the `std.console` helpers through the runtime crate. While the execution backend is still under construction, you can already structure your source code using the same patterns the runtime will support.

## Echo program

```spectra
module app.main;

import std.console;

fn main(): i32 {
    println("Type something and press enter:");

    // `read_line` returns the text without the trailing newline.
    let user_input = read_line();

    println("You typed:");
    println(user_input);

    return 0;
}
```

## Handling command-line arguments

```spectra
module app.main;

import std.console;
import std.args;

fn main(): i32 {
    if std.args::is_empty() {
        println("No arguments provided.");
        return 0;
    }

    println("Arguments:");
    let list = std.args::all();
    let count = std.args::len();

    // The first entry is always the executable path.
    let index = 1;
    while index < count {
        println(list[index]);
        index = index + 1;
    }

    return 0;
}
```

> **Nota:** `spectra new` já inclui `import std.console;` no `main.spc` gerado e cria um stub em `src/std/console.spc` até que o runtime esteja conectado. A CLI ainda não executa os binários gerados, mas os exemplos acima servem como guia de como estruturar o código quando o backend de execução estiver disponível.
