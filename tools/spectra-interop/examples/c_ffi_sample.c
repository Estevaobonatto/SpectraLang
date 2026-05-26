#include <stdio.h>
#include <string.h>
#include "spectra_interop.h"

int main(void) {
    const char *path = "spectra_c_ffi_sample.npy";
    double values[4] = {1.0, 2.0, 3.0, 4.0};
    int status = spectra_interop_npy_write_f64(path, strlen(path), values, 4);
    if (status != SPECTRA_INTEROP_OK) {
        return status;
    }

    SpectraF64Array loaded = spectra_interop_npy_read_f64(path, strlen(path));
    if (loaded.data == NULL || loaded.len != 4) {
        return 10;
    }

    double sum = spectra_interop_tensor_f64_sum(loaded.data, loaded.len);
    spectra_interop_f64_array_free(loaded);
    if (sum != 10.0) {
        return 11;
    }

    printf("c ffi sample ok: sum=%.1f\n", sum);
    return 0;
}
