#ifndef SPECTRA_INTEROP_H
#define SPECTRA_INTEROP_H

#include <stdint.h>
#include <stddef.h>

#ifdef __cplusplus
extern "C" {
#endif

#define SPECTRA_INTEROP_OK 0
#define SPECTRA_INTEROP_INVALID_ARGUMENT 1
#define SPECTRA_INTEROP_IO_ERROR 2
#define SPECTRA_INTEROP_FORMAT_ERROR 3

typedef struct SpectraF64Array {
    double *data;
    size_t len;
} SpectraF64Array;

uint32_t spectra_interop_abi_version(void);
int64_t spectra_interop_add_i64(int64_t lhs, int64_t rhs);
int8_t spectra_interop_identity_i8(int8_t value);
uint8_t spectra_interop_identity_u8(uint8_t value);
int16_t spectra_interop_identity_i16(int16_t value);
uint16_t spectra_interop_identity_u16(uint16_t value);
int32_t spectra_interop_identity_i32(int32_t value);
uint32_t spectra_interop_identity_u32(uint32_t value);
int64_t spectra_interop_identity_i64(int64_t value);
uint64_t spectra_interop_identity_u64(uint64_t value);
float spectra_interop_identity_f32(float value);
double spectra_interop_identity_f64(double value);
int32_t spectra_interop_checked_i64_to_i8(int64_t value, int8_t *out);
double spectra_interop_tensor_f64_sum(const double *data, size_t len);
int32_t spectra_interop_npy_write_f64(
    const char *path,
    size_t path_len,
    const double *data,
    size_t len
);
SpectraF64Array spectra_interop_npy_read_f64(const char *path, size_t path_len);
void spectra_interop_f64_array_free(SpectraF64Array array);

#ifdef __cplusplus
}
#endif

#endif
