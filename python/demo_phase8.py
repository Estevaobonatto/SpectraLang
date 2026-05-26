from pathlib import Path

from spectra_bridge import read_npy, run_spectra_main, write_npy


def main() -> int:
    repo = Path(__file__).resolve().parents[1]
    program = repo / "tests" / "validation" / "01_basic_syntax.spectra"
    result = run_spectra_main([program], cwd=repo)
    if result.returncode != 0:
        print(result.stderr)
        return result.returncode

    path = repo / "target" / "spectra_python_roundtrip.npy"
    write_npy(path, [1.0, 2.0, 3.5])
    loaded = read_npy(path)
    path.unlink(missing_ok=True)
    if float(loaded.sum()) != 6.5:
        return 10
    print("python interop ok")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
