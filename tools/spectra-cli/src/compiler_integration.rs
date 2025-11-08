// Full compiler integration
// Provides a backend driver that plugs midend + backend into the shared pipeline.

use std::collections::HashMap;
use std::fmt::Write as _;
use std::time::{Duration, Instant};

use spectra_backend::CodeGenerator;
use spectra_compiler::{
    error::MidendError, pipeline::CompilationMetrics, span::Span, BackendDriver, BackendError,
    lint::LintDiagnostic, CompilationOptions, CompilationPipeline, CompilationResult,
    CompilerError,
};
use spectra_midend::{
    ir::{pretty::format_module, Module as IRModule},
    lowering::ASTLowering,
    passes::{
        constant_folding::ConstantFolding, dead_code_elimination::DeadCodeElimination,
        validation::LoopStructureValidation, verification::verify_module, Pass,
    },
};

#[derive(Debug)]
struct PassReport {
    name: &'static str,
    duration: Duration,
    modified: bool,
}

#[derive(Default, Debug)]
struct PassAggregate {
    total_duration: Duration,
    runs: usize,
    modified_runs: usize,
}

#[derive(Default, Debug)]
struct AggregateMetrics {
    files: usize,
    lowering_total: Duration,
    codegen_total: Duration,
    front_total: Duration,
    lexing_total: Duration,
    parsing_total: Duration,
    semantic_total: Duration,
    backend_total: Duration,
    passes: HashMap<&'static str, PassAggregate>,
}

impl AggregateMetrics {
    fn new() -> Self {
        Self::default()
    }

    fn record(
        &mut self,
        artifacts: &FullPipelineArtifacts,
        front_metrics: Option<&CompilationMetrics>,
    ) {
        self.files += 1;
        self.lowering_total += artifacts.lowering_duration;
        self.codegen_total += artifacts.codegen_duration;

        if let Some(metrics) = front_metrics {
            self.front_total += metrics.total;
            self.lexing_total += metrics.lexing;
            self.parsing_total += metrics.parsing;
            self.semantic_total += metrics.semantic;
            self.backend_total += metrics.backend;
        }

        for pass in &artifacts.passes {
            let entry = self
                .passes
                .entry(pass.name)
                .or_insert_with(PassAggregate::default);
            entry.total_duration += pass.duration;
            entry.runs += 1;
            if pass.modified {
                entry.modified_runs += 1;
            }
        }
    }

    fn print_summary(&self) {
        if self.files == 0 {
            return;
        }

        fn average(duration: Duration, count: usize) -> Duration {
            duration.checked_div(count as u32).unwrap_or(Duration::ZERO)
        }

        println!(
            "\n📊 Aggregated build metrics ({} file{}):",
            self.files,
            if self.files == 1 { "" } else { "s" }
        );

        println!(
            "  • Front-end total: {:?} (avg {:?})",
            self.front_total,
            average(self.front_total, self.files)
        );
        println!(
            "  • Lexing total:    {:?} (avg {:?})",
            self.lexing_total,
            average(self.lexing_total, self.files)
        );
        println!(
            "  • Parsing total:   {:?} (avg {:?})",
            self.parsing_total,
            average(self.parsing_total, self.files)
        );
        println!(
            "  • Semantic total:  {:?} (avg {:?})",
            self.semantic_total,
            average(self.semantic_total, self.files)
        );
        println!(
            "  • Backend total:   {:?} (avg {:?})",
            self.backend_total,
            average(self.backend_total, self.files)
        );
        println!(
            "  • Lowering total:  {:?} (avg {:?})",
            self.lowering_total,
            average(self.lowering_total, self.files)
        );
        println!(
            "  • Codegen total:   {:?} (avg {:?})",
            self.codegen_total,
            average(self.codegen_total, self.files)
        );

        if !self.passes.is_empty() {
            println!("  • Optimization passes:");
            let mut entries: Vec<_> = self.passes.iter().collect();
            entries.sort_by(|a, b| b.1.total_duration.cmp(&a.1.total_duration));
            for (name, data) in entries {
                println!(
                    "      - {:<24} {:?} total (runs: {}, modified: {})",
                    name, data.total_duration, data.runs, data.modified_runs
                );
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct PassSummary {
    pub name: &'static str,
    pub duration: Duration,
    pub modified: bool,
}

#[derive(Debug, Clone)]
pub struct ModulePipelineSummary {
    pub filename: String,
    pub lowering_duration: Duration,
    pub codegen_duration: Duration,
    pub frontend_metrics: Option<CompilationMetrics>,
    pub passes: Vec<PassSummary>,
}

#[derive(Debug)]
struct FullPipelineArtifacts {
    ir_module: IRModule,
    passes: Vec<PassReport>,
    lowering_duration: Duration,
    codegen_duration: Duration,
}

struct FullPipelineBackend {
    codegen: Option<CodeGenerator>,
}

impl FullPipelineBackend {
    fn new() -> Self {
        Self { codegen: None }
    }
}

impl BackendDriver for FullPipelineBackend {
    type Artifacts = FullPipelineArtifacts;

    fn run(
        &mut self,
        ast: &spectra_compiler::ast::Module,
        options: &CompilationOptions,
    ) -> Result<Self::Artifacts, Vec<CompilerError>> {
        let mut lowering = ASTLowering::new();
        let lowering_start = Instant::now();
        let mut ir_module = lowering.lower_module(ast);
        let lowering_duration = lowering_start.elapsed();

        let mut pass_reports = Vec::new();

        let verification_start = Instant::now();
        if let Err(errors) = verify_module(&ir_module) {
            let ir_errors = errors
                .into_iter()
                .map(|msg| CompilerError::Midend(MidendError::new(msg)))
                .collect();
            return Err(ir_errors);
        }
        pass_reports.push(PassReport {
            name: "IR Verification (pre-opt)",
            duration: verification_start.elapsed(),
            modified: false,
        });

        if options.dump_ir {
            println!("=== IR (before optimization) ===");
            println!("{}", format_module(&ir_module));
            println!();
        }

        if options.optimize {
            if options.opt_level >= 1 {
                let mut cf = ConstantFolding::new();
                let pass_start = Instant::now();
                let modified = cf.run(&mut ir_module);
                pass_reports.push(PassReport {
                    name: "Constant Folding",
                    duration: pass_start.elapsed(),
                    modified,
                });
            }

            if options.opt_level >= 2 {
                let mut dce = DeadCodeElimination::new();
                let pass_start = Instant::now();
                let modified = dce.run(&mut ir_module);
                pass_reports.push(PassReport {
                    name: "Dead Code Elimination",
                    duration: pass_start.elapsed(),
                    modified,
                });
            }
        }

        let mut loop_check = LoopStructureValidation::new();
        let validation_start = Instant::now();
        loop_check.run(&mut ir_module);

        if loop_check.has_errors() {
            let errors: Vec<CompilerError> = loop_check
                .take_errors()
                .into_iter()
                .map(|err| {
                    CompilerError::Midend(MidendError::new(format!(
                        "Loop validation failed in function '{}' at block {} ('{}'): {}",
                        err.function, err.header_block, err.header_label, err.message
                    )))
                })
                .collect();
            return Err(errors);
        }

        pass_reports.push(PassReport {
            name: "Loop Structure Validation",
            duration: validation_start.elapsed(),
            modified: false,
        });

        let verify_after_start = Instant::now();
        if let Err(errors) = verify_module(&ir_module) {
            let ir_errors = errors
                .into_iter()
                .map(|msg| CompilerError::Midend(MidendError::new(msg)))
                .collect();
            return Err(ir_errors);
        }
        pass_reports.push(PassReport {
            name: "IR Verification (post-opt)",
            duration: verify_after_start.elapsed(),
            modified: false,
        });

        if options.dump_ir {
            println!("=== IR (after optimization) ===");
            println!("{}", format_module(&ir_module));
            println!();
        }

        let mut codegen = CodeGenerator::new();
        let codegen_start = Instant::now();
        let codegen_result = codegen.generate_module(&ir_module);
        let codegen_duration = codegen_start.elapsed();

        if let Err(error) = codegen_result {
            return Err(vec![CompilerError::Backend(BackendError::new(error))]);
        }

        self.codegen = Some(codegen);

        Ok(FullPipelineArtifacts {
            ir_module,
            passes: pass_reports,
            lowering_duration,
            codegen_duration,
        })
    }

    fn execute(
        &mut self,
        artifacts: &Self::Artifacts,
        _options: &CompilationOptions,
    ) -> Result<(), Vec<CompilerError>> {
        let codegen = match self.codegen.as_mut() {
            Some(codegen) => codegen,
            None => {
                println!(
                    "\n⚠️ Backend artifacts missing code generator; JIT execution unavailable"
                );
                return Ok(());
            }
        };

        if !artifacts
            .ir_module
            .functions
            .iter()
            .any(|func| func.name == "main")
        {
            println!("\n⚠️ No entry point 'main' found; skipping execution");
            return Ok(());
        }

        let runtime_state = spectra_runtime::initialize();
        // Ensure stdlib host calls are registered before bridging into JITed code.
        spectra_runtime::register_standard_library();
        let execution_start = Instant::now();

        let return_value = unsafe { codegen.execute_entry_point("main", &artifacts.ir_module) };
        let execution_duration = execution_start.elapsed();

        // Ensure manual allocations do not leak across invocation boundaries.
        spectra_runtime::ffi::spectra_rt_manual_clear();

        let return_value =
            return_value.map_err(|err| vec![CompilerError::Backend(BackendError::new(err))])?;

        let runtime_uptime = runtime_state.uptime();
        let init_thread = runtime_state.init_thread_id();

        if let Some(value) = return_value {
            println!(
                "\n✅ Execution completed (JIT)\n   - main() returned {}\n   - execution time {:?}\n   - runtime uptime {:?} (init thread {:?})",
                value, execution_duration, runtime_uptime, init_thread
            );
        } else {
            println!(
                "\n✅ Execution completed (JIT)\n   - main() returned void\n   - execution time {:?}\n   - runtime uptime {:?} (init thread {:?})",
                execution_duration, runtime_uptime, init_thread
            );
        }

        Ok(())
    }
}

/// Complete compiler that integrates all phases
pub struct SpectraCompiler {
    options: CompilationOptions,
    pipeline: CompilationPipeline<FullPipelineBackend>,
    aggregate: Option<AggregateMetrics>,
    last_summary: Option<ModulePipelineSummary>,
    emit_internal_metrics: bool,
}

impl SpectraCompiler {
    pub fn new(options: CompilationOptions) -> Self {
        let aggregate = if options.collect_metrics {
            Some(AggregateMetrics::new())
        } else {
            None
        };

        let pipeline =
            CompilationPipeline::new(options.clone()).with_backend(FullPipelineBackend::new());

        Self {
            options,
            pipeline,
            aggregate,
            last_summary: None,
            emit_internal_metrics: true,
        }
    }

    /// Compile source code to native code
    pub fn compile(&mut self, source: &str, filename: &str) -> Result<(), String> {
        self.last_summary = None;
        println!("🚀 SpectraLang Compiler");
        println!("━━━━━━━━━━━━━━━━━━━━");
        println!();

        let compilation = self
            .pipeline
            .compile(source, filename)
            .map_err(|errors| render_errors(&errors, source, filename, "compilation"))?;

        let CompilationResult {
            backend_artifacts: artifacts,
            metrics,
            warnings,
            ..
        } = compilation;

        self.emit_lint_warnings(&warnings, filename);

        if let Some(aggregate) = self.aggregate.as_mut() {
            aggregate.record(&artifacts, metrics.as_ref());
        }

        if self.options.optimize {
            let modified_passes: Vec<_> = artifacts
                .passes
                .iter()
                .filter(|report| report.modified)
                .map(|report| report.name)
                .collect();

            if modified_passes.is_empty() {
                println!("Optimization passes applied: none (IR unchanged)");
            } else {
                println!(
                    "Optimization passes applied: {}",
                    modified_passes.join(", ")
                );
            }
        }

        if self.options.collect_metrics
            && self.emit_internal_metrics
            && !artifacts.passes.is_empty()
        {
            println!("Pass timings:");
            for report in &artifacts.passes {
                let status = if report.modified {
                    "modified"
                } else {
                    "no change"
                };
                println!(
                    "  • {:<28} {:>10?} ({})",
                    report.name, report.duration, status
                );
            }
        }

        if self.options.collect_metrics && self.emit_internal_metrics {
            println!("Lowering time: {:?}", artifacts.lowering_duration);
            println!("Code generation time: {:?}", artifacts.codegen_duration);
        }

        if self.emit_internal_metrics {
            if let Some(metrics) = metrics.as_ref() {
                println!("Front-end timings:");
                println!("  • Lexing:    {:?}", metrics.lexing);
                println!("  • Parsing:   {:?}", metrics.parsing);
                println!("  • Semantic:  {:?}", metrics.semantic);
                println!("  • Backend:   {:?}", metrics.backend);
                println!("  • Total:     {:?}", metrics.total);
            }
        }

        println!(
            "IR functions emitted: {}",
            artifacts.ir_module.functions.len()
        );

        let pass_summaries = artifacts
            .passes
            .iter()
            .map(|report| PassSummary {
                name: report.name,
                duration: report.duration,
                modified: report.modified,
            })
            .collect();

        self.last_summary = Some(ModulePipelineSummary {
            filename: filename.to_string(),
            lowering_duration: artifacts.lowering_duration,
            codegen_duration: artifacts.codegen_duration,
            frontend_metrics: metrics.clone(),
            passes: pass_summaries,
        });

        if self.options.run_jit {
            self.pipeline
                .execute_artifacts(&artifacts)
                .map_err(|errors| render_errors(&errors, source, filename, "execution"))?;
        }

        println!("✨ Compilation successful!");
        println!("━━━━━━━━━━━━━━━━━━━━");

        Ok(())
    }

    pub fn print_aggregate_summary(&self) {
        if let Some(aggregate) = &self.aggregate {
            aggregate.print_summary();
        }
    }

    pub fn take_last_summary(&mut self) -> Option<ModulePipelineSummary> {
        self.last_summary.take()
    }

    pub fn set_emit_internal_metrics(&mut self, emit: bool) {
        self.emit_internal_metrics = emit;
    }

    fn emit_lint_warnings(&self, warnings: &[LintDiagnostic], filename: &str) {
        if warnings.is_empty() {
            return;
        }

        println!("⚠️  lint warnings detected ({}):", warnings.len());
        for warning in warnings {
            println!(
                "warning[{}] {}:{}:{} {}",
                warning.rule.code(),
                filename,
                warning.span.start_location.line,
                warning.span.start_location.column,
                warning.message
            );

            if let Some(note) = &warning.note {
                println!("    note: {}", note);
            }
        }
        println!();
    }

    /// Compile and execute (JIT)
    #[allow(dead_code)]
    pub fn compile_and_execute(&mut self, source: &str) -> Result<(), String> {
        println!("🚀 SpectraLang Compiler");
        println!("━━━━━━━━━━━━━━━━━━━━");
        println!();

        let compilation = self
            .pipeline
            .compile(source, "<jit>")
            .map_err(|errors| render_errors(&errors, source, "<jit>", "compilation"))?;

        let CompilationResult {
            backend_artifacts: artifacts,
            metrics,
            warnings,
            ..
        } = compilation;

        self.emit_lint_warnings(&warnings, "<jit>");

        if self.options.optimize {
            let modified_passes: Vec<_> = artifacts
                .passes
                .iter()
                .filter(|report| report.modified)
                .map(|report| report.name)
                .collect();

            if modified_passes.is_empty() {
                println!("Optimization passes applied: none (IR unchanged)");
            } else {
                println!(
                    "Optimization passes applied: {}",
                    modified_passes.join(", ")
                );
            }
        }

        if self.options.collect_metrics && !artifacts.passes.is_empty() {
            println!("Pass timings:");
            for report in &artifacts.passes {
                let status = if report.modified {
                    "modified"
                } else {
                    "no change"
                };
                println!(
                    "  • {:<28} {:>10?} ({})",
                    report.name, report.duration, status
                );
            }
        }

        if self.options.collect_metrics {
            println!("Lowering time: {:?}", artifacts.lowering_duration);
            println!("Code generation time: {:?}", artifacts.codegen_duration);
        }

        if let Some(metrics) = metrics.as_ref() {
            println!("Front-end timings:");
            println!("  • Lexing:    {:?}", metrics.lexing);
            println!("  • Parsing:   {:?}", metrics.parsing);
            println!("  • Semantic:  {:?}", metrics.semantic);
            println!("  • Backend:   {:?}", metrics.backend);
            println!("  • Total:     {:?}", metrics.total);
        }

        println!(
            "IR functions emitted: {}",
            artifacts.ir_module.functions.len()
        );

        self.pipeline
            .execute_artifacts(&artifacts)
            .map_err(|errors| render_errors(&errors, source, "<jit>", "execution"))?;

        println!("✨ Compilation successful!");
        println!("━━━━━━━━━━━━━━━━━━━━");

        Ok(())
    }
}

fn render_errors(errors: &[CompilerError], source: &str, filename: &str, stage: &str) -> String {
    if errors.is_empty() {
        return format!("{} failed with no diagnostics.", capitalize(stage));
    }

    let mut output = String::new();
    let title = format!("{} errors:", capitalize(stage));
    let _ = writeln!(&mut output, "{}", title);

    for (idx, error) in errors.iter().enumerate() {
        if idx > 0 {
            output.push('\n');
        }
        output.push_str(&render_error(error, source, filename));
    }

    output
}

fn render_error(error: &CompilerError, source: &str, filename: &str) -> String {
    match error {
        CompilerError::Lexical(e) => render_span_diagnostic(
            "lexical",
            &e.message,
            &e.span,
            e.hint.as_deref(),
            e.context.as_deref(),
            source,
            filename,
        ),
        CompilerError::Parse(e) => render_span_diagnostic(
            "parse",
            &e.message,
            &e.span,
            e.hint.as_deref(),
            e.context.as_deref(),
            source,
            filename,
        ),
        CompilerError::Semantic(e) => render_span_diagnostic(
            "semantic",
            &e.message,
            &e.span,
            e.hint.as_deref(),
            e.context.as_deref(),
            source,
            filename,
        ),
        CompilerError::Midend(e) => {
            let mut buf = String::new();
            let _ = writeln!(&mut buf, "midend error: {}", e.message);
            buf
        }
        CompilerError::Backend(e) => {
            let mut buf = String::new();
            let _ = writeln!(&mut buf, "backend error: {}", e.message);
            buf
        }
    }
}

fn render_span_diagnostic(
    phase: &str,
    message: &str,
    span: &Span,
    hint: Option<&str>,
    context: Option<&str>,
    source: &str,
    filename: &str,
) -> String {
    let mut buf = String::new();
    let _ = writeln!(
        &mut buf,
        "{}:{}:{}: {} error: {}",
        filename, span.start_location.line, span.start_location.column, phase, message
    );

    if let Some(raw_line) = get_source_line(source, span.start_location.line) {
        let line_text = raw_line.trim_end_matches('\r');
        let gutter_width = span.start_location.line.to_string().len();
        let _ = writeln!(&mut buf, "{:>width$} |", "", width = gutter_width);
        let _ = writeln!(
            &mut buf,
            "{:>width$} | {}",
            span.start_location.line,
            line_text,
            width = gutter_width
        );

        if let Some(marker_line) = build_highlight_line(span, line_text) {
            let _ = writeln!(
                &mut buf,
                "{:>width$} | {}",
                "",
                marker_line,
                width = gutter_width
            );
        }
    }

    if span.start_location.line != span.end_location.line {
        let _ = writeln!(
            &mut buf,
            "  = note: spans multiple lines ({} → {})",
            span.start_location.line, span.end_location.line
        );
    }

    if let Some(context) = context {
        let _ = writeln!(&mut buf, "  = note: {}", context);
    }

    if let Some(hint) = hint {
        let _ = writeln!(&mut buf, "  = help: {}", hint);
    }

    buf
}

fn get_source_line<'a>(source: &'a str, line_number: usize) -> Option<&'a str> {
    if line_number == 0 {
        return None;
    }

    source.lines().nth(line_number.saturating_sub(1))
}

fn build_highlight_line(span: &Span, line_text: &str) -> Option<String> {
    if line_text.is_empty() {
        return None;
    }

    let total_chars = line_text.chars().count();
    let start_column = span.start_location.column.max(1);
    let mut start_index = start_column.saturating_sub(1);
    if start_index > total_chars {
        start_index = total_chars;
    }

    let end_column = if span.start_location.line == span.end_location.line {
        span.end_location.column.max(start_column)
    } else {
        total_chars + 1
    };

    let mut end_index = end_column.saturating_sub(1);
    if end_index < start_index {
        end_index = start_index;
    }
    if end_index > total_chars {
        end_index = total_chars;
    }

    let span_width = end_index.saturating_sub(start_index);
    let highlight_len = if span_width == 0 { 1 } else { span_width + 1 };

    let mut marker = String::new();
    for _ in 0..start_index {
        marker.push(' ');
    }
    for _ in 0..highlight_len {
        marker.push('^');
    }

    Some(marker)
}

fn capitalize(text: &str) -> String {
    let mut chars = text.chars();
    match chars.next() {
        Some(first) => {
            let mut result = first.to_uppercase().collect::<String>();
            result.push_str(chars.as_str());
            result
        }
        None => String::new(),
    }
}

impl Default for SpectraCompiler {
    fn default() -> Self {
        Self::new(CompilationOptions::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    use spectra_compiler::lint::LintOptions;

    #[test]
    fn test_end_to_end_simple() {
        let source = r#"
            module test;
            
            fn add(a: int, b: int) -> int {
                return a + b;
            }
            
            pub fn main() {
                let x = add(5, 3);
                return;
            }
        "#;

        let mut compiler = SpectraCompiler::default();
        let result = compiler.compile(source, "test.spectra");

        assert!(result.is_ok());
    }

    #[test]
    fn test_end_to_end_with_optimization() {
        let source = r#"
            module test;
            
            fn compute() -> int {
                let x = 10 + 20;
                let y = x * 2;
                return y;
            }
            
            pub fn main() {
                let result = compute();
                return;
            }
        "#;

        let options = CompilationOptions {
            optimize: true,
            opt_level: 2,
            dump_ir: false,
            dump_ast: false,
            run_jit: false,
            collect_metrics: false,
            experimental_features: HashSet::new(),
            lint: LintOptions::default(),
        };

        let mut compiler = SpectraCompiler::new(options);
        let result = compiler.compile(source, "test.spectra");

        assert!(result.is_ok());
    }

    #[test]
    fn test_end_to_end_control_flow() {
        let source = r#"
            module test;
            
            fn max(a: int, b: int) -> int {
                if a > b {
                    return a;
                } else {
                    return b;
                }
            }
            
            pub fn main() {
                let result = max(10, 20);
                return;
            }
        "#;

        let mut compiler = SpectraCompiler::default();
        let result = compiler.compile(source, "test.spectra");

        assert!(result.is_ok());
    }

    #[test]
    fn test_end_to_end_loop() {
        let source = r#"
            module test;
            
            fn factorial(n: int) -> int {
                let result = 1;
                let i = 1;
                
                while i <= n {
                    result = result * i;
                    i = i + 1;
                }
                
                return result;
            }
            
            pub fn main() {
                let result = factorial(5);
                return;
            }
        "#;

        let mut compiler = SpectraCompiler::default();
        let result = compiler.compile(source, "test.spectra");

        assert!(result.is_ok());
    }
}
