// Phase 31: digit-sum (Java)

public class Bench {
    public static void main(String[] args) {
        final int iters = 200;
        final int n = 10_000;
        long total = 0L;
        for (int it = 0; it < iters; it++) {
            long acc = 0L;
            for (int i = 1; i <= n; i++) {
                int x = i;
                long ds = 0L;
                while (x > 0) {
                    ds += x % 10;
                    x /= 10;
                }
                acc += ds;
            }
            total += acc;
        }
        if (total != 180_001L * (long) iters) {
            System.err.println("unexpected: " + total);
            System.exit(1);
        }
    }
}
