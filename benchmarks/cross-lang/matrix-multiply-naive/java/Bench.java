// Phase 31: matrix-multiply-naive (Java)

public class Bench {
    public static void main(String[] args) {
        final int iters = 20_000;
        final int n = 16;
        long total = 0L;
        for (int it = 0; it < iters; it++) {
            long[] a = new long[n * n];
            long[] b = new long[n * n];
            long[] c = new long[n * n];
            for (int i = 0; i < n; i++) {
                for (int j = 0; j < n; j++) {
                    int v = i + j;
                    a[i * n + j] = (long) (v - (v / 100) * 100);
                }
            }
            for (int i = 0; i < n; i++) {
                for (int j = 0; j < n; j++) {
                    int v = i * 2 + j;
                    b[i * n + j] = (long) (v - (v / 100) * 100);
                }
            }
            for (int i = 0; i < n; i++) {
                for (int k = 0; k < n; k++) {
                    long aik = a[i * n + k];
                    for (int j = 0; j < n; j++) {
                        c[i * n + j] += aik * b[k * n + j];
                    }
                }
            }
            long checksum = 0L;
            for (int i = 0; i < n; i++) {
                for (int j = 0; j < n; j++) {
                    checksum += c[i * n + j] * (i * n + j + 1);
                }
            }
            total += checksum;
        }
        if (total != 232647680L * (long) iters) {
            System.err.println("unexpected: " + total);
            System.exit(1);
        }
    }
}
