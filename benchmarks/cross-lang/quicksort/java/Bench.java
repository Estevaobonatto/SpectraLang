// Phase 31: quicksort (Java)

public class Bench {
    static int partition(long[] arr, int lo, int hi) {
        long pivot = arr[hi];
        int i = lo;
        for (int j = lo; j < hi; j++) {
            if (arr[j] < pivot) {
                long tmp = arr[i];
                arr[i] = arr[j];
                arr[j] = tmp;
                i++;
            }
        }
        long tmp = arr[i];
        arr[i] = arr[hi];
        arr[hi] = tmp;
        return i;
    }

    static void qs(long[] arr, int lo, int hi) {
        if (lo >= hi) return;
        int p = partition(arr, lo, hi);
        qs(arr, lo, p - 1);
        qs(arr, p + 1, hi);
    }

    public static void main(String[] args) {
        final int iters = 50_000;
        long[] src = {
            3, 10, 17, 24, 31, 38, 45, 52, 59, 66, 73, 80, 87, 94, 1, 8,
            15, 22, 29, 36, 43, 50, 57, 64, 71, 78, 85, 92, 99, 6, 13, 20,
            27, 34, 41, 48, 55, 62, 69, 76, 83, 90, 97, 4, 11, 18, 25, 32,
            39, 46, 53, 60, 67, 74, 81, 88, 95, 2, 9, 16, 23, 30, 37, 44
        };
        long total = 0L;
        for (int it = 0; it < iters; it++) {
            long[] arr = new long[64];
            System.arraycopy(src, 0, arr, 0, 64);
            qs(arr, 0, 63);
            long checksum = 0L;
            for (int k = 0; k < 64; k++) {
                checksum += arr[k] * (k + 1);
            }
            total += checksum;
        }
        if (total != 131629L * (long) iters) {
            System.err.println("unexpected: " + total);
            System.exit(1);
        }
    }
}
