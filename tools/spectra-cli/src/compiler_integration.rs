// Full compiler integration
// Provides a backend driver that plugs midend + backend into the shared pipeline.

use std::collections::HashMap;
use std::fmt::Write as _;
use std::time::{Duration, Instant};

use spectra_backend::{AotCodeGenerator, AotOptions, CodeGenerator};
use spectra_compiler::{
    error::MidendError, lint::LintDiagnostic, pipeline::CompilationMetrics, span::Span,
    BackendDriver, BackendError, CompilationOptions, CompilationPipeline, CompilationResult,
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

// Thread-local that propagates the Spectra program's return value (used as exit
// code) back to the CLI without changing the BackendDriver trait signature.
thread_local! {
    static LAST_EXEC_EXIT: std::cell::Cell<Option<i32>> =
        const { std::cell::Cell::new(None) };
}

/// Returns and clears the exit code stored by the last JIT execution, if any.
pub fn take_last_exec_exit() -> Option<i32> {
    LAST_EXEC_EXIT.with(|cell| cell.replace(None))
}

/// Sets the program arguments forwarded to `std.env` host functions.
pub fn forward_program_args(args: Vec<String>) {
    spectra_runtime::set_program_args(args);
}

#[derive(Debug)]
struct PassReport {
    name: &'static str,
    duration: Duration,
    modified: bool,
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

struct CompilationReport {
    artifacts: FullPipelineArtifacts,
    metrics: Option<CompilationMetrics>,
    warnings: Vec<LintDiagnostic>,
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

#[derive(Debug)]
struct FullPipelineArtifacts {
    ir_module: IRModule,
    passes: Vec<PassReport>,
    lowering_duration: Duration,
    codegen_duration: Duration,
}

struct FullPipelineBackend {
    codegen: Option<CodeGenerator>,
    /// When `true`, the "Execution completed" summary is suppressed so that
    /// only the Spectra program's own stdout/stderr is visible.
    quiet_execution: bool,
}

impl FullPipelineBackend {
    fn new() -> Self {
        Self {
            codegen: None,
            quiet_execution: false,
        }
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
        let mut ir_module = match lowering.lower_module(ast) {
            Ok(module) => module,
            Err(errors) => {
                return Err(errors
                    .into_iter()
                    .map(|e| CompilerError::Midend(e))
                    .collect());
            }
        };
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

        // Reuse the same CodeGenerator (and its underlying JITModule) across all
        // modules in a project build.  This keeps every previously compiled function
        // in the JIT's function_map so that cross-module calls (e.g. main_app
        // calling square() from mathutils) can be resolved correctly.
        let codegen = self.codegen.get_or_insert_with(CodeGenerator::new);
        let codegen_start = Instant::now();
        let codegen_result = codegen.generate_module(&ir_module);
        let codegen_duration = codegen_start.elapsed();

        if let Err(error) = codegen_result {
            return Err(vec![CompilerError::Backend(BackendError::new(error))]);
        }

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
        options: &CompilationOptions,
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
            // Library modules have no entry point — skip JIT execution silently.
            // The caller is responsible for ensuring that at least one module in
            // the project defines `main`; see execute_plan_with_options().
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

        // Store the program's exit code so the CLI can propagate it.
        let exit_code = return_value.map(|v| v as i32).unwrap_or(0);
        LAST_EXEC_EXIT.with(|cell| cell.set(Some(exit_code)));

        // Print the execution summary only when the user asked for timing data
        // or when quiet_execution has not been requested.
        if !self.quiet_execution || options.collect_metrics {
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
    emit_output: bool,
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
            emit_output: true,
        }
    }

    /// Set the package name so the semantic analyzer can enforce `internal` visibility.
    pub fn set_package_name(&mut self, name: impl Into<String>) {
        self.pipeline.package_name = Some(name.into());
    }

    /// Compile source code to native code
    pub fn compile(&mut self, source: &str, filename: &str) -> Result<(), String> {
        if self.emit_output {
            println!("🚀 SpectraLang Compiler");
            println!("━━━━━━━━━━━━━━━━━━━━");
            println!();
        }

        let report = self
            .compile_to_report(source, filename)
            .map_err(|errors| render_errors(&errors, source, filename, "compilation"))?;

        if self.emit_output {
            self.emit_lint_warnings(&report.warnings, filename, source);

            if self.options.optimize {
                let modified_passes: Vec<_> = report
                    .artifacts
                    .passes
                    .iter()
                    .filter(|entry| entry.modified)
                    .map(|entry| entry.name)
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
                && !report.artifacts.passes.is_empty()
            {
                println!("Pass timings:");
                for entry in &report.artifacts.passes {
                    let status = if entry.modified {
                        "modified"
                    } else {
                        "no change"
                    };
                    println!(
                        "  • {:<28} {:>10?} ({})",
                        entry.name, entry.duration, status
                    );
                }
            }

            if self.options.collect_metrics && self.emit_internal_metrics {
                println!("Lowering time: {:?}", report.artifacts.lowering_duration);
                println!("Code generation time: {:?}", report.artifacts.codegen_duration);
            }

            if self.emit_internal_metrics {
                if let Some(metrics) = report.metrics.as_ref() {
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
                report.artifacts.ir_module.functions.len()
            );

            println!("✨ Compilation successful!");
            println!("━━━━━━━━━━━━━━━━━━━━");
        }

        if self.options.run_jit {
            self.pipeline
                .execute_artifacts(&report.artifacts)
                .map_err(|errors| render_errors(&errors, source, filename, "execution"))?;
        }

        Ok(())
    }

    /// Compile a source file to a native object file. Returns the raw object bytes.
    pub fn compile_to_object_bytes(&mut self, source: &str, filename: &str) -> Result<Vec<u8>, String> {
        let report = self
            .compile_to_report(source, filename)
            .map_err(|errors| render_errors(&errors, source, filename, "compilation"))?;

        let aot = AotCodeGenerator::new();
        aot.compile_to_object(&report.artifacts.ir_module, &AotOptions::default())
    }

    /// Compile a source file to a native object file that contains a full
    /// executable entry point (`main` shim + `spectra_rt_startup_with_args`).
    /// The resulting bytes must be linked with `libspectra_runtime.a` to produce
    /// a standalone executable.
    pub fn compile_to_executable_object_bytes(
        &mut self,
        source: &str,
        filename: &str,
    ) -> Result<Vec<u8>, String> {
        let report = self
            .compile_to_report(source, filename)
            .map_err(|errors| render_errors(&errors, source, filename, "compilation"))?;

        let aot = AotCodeGenerator::new();
        aot.compile_to_object(
            &report.artifacts.ir_module,
            &AotOptions { emit_executable: true },
        )
    }

    fn compile_to_report(
        &mut self,
        source: &str,
        filename: &str,
    ) -> Result<CompilationReport, Vec<CompilerError>> {
        self.last_summary = None;

        let compilation = self.pipeline.compile(source, filename)?;

        let CompilationResult {
            backend_artifacts: artifacts,
            metrics,
            warnings,
            ..
        } = compilation;

        if let Some(aggregate) = self.aggregate.as_mut() {
            aggregate.record(&artifacts, metrics.as_ref());
        }

        let pass_summaries = artifacts
            .passes
            .iter()
            .map(|entry| PassSummary {
                name: entry.name,
                duration: entry.duration,
                modified: entry.modified,
            })
            .collect();

        self.last_summary = Some(ModulePipelineSummary {
            filename: filename.to_string(),
            lowering_duration: artifacts.lowering_duration,
            codegen_duration: artifacts.codegen_duration,
            frontend_metrics: metrics.clone(),
            passes: pass_summaries,
        });

        Ok(CompilationReport {
            artifacts,
            metrics,
            warnings,
        })
    }

    pub fn set_emit_output(&mut self, emit: bool) {
        self.emit_output = emit;
    }

    /// When `true`, suppresses the "Execution completed" meta-information line
    /// so that only the Spectra program's own output reaches the terminal.
    /// Automatically cleared when `--timings` / `collect_metrics` is active.
    pub fn set_quiet_execution(&mut self, quiet: bool) {
        self.pipeline.backend_mut().quiet_execution = quiet;
    }

    pub fn compile_for_diagnostics(
        &mut self,
        source: &str,
        filename: &str,
    ) -> Result<Vec<LintDiagnostic>, Vec<CompilerError>> {
        let report = self.compile_to_report(source, filename)?;
        Ok(report.warnings)
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

    fn emit_lint_warnings(&self, warnings: &[LintDiagnostic], filename: &str, source: &str) {
        if warnings.is_empty() {
            return;
        }

        for warning in warnings {
            let message = render_lint_warning(warning, filename, source);
            // Print directly — render_lint_warning already produces the full
            // "warning[...]: ..." diagnostic block; adding another prefix would
            // double-wrap the first line and misalign the source-span gutter.
            eprint!("{}", message);
        }
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

        self.emit_lint_warnings(&warnings, "<jit>", source);

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

/// ANSI color codes for terminal output. Automatically disabled when NO_COLOR is set.
struct Colors {
    error_label: &'static str,
    warning_label: &'static str,
    arrow: &'static str,
    gutter: &'static str,
    caret_error: &'static str,
    caret_warning: &'static str,
    note_label: &'static str,
    help_label: &'static str,
    bold: &'static str,
    reset: &'static str,
}

const COLORS_ON: Colors = Colors {
    error_label: "\x1b[1;31m",
    warning_label: "\x1b[1;33m",
    arrow: "\x1b[36m",
    gutter: "\x1b[34m",
    caret_error: "\x1b[1;31m",
    caret_warning: "\x1b[1;33m",
    note_label: "\x1b[1;34m",
    help_label: "\x1b[1;32m",
    bold: "\x1b[1m",
    reset: "\x1b[0m",
};

const COLORS_OFF: Colors = Colors {
    error_label: "",
    warning_label: "",
    arrow: "",
    gutter: "",
    caret_error: "",
    caret_warning: "",
    note_label: "",
    help_label: "",
    bold: "",
    reset: "",
};

fn get_colors() -> &'static Colors {
    use std::io::IsTerminal as _;
    if std::env::var("NO_COLOR").is_ok()
        || std::env::var("TERM").as_deref() == Ok("dumb")
        || !std::io::stderr().is_terminal()
    {
        &COLORS_OFF
    } else {
        &COLORS_ON
    }
}

fn render_errors(errors: &[CompilerError], source: &str, filename: &str, stage: &str) -> String {
    if errors.is_empty() {
        return format!("{} failed with no diagnostics.", capitalize(stage));
    }

    let c = get_colors();
    let mut output = String::new();
    for (idx, error) in errors.iter().enumerate() {
        if idx > 0 {
            output.push('\n');
        }
        output.push_str(&render_error(error, source, filename));
    }

    // Summary line — mirrors rustc: "error: aborting due to N previous error(s)"
    let n = errors.len();
    output.push('\n');
    let _ = writeln!(
        &mut output,
        "{}error{}: aborting due to {} previous error{}",
        c.error_label,
        c.reset,
        n,
        if n == 1 { "" } else { "s" },
    );

    output
}

enum DiagnosticSeverity {
    Error,
    Warning,
}

impl DiagnosticSeverity {
    fn as_str(&self) -> &'static str {
        match self {
            DiagnosticSeverity::Error => "error",
            DiagnosticSeverity::Warning => "warning",
        }
    }

    fn color<'a>(&self, c: &'a Colors) -> &'a str {
        match self {
            DiagnosticSeverity::Error => c.error_label,
            DiagnosticSeverity::Warning => c.warning_label,
        }
    }

    fn caret_color<'a>(&self, c: &'a Colors) -> &'a str {
        match self {
            DiagnosticSeverity::Error => c.caret_error,
            DiagnosticSeverity::Warning => c.caret_warning,
        }
    }
}

fn render_error(error: &CompilerError, source: &str, filename: &str) -> String {
    match error {
        CompilerError::Lexical(e) => render_span_diagnostic(
            "syntax",
            DiagnosticSeverity::Error,
            &e.message,
            &e.span,
            e.hint.as_deref(),
            e.context.as_deref(),
            source,
            filename,
        ),
        CompilerError::Parse(e) => render_span_diagnostic(
            "syntax",
            DiagnosticSeverity::Error,
            &e.message,
            &e.span,
            e.hint.as_deref(),
            e.context.as_deref(),
            source,
            filename,
        ),
        CompilerError::Semantic(e) => render_span_diagnostic(
            "semantic",
            DiagnosticSeverity::Error,
            &e.message,
            &e.span,
            e.hint.as_deref(),
            e.context.as_deref(),
            source,
            filename,
        ),
        CompilerError::Midend(e) => {
            let c = get_colors();
            format!(
                "{}error[internal]{}: {}{}{}\n",
                c.error_label, c.reset, c.bold, e.message, c.reset
            )
        }
        CompilerError::Backend(e) => {
            let c = get_colors();
            format!(
                "{}error[codegen]{}: {}{}{}\n",
                c.error_label, c.reset, c.bold, e.message, c.reset
            )
        }
    }
}

fn render_span_diagnostic(
    phase: &str,
    severity: DiagnosticSeverity,
    message: &str,
    span: &Span,
    hint: Option<&str>,
    context: Option<&str>,
    source: &str,
    filename: &str,
) -> String {
    let c = get_colors();
    let mut buf = String::new();

    // Header: error[phase]: message
    let sev_color = severity.color(c);
    let _ = writeln!(
        &mut buf,
        "{}{}[{}]{}: {}{}{}",
        sev_color,
        severity.as_str(),
        phase,
        c.reset,
        c.bold,
        message,
        c.reset
    );

    // Location: --> filename:line:col
    let _ = writeln!(
        &mut buf,
        "  {}-->{} {}:{}:{}",
        c.arrow,
        c.reset,
        filename,
        span.start_location.line,
        span.start_location.column
    );

    if let Some(raw_line) = get_source_line(source, span.start_location.line) {
        let line_text = raw_line.trim_end_matches('\r');
        let gutter_width = span.start_location.line.to_string().len();
        let pipe = format!("{}|{}", c.gutter, c.reset);

        // Empty gutter line
        let _ = writeln!(&mut buf, "  {}{:>width$} {}", c.gutter, "", c.reset, width = gutter_width);

        // Source line
        let _ = writeln!(
            &mut buf,
            "  {}{:>width$}{} {} {}",
            c.gutter,
            span.start_location.line,
            c.reset,
            pipe,
            line_text,
            width = gutter_width
        );

        // Caret line
        if let Some(marker_line) = build_highlight_line(span, line_text) {
            let caret_color = severity.caret_color(c);
            let _ = writeln!(
                &mut buf,
                "  {}{:>width$} {} {}{}{}",
                c.gutter,
                "",
                pipe,
                caret_color,
                marker_line,
                c.reset,
                width = gutter_width
            );
        }
    }

    if span.start_location.line != span.end_location.line {
        let _ = writeln!(
            &mut buf,
            "  {}= note:{} spans lines {}–{}",
            c.note_label, c.reset,
            span.start_location.line,
            span.end_location.line
        );
    }

    if let Some(context) = context {
        let _ = writeln!(&mut buf, "  {}= note:{} {}", c.note_label, c.reset, context);
    }

    if let Some(hint) = hint {
        let _ = writeln!(&mut buf, "  {}= help:{} {}", c.help_label, c.reset, hint);
    }

    buf
}

fn render_lint_warning(diagnostic: &LintDiagnostic, filename: &str, source: &str) -> String {
    let mut context = diagnostic.note.clone().unwrap_or_default();

    if let Some(secondary) = diagnostic.secondary_span {
        let related = format!(
            "related location: {}:{}:{}",
            filename,
            secondary.start_location.line,
            secondary.start_location.column
        );

        if context.is_empty() {
            context = related;
        } else {
            context.push_str("; ");
            context.push_str(&related);
        }
    }

    let context_owned = if context.is_empty() { None } else { Some(context) };
    let context_ref = context_owned.as_deref();

    render_span_diagnostic(
        &format!("lint({})", diagnostic.rule.code()),
        DiagnosticSeverity::Warning,
        &diagnostic.message,
        &diagnostic.span,
        None,
        context_ref,
        source,
        filename,
    )
}

fn log_warning(message: &str) {
    for (index, line) in message.lines().enumerate() {
        if index == 0 {
            eprintln!("warning: {}", line);
        } else if line.is_empty() {
            eprintln!();
        } else {
            eprintln!("         {}", line);
        }
    }
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
    use spectra_compiler::lint::LintOptions;
    use std::collections::HashSet;

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
