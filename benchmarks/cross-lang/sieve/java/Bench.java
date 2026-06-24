// Phase 31: sieve (Java)

public class Bench {
    public static void main(String[] args) {
        final int iters = 2_000;
        final int n = 200;
        long total = 0L;
        for (int it = 0; it < iters; it++) {
            int[] sieve = new int[n + 1];
            for (int p = 2; p * p <= n; p++) {
                if (sieve[p] == 0) {
                    for (int multiple = p * p; multiple <= n; multiple += p) {
                        if (sieve[multiple] == 0) {
                            sieve[multiple] = 1;
                        }
                    }
                }
            }
            long count = 0L;
            for (int k = 2; k <= n; k++) {
                if (sieve[k] == 0) count++;
            }
            total += count;
        }
        if (total != 46L * (long) iters) {
            System.err.println("unexpected: " + total);
            System.exit(1);
        }
    }
}
