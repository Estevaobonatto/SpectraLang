from __future__ import annotations

import ctypes
import os
import subprocess
from dataclasses import dataclass
from pathlib import Path
from typing import Iterable, Sequence


class SpectraBridgeError(RuntimeError):
    pass


@dataclass(frozen=True)
class SpectraRunResult:
    returncode: int
    stdout: str
    stderr: str


def run_spectra_main(paths: Sequence[str | os.PathLike[str]], cwd: str | os.PathLike[str] | None = None) -> SpectraRunResult:
    repo = Path(cwd) if cwd is not None else Path(__file__).resolve().parents[1]
    command = ["cargo", "run", "-p", "spectra-cli", "--", "run", *[str(path) for path in paths]]
    completed = subprocess.run(command, cwd=repo, text=True, capture_output=True)
    return SpectraRunResult(completed.returncode, completed.stdout, completed.stderr)


def write_npy(path: str | os.PathLike[str], values: Iterable[float]) -> None:
    try:
        import numpy as np
    except ImportError as exc:
        raise SpectraBridgeError("numpy is required for Spectra Python tensor exchange") from exc
    array = np.asarray(list(values), dtype="<f8")
    np.save(Path(path), array, allow_pickle=False)


def read_npy(path: str | os.PathLike[str]):
    try:
        import numpy as np
    except ImportError as exc:
        raise SpectraBridgeError("numpy is required for Spectra Python tensor exchange") from exc
    return np.load(Path(path), allow_pickle=False)


class SpectraInteropLibrary:
    def __init__(self, library_path: str | os.PathLike[str]):
        self._lib = ctypes.CDLL(str(library_path))
        self._lib.spectra_interop_abi_version.restype = ctypes.c_uint32
        self._lib.spectra_interop_add_i64.argtypes = [ctypes.c_int64, ctypes.c_int64]
        self._lib.spectra_interop_add_i64.restype = ctypes.c_int64
        self._lib.spectra_interop_tensor_f64_sum.argtypes = [
            ctypes.POINTER(ctypes.c_double),
            ctypes.c_size_t,
        ]
        self._lib.spectra_interop_tensor_f64_sum.restype = ctypes.c_double

    def abi_version(self) -> int:
        return int(self._lib.spectra_interop_abi_version())

    def add_i64(self, lhs: int, rhs: int) -> int:
        return int(self._lib.spectra_interop_add_i64(lhs, rhs))

    def sum_f64(self, values: Sequence[float]) -> float:
        array_type = ctypes.c_double * len(values)
        array = array_type(*values)
        return float(self._lib.spectra_interop_tensor_f64_sum(array, len(values)))
