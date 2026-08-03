// Phase 31: count-primes (Java)

public class Bench {
    public static void main(String[] args) {
        final int iters = 500;
        final int n = 500;
        long total = 0L;
        for (int it = 0; it < iters; it++) {
            long count = 0L;
            for (int i = 2; i <= n; i++) {
                int isPrime = 1;
                for (int d = 2; d * d <= i; d++) {
                    if (i % d == 0) isPrime = 0;
                }
                if (isPrime == 1) count++;
            }
            total += count;
        }
        if (total != 95L * (long) iters) {
            System.err.println("unexpected: " + total);
            System.exit(1);
        }
    }
}
