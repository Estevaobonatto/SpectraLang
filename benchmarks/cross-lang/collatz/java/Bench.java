// Phase 31: 3n-plus-1 (Java)

public class Bench {
    public static void main(String[] args) {
        final int iters = 1_000;
        long total = 0L;
        for (int it = 0; it < iters; it++) {
            long acc = 0L;
            for (long n = 1; n <= 1000; n++) {
                long x = n;
                long steps = 0L;
                while (x != 1) {
                    if (x % 2 == 0) {
                        x /= 2;
                    } else {
                        x = 3 * x + 1;
                    }
                    steps++;
                }
                acc += steps;
            }
            total += acc;
        }
        if (total != 59542L * (long) iters) {
            System.err.println("unexpected: " + total);
            System.exit(1);
        }
    }
}
