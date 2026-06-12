use crate::ir::{InstructionKind, Module, Type, Value};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fmt::Write;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TensorGraph {
    pub module: String,
    pub functions: Vec<TensorGraphFunction>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TensorGraphFunction {
    pub name: String,
    pub nodes: Vec<TensorGraphNode>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TensorGraphNode {
    pub id: usize,
    pub value: Option<usize>,
    pub op: TensorGraphOp,
    pub inputs: Vec<usize>,
    pub output: TensorMetadata,
    pub source: TensorGraphSource,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TensorGraphSource {
    pub block: usize,
    pub instruction: usize,
    pub host: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TensorGraphOp {
    Parameter,
    Create {
        name: String,
    },
    Reshape,
    Transpose,
    Matmul,
    BatchedMatmul,
    Elementwise {
        name: String,
    },
    FusedElementwise {
        ops: Vec<String>,
    },
    Reduction {
        name: String,
    },
    FusedReduction {
        elementwise_ops: Vec<String>,
        reduction: String,
    },
    DeviceTransfer {
        target: TensorDevice,
    },
    Linear,
    Conv2d,
    Dropout,
    MaxPool2d,
    Loss {
        name: String,
    },
    UnknownHost {
        host: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TensorMetadata {
    pub dtype: TensorDType,
    pub shape: TensorShape,
    pub layout: TensorLayout,
    pub device: TensorDevice,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TensorDType {
    Int,
    Float,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TensorShape {
    Ranked(Vec<Option<usize>>),
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TensorLayout {
    Contiguous,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TensorDevice {
    Cpu,
    Wgpu,
    Reserved(i64),
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TensorGraphError {
    pub function: String,
    pub node: Option<usize>,
    pub kind: TensorGraphErrorKind,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TensorGraphErrorKind {
    Cycle,
    ShapeMismatch,
    DeviceMismatch,
    UnsupportedOperator,
    InvalidDependency,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TensorGraphOptimizationResult {
    pub graph: TensorGraph,
    pub report: TensorGraphOptimizationReport,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TensorGraphOptimizationReport {
    pub original_nodes: usize,
    pub optimized_nodes: usize,
    pub fused_groups: usize,
    pub fused_elementwise_ops: usize,
    pub fused_reductions: usize,
    pub reusable_edges: usize,
    pub tolerance_abs: String,
    pub tolerance_rel: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TensorGraphComparison {
    pub equivalent: bool,
    pub checked_outputs: usize,
    pub tolerance_abs: String,
    pub tolerance_rel: String,
    pub diagnostics: Vec<String>,
}

impl TensorGraph {
    pub fn from_ir_module(module: &Module) -> Self {
        let functions = module
            .functions
            .iter()
            .map(TensorGraphFunction::from_ir_function)
            .collect();
        Self {
            module: module.name.clone(),
            functions,
        }
    }

    pub fn validate(&self) -> Result<(), Vec<TensorGraphError>> {
        let mut errors = Vec::new();
        for function in &self.functions {
            function.validate_into(&mut errors);
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    pub fn stable_dump(&self) -> String {
        let mut out = String::new();
        let _ = writeln!(out, "tensor_graph module {}", self.module);
        for function in &self.functions {
            let _ = writeln!(out, "fn {} {{", function.name);
            for node in &function.nodes {
                let value = node
                    .value
                    .map(|id| format!("%{id}"))
                    .unwrap_or_else(|| "_".to_string());
                let inputs = if node.inputs.is_empty() {
                    "-".to_string()
                } else {
                    node.inputs
                        .iter()
                        .map(|id| format!("n{id}"))
                        .collect::<Vec<_>>()
                        .join(",")
                };
                let _ = writeln!(
                    out,
                    "  n{} {} = {}({}) -> {} @b{}:i{}",
                    node.id,
                    value,
                    node.op.stable_name(),
                    inputs,
                    node.output.stable_name(),
                    node.source.block,
                    node.source.instruction
                );
            }
            let _ = writeln!(out, "}}");
        }
        out
    }

    pub fn optimize(&self) -> Result<TensorGraphOptimizationResult, Vec<TensorGraphError>> {
        self.validate()?;
        let mut report = TensorGraphOptimizationReport {
            original_nodes: self
                .functions
                .iter()
                .map(|function| function.nodes.len())
                .sum(),
            optimized_nodes: 0,
            fused_groups: 0,
            fused_elementwise_ops: 0,
            fused_reductions: 0,
            reusable_edges: 0,
            tolerance_abs: "1e-9".to_string(),
            tolerance_rel: "1e-9".to_string(),
        };
        let functions = self
            .functions
            .iter()
            .map(|function| function.optimize_into(&mut report))
            .collect::<Vec<_>>();
        report.optimized_nodes = functions.iter().map(|function| function.nodes.len()).sum();
        Ok(TensorGraphOptimizationResult {
            graph: TensorGraph {
                module: self.module.clone(),
                functions,
            },
            report,
        })
    }

    pub fn compare_optimized(&self, optimized: &TensorGraph) -> TensorGraphComparison {
        let mut diagnostics = Vec::new();
        let mut checked_outputs = 0;
        if self.module != optimized.module {
            diagnostics.push(format!(
                "module mismatch: '{}' != '{}'",
                self.module, optimized.module
            ));
        }
        for original_function in &self.functions {
            let Some(optimized_function) = optimized
                .functions
                .iter()
                .find(|function| function.name == original_function.name)
            else {
                diagnostics.push(format!(
                    "optimized graph is missing function '{}'",
                    original_function.name
                ));
                continue;
            };
            let original_outputs = original_function.observable_outputs();
            let optimized_outputs = optimized_function.observable_outputs();
            checked_outputs += original_outputs.len();
            if original_outputs != optimized_outputs {
                diagnostics.push(format!(
                    "function '{}' observable outputs differ: {:?} != {:?}",
                    original_function.name, original_outputs, optimized_outputs
                ));
            }
        }
        TensorGraphComparison {
            equivalent: diagnostics.is_empty(),
            checked_outputs,
            tolerance_abs: "1e-9".to_string(),
            tolerance_rel: "1e-9".to_string(),
            diagnostics,
        }
    }
}

impl TensorGraphFunction {
    fn from_ir_function(function: &crate::ir::Function) -> Self {
        let mut extractor = TensorGraphExtractor::default();
        for block in &function.blocks {
            for instruction in &block.instructions {
                match &instruction.kind {
                    InstructionKind::ConstInt { result, value } => {
                        extractor.constants.insert(result.id, *value);
                    }
                    InstructionKind::Add { result, lhs, rhs } => {
                        extractor.fold_const_int(*result, *lhs, *rhs, i64::saturating_add);
                    }
                    InstructionKind::Sub { result, lhs, rhs } => {
                        extractor.fold_const_int(*result, *lhs, *rhs, i64::saturating_sub);
                    }
                    InstructionKind::Mul { result, lhs, rhs } => {
                        extractor.fold_const_int(*result, *lhs, *rhs, i64::saturating_mul);
                    }
                    InstructionKind::Div { result, lhs, rhs } => {
                        extractor.fold_const_int_checked(*result, *lhs, *rhs, |left, right| {
                            (right != 0).then(|| left / right)
                        });
                    }
                    InstructionKind::Rem { result, lhs, rhs } => {
                        extractor.fold_const_int_checked(*result, *lhs, *rhs, |left, right| {
                            (right != 0).then(|| left % right)
                        });
                    }
                    InstructionKind::HostCall {
                        result, host, args, ..
                    } => {
                        extractor.lower_host_call(block.id, instruction.id, *result, host, args);
                    }
                    _ => {}
                }
            }
        }
        Self {
            name: function.name.clone(),
            nodes: extractor.nodes,
        }
    }

    pub fn validate(&self) -> Result<(), Vec<TensorGraphError>> {
        let mut errors = Vec::new();
        self.validate_into(&mut errors);
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    fn optimize_into(&self, report: &mut TensorGraphOptimizationReport) -> Self {
        let consumer_counts = self.consumer_counts();
        let mut old_to_new = HashMap::new();
        let mut skipped = HashSet::new();
        let mut nodes = Vec::new();

        for node in &self.nodes {
            if skipped.contains(&node.id) {
                continue;
            }
            if matches!(node.op, TensorGraphOp::Elementwise { .. })
                && self.elementwise_chain_feeds_reduction(node.id, &consumer_counts)
            {
                continue;
            }
            if let Some(fused) =
                self.try_fuse_reduction(node, &consumer_counts, &mut skipped, report)
            {
                let new_id = nodes.len();
                for old_id in fused.old_node_ids {
                    old_to_new.insert(old_id, new_id);
                }
                nodes.push(fused.node.with_id(new_id));
                continue;
            }
            if let Some(fused) =
                self.try_fuse_elementwise_chain(node, &consumer_counts, &mut skipped, report)
            {
                let new_id = nodes.len();
                for old_id in fused.old_node_ids {
                    old_to_new.insert(old_id, new_id);
                }
                nodes.push(fused.node.with_id(new_id));
                continue;
            }
            let new_id = nodes.len();
            old_to_new.insert(node.id, new_id);
            nodes.push(node.clone().with_id(new_id));
        }

        for node in &mut nodes {
            node.inputs = node
                .inputs
                .iter()
                .filter_map(|input| old_to_new.get(input).copied())
                .collect();
            node.inputs.sort_unstable();
            node.inputs.dedup();
        }

        report.reusable_edges += nodes
            .iter()
            .filter(|node| {
                matches!(
                    node.op,
                    TensorGraphOp::FusedElementwise { .. } | TensorGraphOp::FusedReduction { .. }
                )
            })
            .map(|node| node.inputs.len())
            .sum::<usize>();

        Self {
            name: self.name.clone(),
            nodes,
        }
    }

    fn consumer_counts(&self) -> HashMap<usize, usize> {
        let mut counts = HashMap::new();
        for node in &self.nodes {
            for input in &node.inputs {
                *counts.entry(*input).or_insert(0) += 1;
            }
        }
        counts
    }

    fn try_fuse_reduction(
        &self,
        node: &TensorGraphNode,
        consumer_counts: &HashMap<usize, usize>,
        skipped: &mut HashSet<usize>,
        report: &mut TensorGraphOptimizationReport,
    ) -> Option<FusedTensorNode> {
        let TensorGraphOp::Reduction { name } = &node.op else {
            return None;
        };
        let chain = self.collect_elementwise_chain(*node.inputs.first()?, consumer_counts);
        if chain.len() < 2 {
            return None;
        }
        let first = chain.first()?;
        let base_inputs = first.inputs.clone();
        let elementwise_ops = chain
            .iter()
            .map(|chain_node| match &chain_node.op {
                TensorGraphOp::Elementwise { name } => name.clone(),
                _ => unreachable!("chain only contains elementwise nodes"),
            })
            .collect::<Vec<_>>();
        let old_node_ids = chain
            .iter()
            .map(|chain_node| chain_node.id)
            .chain(std::iter::once(node.id))
            .collect::<Vec<_>>();
        for old_id in &old_node_ids {
            skipped.insert(*old_id);
        }
        report.fused_groups += 1;
        report.fused_elementwise_ops += elementwise_ops.len();
        report.fused_reductions += 1;
        Some(FusedTensorNode {
            old_node_ids,
            node: TensorGraphNode {
                id: node.id,
                value: node.value,
                op: TensorGraphOp::FusedReduction {
                    elementwise_ops,
                    reduction: name.clone(),
                },
                inputs: base_inputs,
                output: node.output.clone(),
                source: node.source.clone(),
            },
        })
    }

    fn try_fuse_elementwise_chain(
        &self,
        node: &TensorGraphNode,
        consumer_counts: &HashMap<usize, usize>,
        skipped: &mut HashSet<usize>,
        report: &mut TensorGraphOptimizationReport,
    ) -> Option<FusedTensorNode> {
        if !matches!(node.op, TensorGraphOp::Elementwise { .. }) {
            return None;
        }
        let chain = self.collect_forward_elementwise_chain(node.id, consumer_counts);
        if chain.len() < 2 {
            return None;
        }
        if chain
            .last()
            .and_then(|last| self.single_consumer(last.id, consumer_counts))
            .is_some_and(|consumer| matches!(consumer.op, TensorGraphOp::Reduction { .. }))
        {
            return None;
        }
        let first = chain.first()?;
        let last = chain.last()?;
        let ops = chain
            .iter()
            .map(|chain_node| match &chain_node.op {
                TensorGraphOp::Elementwise { name } => name.clone(),
                _ => unreachable!("chain only contains elementwise nodes"),
            })
            .collect::<Vec<_>>();
        let old_node_ids = chain
            .iter()
            .map(|chain_node| chain_node.id)
            .collect::<Vec<_>>();
        for old_id in &old_node_ids {
            skipped.insert(*old_id);
        }
        report.fused_groups += 1;
        report.fused_elementwise_ops += ops.len();
        Some(FusedTensorNode {
            old_node_ids,
            node: TensorGraphNode {
                id: first.id,
                value: last.value,
                op: TensorGraphOp::FusedElementwise { ops },
                inputs: first.inputs.clone(),
                output: last.output.clone(),
                source: last.source.clone(),
            },
        })
    }

    fn collect_forward_elementwise_chain(
        &self,
        start_id: usize,
        consumer_counts: &HashMap<usize, usize>,
    ) -> Vec<&TensorGraphNode> {
        let by_id = self
            .nodes
            .iter()
            .map(|node| (node.id, node))
            .collect::<HashMap<_, _>>();
        let mut chain = Vec::new();
        let mut current_id = start_id;
        while let Some(current) = by_id.get(&current_id).copied() {
            if !matches!(current.op, TensorGraphOp::Elementwise { .. }) {
                break;
            }
            chain.push(current);
            if consumer_counts.get(&current_id).copied().unwrap_or(0) != 1 {
                break;
            }
            let Some(next) = self.nodes.iter().find(|candidate| {
                candidate.inputs.len() == 1
                    && candidate.inputs[0] == current_id
                    && matches!(candidate.op, TensorGraphOp::Elementwise { .. })
            }) else {
                break;
            };
            current_id = next.id;
        }
        chain
    }

    fn elementwise_chain_feeds_reduction(
        &self,
        start_id: usize,
        consumer_counts: &HashMap<usize, usize>,
    ) -> bool {
        let mut current_id = start_id;
        loop {
            let Some(current) = self.nodes.iter().find(|node| node.id == current_id) else {
                return false;
            };
            if !matches!(current.op, TensorGraphOp::Elementwise { .. }) {
                return false;
            }
            let Some(consumer) = self.single_consumer(current_id, consumer_counts) else {
                return false;
            };
            if matches!(consumer.op, TensorGraphOp::Reduction { .. }) {
                return true;
            }
            if !matches!(consumer.op, TensorGraphOp::Elementwise { .. }) {
                return false;
            }
            current_id = consumer.id;
        }
    }

    fn single_consumer(
        &self,
        node_id: usize,
        consumer_counts: &HashMap<usize, usize>,
    ) -> Option<&TensorGraphNode> {
        if consumer_counts.get(&node_id).copied().unwrap_or(0) != 1 {
            return None;
        }
        self.nodes.iter().find(|candidate| {
            candidate.inputs.len() == 1 && candidate.inputs.first().copied() == Some(node_id)
        })
    }

    fn collect_elementwise_chain(
        &self,
        start_id: usize,
        consumer_counts: &HashMap<usize, usize>,
    ) -> Vec<&TensorGraphNode> {
        let by_id = self
            .nodes
            .iter()
            .map(|node| (node.id, node))
            .collect::<HashMap<_, _>>();
        let mut reversed = Vec::new();
        let mut current_id = start_id;
        while let Some(current) = by_id.get(&current_id).copied() {
            if !matches!(current.op, TensorGraphOp::Elementwise { .. }) {
                break;
            }
            if consumer_counts.get(&current_id).copied().unwrap_or(0) != 1 {
                break;
            }
            reversed.push(current);
            let Some(previous) = current.inputs.first().and_then(|id| by_id.get(id)).copied()
            else {
                break;
            };
            current_id = previous.id;
        }
        reversed.reverse();
        reversed
    }

    fn observable_outputs(&self) -> BTreeMap<usize, TensorMetadata> {
        let consumed = self
            .nodes
            .iter()
            .flat_map(|node| node.inputs.iter().copied())
            .collect::<HashSet<_>>();
        self.nodes
            .iter()
            .filter(|node| !consumed.contains(&node.id))
            .filter_map(|node| node.value.map(|value| (value, node.output.clone())))
            .collect()
    }

    fn validate_into(&self, errors: &mut Vec<TensorGraphError>) {
        let ids = self
            .nodes
            .iter()
            .map(|node| node.id)
            .collect::<HashSet<_>>();
        let by_id = self
            .nodes
            .iter()
            .map(|node| (node.id, node))
            .collect::<BTreeMap<_, _>>();

        for node in &self.nodes {
            for input in &node.inputs {
                if !ids.contains(input) {
                    errors.push(TensorGraphError::new(
                        &self.name,
                        Some(node.id),
                        TensorGraphErrorKind::InvalidDependency,
                        format!("node n{} references missing dependency n{}", node.id, input),
                    ));
                }
            }
            self.validate_node_contract(node, &by_id, errors);
        }

        let mut visiting = HashSet::new();
        let mut visited = HashSet::new();
        for node in &self.nodes {
            if has_cycle(node.id, &by_id, &mut visiting, &mut visited) {
                errors.push(TensorGraphError::new(
                    &self.name,
                    Some(node.id),
                    TensorGraphErrorKind::Cycle,
                    format!("cycle detected through node n{}", node.id),
                ));
                break;
            }
        }
    }

    fn validate_node_contract(
        &self,
        node: &TensorGraphNode,
        by_id: &BTreeMap<usize, &TensorGraphNode>,
        errors: &mut Vec<TensorGraphError>,
    ) {
        match &node.op {
            TensorGraphOp::Matmul => {
                let Some([left, right]) = input_pair(node, by_id) else {
                    return;
                };
                if let (Some(l), Some(r)) = (left.output.shape.dim(1), right.output.shape.dim(0)) {
                    if l != r {
                        errors.push(TensorGraphError::new(
                            &self.name,
                            Some(node.id),
                            TensorGraphErrorKind::ShapeMismatch,
                            format!("matmul expects lhs dim1 == rhs dim0, got {l} and {r}"),
                        ));
                    }
                }
                validate_same_device(&self.name, node, left, right, errors);
            }
            TensorGraphOp::Elementwise { name } | TensorGraphOp::Loss { name } => {
                if node.inputs.len() >= 2 {
                    let Some([left, right]) = input_pair(node, by_id) else {
                        return;
                    };
                    if !left.output.shape.compatible_with(&right.output.shape) {
                        errors.push(TensorGraphError::new(
                            &self.name,
                            Some(node.id),
                            TensorGraphErrorKind::ShapeMismatch,
                            format!(
                                "{name} expects compatible input shapes, got {} and {}",
                                left.output.shape.stable_name(),
                                right.output.shape.stable_name()
                            ),
                        ));
                    }
                    validate_same_device(&self.name, node, left, right, errors);
                }
            }
            TensorGraphOp::Conv2d => {
                if let (Some(input), Some(kernel)) = (
                    node.inputs.first().and_then(|id| by_id.get(id)).copied(),
                    node.inputs.get(1).and_then(|id| by_id.get(id)).copied(),
                ) {
                    validate_same_device(&self.name, node, input, kernel, errors);
                }
            }
            TensorGraphOp::UnknownHost { host } => {
                errors.push(TensorGraphError::new(
                    &self.name,
                    Some(node.id),
                    TensorGraphErrorKind::UnsupportedOperator,
                    format!("unsupported tensor host operator '{host}'"),
                ));
            }
            _ => {}
        }
    }
}

struct FusedTensorNode {
    old_node_ids: Vec<usize>,
    node: TensorGraphNode,
}

impl TensorGraphNode {
    fn with_id(mut self, id: usize) -> Self {
        self.id = id;
        self
    }
}

impl TensorGraphError {
    fn new(
        function: &str,
        node: Option<usize>,
        kind: TensorGraphErrorKind,
        message: String,
    ) -> Self {
        Self {
            function: function.to_string(),
            node,
            kind,
            message,
        }
    }
}

#[derive(Default)]
struct TensorGraphExtractor {
    nodes: Vec<TensorGraphNode>,
    value_to_node: HashMap<usize, usize>,
    constants: HashMap<usize, i64>,
}

impl TensorGraphExtractor {
    fn fold_const_int(
        &mut self,
        result: Value,
        lhs: Value,
        rhs: Value,
        op: impl FnOnce(i64, i64) -> i64,
    ) {
        self.fold_const_int_checked(result, lhs, rhs, |left, right| Some(op(left, right)));
    }

    fn fold_const_int_checked(
        &mut self,
        result: Value,
        lhs: Value,
        rhs: Value,
        op: impl FnOnce(i64, i64) -> Option<i64>,
    ) {
        if let (Some(left), Some(right)) =
            (self.constants.get(&lhs.id), self.constants.get(&rhs.id))
        {
            if let Some(value) = op(*left, *right) {
                self.constants.insert(result.id, value);
            }
        }
    }

    fn lower_host_call(
        &mut self,
        block: usize,
        instruction: usize,
        result: Option<Value>,
        host: &str,
        args: &[Value],
    ) {
        if !is_tensor_host(host) {
            return;
        }
        let Some(value) = result else {
            return;
        };
        let Some(op) = classify_host(host, args, &self.constants) else {
            return;
        };
        let tensor_positions = tensor_arg_positions(host);
        let inputs = tensor_positions
            .iter()
            .filter_map(|position| args.get(*position).copied())
            .map(|arg| self.node_for_input(arg, block, instruction))
            .collect::<Vec<_>>();
        let output = infer_output_metadata(&op, &inputs, &self.nodes, args, &self.constants);
        let id = self.nodes.len();
        self.nodes.push(TensorGraphNode {
            id,
            value: Some(value.id),
            op,
            inputs,
            output,
            source: TensorGraphSource {
                block,
                instruction,
                host: Some(host.to_string()),
            },
        });
        self.value_to_node.insert(value.id, id);
    }

    fn node_for_input(&mut self, value: Value, block: usize, instruction: usize) -> usize {
        if let Some(node_id) = self.value_to_node.get(&value.id) {
            return *node_id;
        }
        let id = self.nodes.len();
        self.nodes.push(TensorGraphNode {
            id,
            value: Some(value.id),
            op: TensorGraphOp::Parameter,
            inputs: Vec::new(),
            output: TensorMetadata::unknown(),
            source: TensorGraphSource {
                block,
                instruction,
                host: None,
            },
        });
        self.value_to_node.insert(value.id, id);
        id
    }
}

impl TensorGraphOp {
    pub fn stable_name(&self) -> String {
        match self {
            TensorGraphOp::Parameter => "param".to_string(),
            TensorGraphOp::Create { name } => format!("create.{name}"),
            TensorGraphOp::Reshape => "reshape".to_string(),
            TensorGraphOp::Transpose => "transpose".to_string(),
            TensorGraphOp::Matmul => "matmul".to_string(),
            TensorGraphOp::BatchedMatmul => "matmul_batched".to_string(),
            TensorGraphOp::Elementwise { name } => format!("elementwise.{name}"),
            TensorGraphOp::FusedElementwise { ops } => {
                format!("fused_elementwise.{}", ops.join("+"))
            }
            TensorGraphOp::Reduction { name } => format!("reduction.{name}"),
            TensorGraphOp::FusedReduction {
                elementwise_ops,
                reduction,
            } => {
                format!(
                    "fused_reduction.{}->{}",
                    elementwise_ops.join("+"),
                    reduction
                )
            }
            TensorGraphOp::DeviceTransfer { target } => {
                format!("to_device.{}", target.stable_name())
            }
            TensorGraphOp::Linear => "linear".to_string(),
            TensorGraphOp::Conv2d => "conv2d".to_string(),
            TensorGraphOp::Dropout => "dropout".to_string(),
            TensorGraphOp::MaxPool2d => "max_pool2d".to_string(),
            TensorGraphOp::Loss { name } => format!("loss.{name}"),
            TensorGraphOp::UnknownHost { host } => format!("unknown.{host}"),
        }
    }
}

impl TensorMetadata {
    pub fn new(dtype: TensorDType, shape: TensorShape, device: TensorDevice) -> Self {
        Self {
            dtype,
            shape,
            layout: TensorLayout::Contiguous,
            device,
        }
    }

    pub fn unknown() -> Self {
        Self {
            dtype: TensorDType::Unknown,
            shape: TensorShape::Unknown,
            layout: TensorLayout::Unknown,
            device: TensorDevice::Unknown,
        }
    }

    fn stable_name(&self) -> String {
        format!(
            "dtype={} shape={} layout={} device={}",
            self.dtype.stable_name(),
            self.shape.stable_name(),
            self.layout.stable_name(),
            self.device.stable_name()
        )
    }
}

impl TensorDType {
    fn stable_name(self) -> &'static str {
        match self {
            TensorDType::Int => "int",
            TensorDType::Float => "float",
            TensorDType::Unknown => "?",
        }
    }
}

impl TensorShape {
    pub fn rank(&self) -> Option<usize> {
        match self {
            TensorShape::Ranked(dims) => Some(dims.len()),
            TensorShape::Unknown => None,
        }
    }

    pub fn dim(&self, index: usize) -> Option<usize> {
        match self {
            TensorShape::Ranked(dims) => dims.get(index).copied().flatten(),
            TensorShape::Unknown => None,
        }
    }

    fn compatible_with(&self, other: &Self) -> bool {
        match (self, other) {
            (TensorShape::Unknown, _) | (_, TensorShape::Unknown) => true,
            (TensorShape::Ranked(left), TensorShape::Ranked(right)) => {
                left.len() == right.len()
                    && left
                        .iter()
                        .zip(right)
                        .all(|(l, r)| l.is_none() || r.is_none() || l == r)
            }
        }
    }

    fn stable_name(&self) -> String {
        match self {
            TensorShape::Unknown => "?".to_string(),
            TensorShape::Ranked(dims) => format!(
                "[{}]",
                dims.iter()
                    .map(|dim| dim
                        .map(|v| v.to_string())
                        .unwrap_or_else(|| "?".to_string()))
                    .collect::<Vec<_>>()
                    .join(",")
            ),
        }
    }
}

impl TensorLayout {
    fn stable_name(&self) -> &'static str {
        match self {
            TensorLayout::Contiguous => "contiguous",
            TensorLayout::Unknown => "?",
        }
    }
}

impl TensorDevice {
    fn stable_name(&self) -> String {
        match self {
            TensorDevice::Cpu => "cpu".to_string(),
            TensorDevice::Wgpu => "wgpu".to_string(),
            TensorDevice::Reserved(code) => format!("reserved{code}"),
            TensorDevice::Unknown => "?".to_string(),
        }
    }
}

fn is_tensor_host(host: &str) -> bool {
    host.starts_with("spectra.std.tensor.") || host.starts_with("spectra.std.ml.")
}

fn classify_host(
    host: &str,
    args: &[Value],
    constants: &HashMap<usize, i64>,
) -> Option<TensorGraphOp> {
    let name = host
        .strip_prefix("spectra.std.tensor.")
        .or_else(|| host.strip_prefix("spectra.std.ml."))?;
    match name {
        "zeros" | "ones" | "full" | "full_f" | "arange" | "zeros2" | "ones2" | "full2"
        | "full2_f" | "uniform" | "uniform_f" | "normal_f" | "bernoulli" => {
            Some(TensorGraphOp::Create {
                name: name.to_string(),
            })
        }
        "reshape" => Some(TensorGraphOp::Reshape),
        "transpose" => Some(TensorGraphOp::Transpose),
        "matmul" => Some(TensorGraphOp::Matmul),
        "matmul_batched" => Some(TensorGraphOp::BatchedMatmul),
        "add" | "sub" | "mul" | "div" | "neg" | "relu" | "sigmoid_f" | "tanh_f" | "sqrt_f"
        | "log_f" => Some(TensorGraphOp::Elementwise {
            name: name.to_string(),
        }),
        "sum_t" => Some(TensorGraphOp::Reduction {
            name: name.to_string(),
        }),
        "to_device" => {
            let device = args
                .get(1)
                .and_then(|value| constants.get(&value.id))
                .map(|code| device_from_code(*code))
                .unwrap_or(TensorDevice::Unknown);
            Some(TensorGraphOp::DeviceTransfer { target: device })
        }
        "cpu" => Some(TensorGraphOp::DeviceTransfer {
            target: TensorDevice::Cpu,
        }),
        "linear" => Some(TensorGraphOp::Linear),
        "conv2d" => Some(TensorGraphOp::Conv2d),
        "dropout" => Some(TensorGraphOp::Dropout),
        "max_pool2d" => Some(TensorGraphOp::MaxPool2d),
        "mse_loss" | "bce_loss" => Some(TensorGraphOp::Loss {
            name: name.to_string(),
        }),
        _ if returns_tensor_like(host, name) => Some(TensorGraphOp::UnknownHost {
            host: host.to_string(),
        }),
        _ => None,
    }
}

fn returns_tensor_like(host: &str, name: &str) -> bool {
    host.starts_with("spectra.std.ml.")
        || matches!(
            name,
            "requires_grad" | "grad" | "clone" | "slice" | "concat" | "stack" | "permute"
        )
}

fn tensor_arg_positions(host: &str) -> &'static [usize] {
    let name = host
        .strip_prefix("spectra.std.tensor.")
        .or_else(|| host.strip_prefix("spectra.std.ml."))
        .unwrap_or("");
    match name {
        "reshape" | "transpose" | "sum_t" | "neg" | "relu" | "sigmoid_f" | "tanh_f" | "sqrt_f"
        | "log_f" | "to_device" | "cpu" | "dropout" | "max_pool2d" => &[0],
        "add" | "sub" | "mul" | "div" | "matmul" | "matmul_batched" | "mse_loss" | "bce_loss" => {
            &[0, 1]
        }
        "linear" | "conv2d" => &[0, 1, 2],
        "requires_grad" | "grad" | "clone" | "slice" | "permute" => &[0],
        "concat" | "stack" => &[0, 1],
        _ => &[],
    }
}

fn infer_output_metadata(
    op: &TensorGraphOp,
    inputs: &[usize],
    nodes: &[TensorGraphNode],
    args: &[Value],
    constants: &HashMap<usize, i64>,
) -> TensorMetadata {
    let input = |index: usize| inputs.get(index).and_then(|id| nodes.get(*id));
    match op {
        TensorGraphOp::Create { name } => infer_create_metadata(name, args, constants),
        TensorGraphOp::Reshape => {
            let mut meta = input(0)
                .map(|node| node.output.clone())
                .unwrap_or_else(TensorMetadata::unknown);
            let rows = const_usize(args.get(1), constants);
            let cols = const_usize(args.get(2), constants);
            meta.shape = TensorShape::Ranked(vec![rows, cols]);
            meta
        }
        TensorGraphOp::Transpose => {
            let mut meta = input(0)
                .map(|node| node.output.clone())
                .unwrap_or_else(TensorMetadata::unknown);
            if let TensorShape::Ranked(dims) = &meta.shape {
                if dims.len() == 2 {
                    meta.shape = TensorShape::Ranked(vec![dims[1], dims[0]]);
                }
            }
            meta
        }
        TensorGraphOp::Matmul => {
            let left = input(0).map(|node| &node.output);
            let right = input(1).map(|node| &node.output);
            let dtype = left
                .map(|meta| meta.dtype)
                .filter(|dtype| *dtype != TensorDType::Unknown)
                .or_else(|| right.map(|meta| meta.dtype))
                .unwrap_or(TensorDType::Unknown);
            let device = left
                .map(|meta| meta.device.clone())
                .filter(|device| *device != TensorDevice::Unknown)
                .or_else(|| right.map(|meta| meta.device.clone()))
                .unwrap_or(TensorDevice::Unknown);
            let shape = match (left.map(|meta| &meta.shape), right.map(|meta| &meta.shape)) {
                (Some(TensorShape::Ranked(l)), Some(TensorShape::Ranked(r)))
                    if l.len() == 2 && r.len() == 2 =>
                {
                    TensorShape::Ranked(vec![l[0], r[1]])
                }
                _ => TensorShape::Unknown,
            };
            TensorMetadata::new(dtype, shape, device)
        }
        TensorGraphOp::Reduction { .. } | TensorGraphOp::Loss { .. } => input(0)
            .map(|node| {
                TensorMetadata::new(
                    node.output.dtype,
                    TensorShape::Ranked(Vec::new()),
                    node.output.device.clone(),
                )
            })
            .unwrap_or_else(TensorMetadata::unknown),
        TensorGraphOp::DeviceTransfer { target } => {
            let mut meta = input(0)
                .map(|node| node.output.clone())
                .unwrap_or_else(TensorMetadata::unknown);
            meta.device = target.clone();
            meta
        }
        TensorGraphOp::Elementwise { .. }
        | TensorGraphOp::FusedElementwise { .. }
        | TensorGraphOp::Dropout
        | TensorGraphOp::MaxPool2d
        | TensorGraphOp::BatchedMatmul
        | TensorGraphOp::Linear
        | TensorGraphOp::Conv2d => input(0)
            .map(|node| node.output.clone())
            .unwrap_or_else(TensorMetadata::unknown),
        TensorGraphOp::FusedReduction { .. } => input(0)
            .map(|node| {
                TensorMetadata::new(
                    node.output.dtype,
                    TensorShape::Ranked(Vec::new()),
                    node.output.device.clone(),
                )
            })
            .unwrap_or_else(TensorMetadata::unknown),
        TensorGraphOp::Parameter | TensorGraphOp::UnknownHost { .. } => TensorMetadata::unknown(),
    }
}

fn infer_create_metadata(
    name: &str,
    args: &[Value],
    constants: &HashMap<usize, i64>,
) -> TensorMetadata {
    let dtype = if name.ends_with("_f") || matches!(name, "uniform_f" | "normal_f" | "bernoulli") {
        TensorDType::Float
    } else {
        TensorDType::Int
    };
    let shape = match name {
        "zeros" | "ones" | "full" | "full_f" | "uniform" | "uniform_f" | "normal_f"
        | "bernoulli" => TensorShape::Ranked(vec![const_usize(args.first(), constants)]),
        "arange" => TensorShape::Ranked(vec![infer_arange_len(args, constants)]),
        "zeros2" | "ones2" | "full2" | "full2_f" => TensorShape::Ranked(vec![
            const_usize(args.first(), constants),
            const_usize(args.get(1), constants),
        ]),
        _ => TensorShape::Unknown,
    };
    TensorMetadata::new(dtype, shape, TensorDevice::Cpu)
}

fn infer_arange_len(args: &[Value], constants: &HashMap<usize, i64>) -> Option<usize> {
    let start = args
        .first()
        .and_then(|value| constants.get(&value.id))
        .copied()?;
    let end = args
        .get(1)
        .and_then(|value| constants.get(&value.id))
        .copied()?;
    let step = args
        .get(2)
        .and_then(|value| constants.get(&value.id))
        .copied()?;
    if step == 0 {
        return None;
    }
    let distance = if step > 0 {
        if start >= end {
            return Some(0);
        }
        end.saturating_sub(start)
    } else {
        if start <= end {
            return Some(0);
        }
        start.saturating_sub(end)
    };
    let step_abs = step.unsigned_abs();
    Some(distance.unsigned_abs().div_ceil(step_abs) as usize)
}

fn const_usize(value: Option<&Value>, constants: &HashMap<usize, i64>) -> Option<usize> {
    value
        .and_then(|value| constants.get(&value.id))
        .and_then(|value| usize::try_from(*value).ok())
}

fn device_from_code(code: i64) -> TensorDevice {
    match code {
        0 => TensorDevice::Cpu,
        6 => TensorDevice::Wgpu,
        other => TensorDevice::Reserved(other),
    }
}

fn input_pair<'a>(
    node: &TensorGraphNode,
    by_id: &'a BTreeMap<usize, &TensorGraphNode>,
) -> Option<[&'a TensorGraphNode; 2]> {
    Some([
        by_id.get(node.inputs.first()?)?,
        by_id.get(node.inputs.get(1)?)?,
    ])
}

fn validate_same_device(
    function: &str,
    node: &TensorGraphNode,
    left: &TensorGraphNode,
    right: &TensorGraphNode,
    errors: &mut Vec<TensorGraphError>,
) {
    if left.output.device != TensorDevice::Unknown
        && right.output.device != TensorDevice::Unknown
        && left.output.device != right.output.device
    {
        errors.push(TensorGraphError::new(
            function,
            Some(node.id),
            TensorGraphErrorKind::DeviceMismatch,
            format!(
                "{} expects operands on the same device, got {} and {}",
                node.op.stable_name(),
                left.output.device.stable_name(),
                right.output.device.stable_name()
            ),
        ));
    }
}

fn has_cycle(
    node_id: usize,
    by_id: &BTreeMap<usize, &TensorGraphNode>,
    visiting: &mut HashSet<usize>,
    visited: &mut HashSet<usize>,
) -> bool {
    if visited.contains(&node_id) {
        return false;
    }
    if !visiting.insert(node_id) {
        return true;
    }
    if let Some(node) = by_id.get(&node_id) {
        for input in &node.inputs {
            if has_cycle(*input, by_id, visiting, visited) {
                return true;
            }
        }
    }
    visiting.remove(&node_id);
    visited.insert(node_id);
    false
}

impl From<&Type> for TensorMetadata {
    fn from(ty: &Type) -> Self {
        match ty {
            Type::Tensor {
                dtype,
                rank,
                dims,
                layout,
                device,
            } => {
                let dtype = match dtype.as_ref() {
                    Type::Int => TensorDType::Int,
                    Type::Float => TensorDType::Float,
                    _ => TensorDType::Unknown,
                };
                let shape = dims
                    .clone()
                    .or_else(|| rank.map(|rank| vec![None; rank]))
                    .map(TensorShape::Ranked)
                    .unwrap_or(TensorShape::Unknown);
                let layout = match layout.as_deref() {
                    Some("contiguous") => TensorLayout::Contiguous,
                    _ => TensorLayout::Unknown,
                };
                let device = match device.as_deref() {
                    Some("cpu") => TensorDevice::Cpu,
                    Some("wgpu") => TensorDevice::Wgpu,
                    _ => TensorDevice::Unknown,
                };
                Self {
                    dtype,
                    shape,
                    layout,
                    device,
                }
            }
            _ => Self::unknown(),
        }
    }
}
