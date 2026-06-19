use spectra_api::conformance::run_v0_suite;
use std::env;
use std::fs;
use std::path::PathBuf;

fn main() {
    let report = run_v0_suite();
    let json = report.to_json_string();
    let mut args = env::args().skip(1);
    let mut output: Option<PathBuf> = None;
    while let Some(arg) = args.next() {
        if arg == "--output" {
            output = args.next().map(PathBuf::from);
        }
    }

    if let Some(path) = output {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create report directory");
        }
        fs::write(&path, json.as_bytes()).expect("write conformance report");
    } else {
        println!("{json}");
    }

    if !report.is_success() {
        std::process::exit(1);
    }
}
