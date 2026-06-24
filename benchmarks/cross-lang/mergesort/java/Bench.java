// Phase 31: mergesort (Java)

public class Bench {
    static void mergeInPlace(long[] arr, long[] scratch, int lo, int mid, int hi) {
        for (int i = lo; i < hi; i++) {
            scratch[i] = arr[i];
        }
        int l = lo, r = mid, k = lo;
        while (l < mid) {
            if (r >= hi) {
                while (l < mid) {
                    arr[k++] = scratch[l++];
                }
            } else {
                if (scratch[l] <= scratch[r]) {
                    arr[k++] = scratch[l++];
                } else {
                    arr[k++] = scratch[r++];
                }
            }
        }
        while (r < hi) {
            arr[k++] = scratch[r++];
        }
    }

    public static void main(String[] args) {
        final int iters = 30_000;
        long[] src = {
            5, 16, 27, 38, 49, 60, 71, 82, 93, 7, 18, 29, 40, 51, 62, 73,
            84, 95, 9, 20, 31, 42, 53, 64, 75, 86, 0, 11, 22, 33, 44, 55,
            66, 77, 88, 2, 13, 24, 35, 46, 57, 68, 79, 90, 4, 15, 26, 37,
            48, 59, 70, 81, 92, 6, 17, 28, 39, 50, 61, 72, 83, 94, 8, 19
        };
        long total = 0L;
        for (int it = 0; it < iters; it++) {
            long[] arr = new long[64];
            System.arraycopy(src, 0, arr, 0, 64);
            long[] scratch = new long[64];
            for (int w = 1; w < 64; w *= 2) {
                int step = w * 2;
                for (int lo = 0; lo < 64; lo += step) {
                    int mid = lo + w;
                    int hi = lo + step;
                    if (mid > 64) mid = 64;
                    if (hi > 64) hi = 64;
                    mergeInPlace(arr, scratch, lo, mid, hi);
                }
            }
            long checksum = 0L;
            for (int k = 0; k < 64; k++) {
                checksum += arr[k] * (k + 1);
            }
            total += checksum;
        }
        if (total != 130926L * (long) iters) {
            System.err.println("unexpected: " + total);
            System.exit(1);
        }
    }
}
