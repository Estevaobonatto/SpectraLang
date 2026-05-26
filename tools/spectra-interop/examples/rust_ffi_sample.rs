use spectra_interop::TensorF64;

fn main() {
    let path = std::env::temp_dir().join("spectra_rust_ffi_sample.npy");
    let tensor = TensorF64::new(vec![1.0, 2.0, 3.0, 4.0]);
    tensor.write_npy(&path).expect("write npy");
    let loaded = TensorF64::read_npy(&path).expect("read npy");
    let _ = std::fs::remove_file(&path);
    assert_eq!(loaded.sum(), 10.0);
    println!("rust ffi sample ok: sum={}", loaded.sum());
}
