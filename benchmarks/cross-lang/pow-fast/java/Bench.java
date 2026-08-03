// Phase 31: pow-fast (Java)

public class Bench {
    public static void main(String[] args) {
        final int iters = 50_000;
        int[] bases = {2, 3, 5, 7, 10, 13, 2, 4, 6, 8};
        int[] exps = {10, 8, 6, 5, 4, 3, 20, 15, 12, 10};
        long total = 0L;
        for (int it = 0; it < iters; it++) {
            long acc = 0L;
            for (int p = 0; p < 10; p++) {
                long base = bases[p];
                int exp = exps[p];
                long result = 1L;
                while (exp > 0) {
                    if (exp % 2 == 1) result *= base;
                    base *= base;
                    exp /= 2;
                }
                acc += result;
            }
            total += acc;
        }
        if (total != 4_325_366_774L * (long) iters) {
            System.err.println("unexpected: " + total);
            System.exit(1);
        }
    }
}
