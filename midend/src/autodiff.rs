//! Compiler-visible reverse-mode autodiff graph.
//!
//! The runtime still owns tensor storage and kernel execution, but this module
//! owns the differentiation contract: which forward operations participate,
//! which values must be retained, and which reverse rule is selected.

use crate::tensor_graph::{TensorGraph, TensorGraphFunction, TensorGraphOp, TensorMetadata, TensorGraphSource};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AutodiffGraph {
    pub schema: &'static str,
    pub functions: Vec<AutodiffFunction>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AutodiffFunction {
    pub name: String,
    pub forward: Vec<AutodiffNode>,
    pub backward: Vec<AutodiffNode>,
    pub loss_nodes: Vec<usize>,
    /// Compatibility accessor for consumers that only support one loss.
    pub loss_node: Option<usize>,
    pub diagnostics: Vec<AutodiffDiagnostic>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AutodiffNode {
    pub id: usize,
    pub kind: AutodiffNodeKind,
    pub inputs: Vec<usize>,
    pub output: TensorMetadata,
    pub source: TensorGraphSource,
    pub rule: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AutodiffNodeKind {
    Forward { op: String },
    SaveForBackward { forward_node: usize },
    BackwardSeed { loss_node: usize },
    Gradient { target_node: usize, op: String },
    AccumulateGradient { target_node: usize },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AutodiffDiagnostic {
    pub code: &'static str,
    pub node: usize,
    pub operation: String,
    pub message: String,
}

impl AutodiffGraph {
    pub const SCHEMA: &'static str = "spectralang.r3004_autodiff_ir.v1";

    pub fn from_tensor_graph(graph: &TensorGraph) -> Self {
        Self {
            schema: Self::SCHEMA,
            functions: graph
                .functions
                .iter()
                .map(AutodiffFunction::from_tensor_function)
                .collect(),
        }
    }

    pub fn has_gradient_nodes(&self) -> bool {
        self.functions.iter().any(|function| !function.backward.is_empty())
    }

    pub fn stable_dump(&self) -> String {
        let mut out = format!("autodiff_ir schema={}\n", self.schema);
        for function in &self.functions {
            out.push_str(&format!("fn {} losses={:?}\n", function.name, function.loss_nodes));
            for node in &function.forward {
                out.push_str(&format!("  forward %{} {} rule={} inputs={:?}\n", node.id, node_name(&node.kind), node.rule, node.inputs));
            }
            for node in &function.backward {
                out.push_str(&format!("  backward %{} {} rule={} inputs={:?}\n", node.id, node_name(&node.kind), node.rule, node.inputs));
            }
            for diagnostic in &function.diagnostics {
                out.push_str(&format!("  diagnostic {} node={} op={} {}\n", diagnostic.code, diagnostic.node, diagnostic.operation, diagnostic.message));
            }
        }
        out
    }
}

impl AutodiffFunction {
    fn from_tensor_function(function: &TensorGraphFunction) -> Self {
        let forward = function
            .nodes
            .iter()
            .map(|node| AutodiffNode {
                id: node.id,
                kind: AutodiffNodeKind::Forward {
                    op: node.op.stable_name(),
                },
                inputs: node.inputs.clone(),
                output: node.output.clone(),
                source: node.source.clone(),
                rule: gradient_rule(&node.op).unwrap_or_else(|| "none".to_string()),
            })
            .collect::<Vec<_>>();

        let loss_nodes = function
            .nodes
            .iter()
            .filter(|node| is_loss_candidate(&node.op))
            .map(|node| node.id)
            .collect::<Vec<_>>();
        let loss_node = loss_nodes.first().copied();

        let mut backward = Vec::new();
        let mut diagnostics = Vec::new();
        if !loss_nodes.is_empty() {
            let mut reachable = std::collections::BTreeSet::new();
            for loss in &loss_nodes {
                collect_ancestors(*loss, function, &mut reachable);
            }
            for loss in &loss_nodes {
            backward.push(AutodiffNode {
                id: 0,
                kind: AutodiffNodeKind::BackwardSeed { loss_node: *loss },
                inputs: vec![*loss],
                output: function.nodes[*loss].output.clone(),
                source: function.nodes[*loss].source.clone(),
                rule: "seed=1".to_string(),
            });
            }

            for node in function.nodes.iter().rev() {
                if !reachable.contains(&node.id)
                    || matches!(node.op, TensorGraphOp::Parameter | TensorGraphOp::Create { .. })
                {
                    continue;
                }
                if is_autodiff_auxiliary(&node.op) {
                    continue;
                }
                match gradient_rule(&node.op) {
                    Some(rule) => {
                        let save_id = backward.len();
                        backward.push(AutodiffNode {
                            id: save_id,
                            kind: AutodiffNodeKind::SaveForBackward { forward_node: node.id },
                            inputs: vec![node.id],
                            output: node.output.clone(),
                            source: node.source.clone(),
                            rule: "save_forward_value".to_string(),
                        });
                        backward.push(AutodiffNode {
                            id: backward.len(),
                            kind: AutodiffNodeKind::Gradient {
                                target_node: node.id,
                                op: node.op.stable_name(),
                            },
                            inputs: node.inputs.clone(),
                            output: node.output.clone(),
                            source: node.source.clone(),
                            rule,
                        });
                        if node.inputs.len() > 1 {
                            backward.push(AutodiffNode {
                                id: backward.len(),
                                kind: AutodiffNodeKind::AccumulateGradient {
                                    target_node: node.inputs[0],
                                },
                                inputs: node.inputs.clone(),
                                output: node.output.clone(),
                                source: node.source.clone(),
                                rule: "stable_sum".to_string(),
                            });
                        }
                    }
                    None => diagnostics.push(AutodiffDiagnostic {
                        code: "E3004",
                        node: node.id,
                        operation: node.op.stable_name(),
                        message: "operation has no registered compiler-native gradient rule".to_string(),
                    }),
                }
            }
        }

        Self {
            name: function.name.clone(),
            forward,
            backward,
            loss_nodes,
            loss_node,
            diagnostics,
        }
    }
}

fn is_loss_candidate(op: &TensorGraphOp) -> bool {
    matches!(op, TensorGraphOp::Reduction { .. } | TensorGraphOp::Loss { .. })
        || matches!(op, TensorGraphOp::Elementwise { name } if name == "dot_t")
}

fn collect_ancestors(
    node_id: usize,
    function: &TensorGraphFunction,
    reachable: &mut std::collections::BTreeSet<usize>,
) {
    if !reachable.insert(node_id) {
        return;
    }
    if let Some(node) = function.nodes.iter().find(|node| node.id == node_id) {
        for input in &node.inputs {
            collect_ancestors(*input, function, reachable);
        }
    }
}

fn gradient_rule(op: &TensorGraphOp) -> Option<String> {
    match op {
        TensorGraphOp::Elementwise { name } => match name.as_str() {
            "add" => Some("d(a+b)=(g,g)".to_string()),
            "sub" => Some("d(a-b)=(g,-g)".to_string()),
            "mul" => Some("d(a*b)=(g*b,g*a)".to_string()),
            "div" => Some("d(a/b)=(g/b,-g*a/(b*b))".to_string()),
            "neg" => Some("d(-a)=-g".to_string()),
            "exp" | "exp_f" => Some("d(exp(a))=g*exp(a)".to_string()),
            "log" | "log_f" => Some("d(log(a))=g/a".to_string()),
            "relu" => Some("d(relu(a))=g*(a>0)".to_string()),
            "sigmoid" | "sigmoid_f" => Some("d(sigmoid(a))=g*y*(1-y)".to_string()),
            _ => None,
        },
        TensorGraphOp::Reduction { name } => match name.as_str() {
            "sum_t" => Some("broadcast(g,input_shape)".to_string()),
            "mean_t" => Some("broadcast(g/input_size,input_shape)".to_string()),
            "dot_t" => Some("(g*b,g*a)".to_string()),
            _ => None,
        },
        TensorGraphOp::Matmul => Some("(g@transpose(b),transpose(a)@g)".to_string()),
        TensorGraphOp::Transpose => Some("transpose(g)".to_string()),
        TensorGraphOp::Reshape => Some("reshape(g,input_shape)".to_string()),
        TensorGraphOp::DeviceTransfer { .. } => Some("identity_with_device_transfer(g)".to_string()),
        TensorGraphOp::Linear => Some("linear_backward(input,weight,bias,g)".to_string()),
        TensorGraphOp::Loss { name } if name == "mse_loss" => {
            Some("2*(prediction-target)/count".to_string())
        }
        _ => None,
    }
}

fn is_autodiff_auxiliary(op: &TensorGraphOp) -> bool {
    matches!(op, TensorGraphOp::UnknownHost { host }
        if host.ends_with(".requires_grad")
            || host.ends_with(".grad")
            || host.ends_with(".free_all")
            || host.ends_with(".set_grad_enabled"))
}

fn node_name(kind: &AutodiffNodeKind) -> String {
    match kind {
        AutodiffNodeKind::Forward { op } => format!("forward.{op}"),
        AutodiffNodeKind::SaveForBackward { forward_node } => format!("save(%{forward_node})"),
        AutodiffNodeKind::BackwardSeed { loss_node } => format!("seed(%{loss_node})"),
        AutodiffNodeKind::Gradient { target_node, op } => format!("grad(%{target_node},{op})"),
        AutodiffNodeKind::AccumulateGradient { target_node } => format!("accumulate(%{target_node})"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tensor_graph::{TensorDType, TensorDevice, TensorGraphNode, TensorGraphOp, TensorGraphSource, TensorShape};

    #[test]
    fn builds_reverse_rule_and_accumulation_for_mul_sum() {
        let source = TensorGraphSource { block: 0, instruction: 0, host: None };
        let graph = TensorGraph {
            module: "test".into(),
            functions: vec![TensorGraphFunction {
                name: "loss".into(),
                nodes: vec![
                    TensorGraphNode { id: 0, value: Some(0), op: TensorGraphOp::Parameter, inputs: vec![], output: TensorMetadata::unknown(), source: source.clone() },
                    TensorGraphNode { id: 1, value: Some(1), op: TensorGraphOp::Elementwise { name: "mul".into() }, inputs: vec![0, 0], output: TensorMetadata::new(TensorDType::Float, TensorShape::Ranked(vec![Some(3)]), TensorDevice::Cpu), source: source.clone() },
                    TensorGraphNode { id: 2, value: Some(2), op: TensorGraphOp::Reduction { name: "sum_t".into() }, inputs: vec![1], output: TensorMetadata::new(TensorDType::Float, TensorShape::Ranked(vec![]), TensorDevice::Cpu), source },
                ],
            }],
        };
        let autodiff = AutodiffGraph::from_tensor_graph(&graph);
        let function = &autodiff.functions[0];
        assert_eq!(function.loss_node, Some(2));
        assert!(function.backward.iter().any(|node| matches!(node.kind, AutodiffNodeKind::Gradient { target_node: 1, .. })));
        assert!(function.backward.iter().any(|node| matches!(node.kind, AutodiffNodeKind::AccumulateGradient { .. })));
        assert!(function.diagnostics.is_empty());
    }

    #[test]
    fn reports_unsupported_gradient_rule() {
        let graph = TensorGraph {
            module: "test".into(),
            functions: vec![TensorGraphFunction {
                name: "loss".into(),
                nodes: vec![TensorGraphNode {
                    id: 0,
                    value: Some(0),
                    op: TensorGraphOp::Elementwise { name: "tanh_f".into() },
                    inputs: vec![],
                    output: TensorMetadata::unknown(),
                    source: TensorGraphSource { block: 0, instruction: 0, host: None },
                }, TensorGraphNode {
                    id: 1,
                    value: Some(1),
                    op: TensorGraphOp::Loss { name: "mse_loss".into() },
                    inputs: vec![0],
                    output: TensorMetadata::unknown(),
                    source: TensorGraphSource { block: 0, instruction: 1, host: None },
                }],
            }],
        };
        let autodiff = AutodiffGraph::from_tensor_graph(&graph);
        assert!(autodiff.functions[0].backward.iter().any(|node| matches!(node.kind, AutodiffNodeKind::BackwardSeed { .. })));
        assert_eq!(autodiff.functions[0].diagnostics[0].code, "E3004");
    }
}
