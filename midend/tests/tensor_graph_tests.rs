use spectra_compiler::{Lexer, Parser};
use spectra_midend::{
    ASTLowering, TensorDType, TensorDevice, TensorGraph, TensorGraphErrorKind, TensorGraphFunction,
    TensorGraphNode, TensorGraphOp, TensorGraphSource, TensorMetadata, TensorShape,
};
use std::collections::HashSet;
use std::fs;
use std::path::Path;

fn assert_snapshot(name: &str, actual: &str) {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("snapshots")
        .join(name);
    let expected = fs::read_to_string(&path)
        .unwrap_or_else(|err| panic!("failed to read snapshot {}: {err}", path.display()));
    assert_eq!(
        actual.trim_end(),
        expected.trim_end(),
        "snapshot {name} changed"
    );
}

fn lower_source(source: &str) -> TensorGraph {
    let tokens = Lexer::new(source).tokenize().expect("lexing should pass");
    let ast = Parser::new(tokens, HashSet::new())
        .parse()
        .expect("parsing should pass");
    let ir = ASTLowering::new()
        .lower_module(&ast)
        .expect("lowering should pass");
    TensorGraph::from_ir_module(&ir)
}

#[test]
fn tensor_graph_snapshot_covers_real_lowered_program() {
    let graph = lower_source(
        r#"
        module tensor_graph_snapshot;

        pub fn main() -> int {
            let flat = std.tensor.arange(1, 5, 1);
            let matrix = std.tensor.reshape(flat, 2, 2);
            let product = std.tensor.matmul(matrix, matrix);
            let activated = std.tensor.relu(product);
            std.tensor.free_all();
            return std.tensor.len(activated);
        }
        "#,
    );

    graph.validate().expect("graph should validate");
    assert_snapshot("tensor_graph.snap", &graph.stable_dump());
}

#[test]
fn tensor_graph_infers_negative_arange_shape() {
    let graph = lower_source(
        r#"
        module tensor_graph_negative_arange;

        pub fn main() -> int {
            let values = std.tensor.arange(5, 1, -2);
            return std.tensor.len(values);
        }
        "#,
    );

    let dump = graph.stable_dump();
    assert!(
        dump.contains("create.arange(-) -> dtype=int shape=[2]"),
        "{dump}"
    );
    graph
        .validate()
        .expect("negative arange graph should validate");
}

#[test]
fn tensor_graph_validation_catches_matmul_shape_mismatch() {
    let graph = TensorGraph {
        module: "manual".to_string(),
        functions: vec![TensorGraphFunction {
            name: "main".to_string(),
            nodes: vec![
                node(
                    0,
                    TensorGraphOp::Parameter,
                    vec![],
                    TensorMetadata::new(
                        TensorDType::Float,
                        TensorShape::Ranked(vec![Some(2), Some(3)]),
                        TensorDevice::Cpu,
                    ),
                ),
                node(
                    1,
                    TensorGraphOp::Parameter,
                    vec![],
                    TensorMetadata::new(
                        TensorDType::Float,
                        TensorShape::Ranked(vec![Some(4), Some(2)]),
                        TensorDevice::Cpu,
                    ),
                ),
                node(
                    2,
                    TensorGraphOp::Matmul,
                    vec![0, 1],
                    TensorMetadata::new(
                        TensorDType::Float,
                        TensorShape::Ranked(vec![Some(2), Some(2)]),
                        TensorDevice::Cpu,
                    ),
                ),
            ],
        }],
    };

    let errors = graph.validate().expect_err("shape mismatch should fail");
    assert!(errors
        .iter()
        .any(|error| error.kind == TensorGraphErrorKind::ShapeMismatch));
}

#[test]
fn tensor_graph_validation_catches_device_mismatch() {
    let graph = TensorGraph {
        module: "manual".to_string(),
        functions: vec![TensorGraphFunction {
            name: "main".to_string(),
            nodes: vec![
                node(
                    0,
                    TensorGraphOp::Parameter,
                    vec![],
                    TensorMetadata::new(
                        TensorDType::Float,
                        TensorShape::Ranked(vec![Some(4)]),
                        TensorDevice::Cpu,
                    ),
                ),
                node(
                    1,
                    TensorGraphOp::Parameter,
                    vec![],
                    TensorMetadata::new(
                        TensorDType::Float,
                        TensorShape::Ranked(vec![Some(4)]),
                        TensorDevice::Wgpu,
                    ),
                ),
                node(
                    2,
                    TensorGraphOp::Elementwise {
                        name: "add".to_string(),
                    },
                    vec![0, 1],
                    TensorMetadata::new(
                        TensorDType::Float,
                        TensorShape::Ranked(vec![Some(4)]),
                        TensorDevice::Cpu,
                    ),
                ),
            ],
        }],
    };

    let errors = graph.validate().expect_err("device mismatch should fail");
    assert!(errors
        .iter()
        .any(|error| error.kind == TensorGraphErrorKind::DeviceMismatch));
}

#[test]
fn tensor_graph_validation_catches_cycles() {
    let graph = TensorGraph {
        module: "manual".to_string(),
        functions: vec![TensorGraphFunction {
            name: "main".to_string(),
            nodes: vec![
                node(
                    0,
                    TensorGraphOp::Elementwise {
                        name: "relu".to_string(),
                    },
                    vec![1],
                    TensorMetadata::unknown(),
                ),
                node(
                    1,
                    TensorGraphOp::Elementwise {
                        name: "relu".to_string(),
                    },
                    vec![0],
                    TensorMetadata::unknown(),
                ),
            ],
        }],
    };

    let errors = graph.validate().expect_err("cycle should fail");
    assert!(errors
        .iter()
        .any(|error| error.kind == TensorGraphErrorKind::Cycle));
}

fn node(
    id: usize,
    op: TensorGraphOp,
    inputs: Vec<usize>,
    output: TensorMetadata,
) -> TensorGraphNode {
    TensorGraphNode {
        id,
        value: Some(id),
        op,
        inputs,
        output,
        source: TensorGraphSource {
            block: 0,
            instruction: id,
            host: None,
        },
    }
}
