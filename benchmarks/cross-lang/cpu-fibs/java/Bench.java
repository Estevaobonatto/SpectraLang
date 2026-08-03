// Phase 31: cpu-fibs (Java)

public class Bench {
    public static void main(String[] args) {
        final int iters = 200_000;
        long total = 0L;
        for (int k = 0; k < iters; k++) {
            long a = 0L, b = 1L;
            for (int i = 0; i < 40; i++) {
                long c = a + b;
                a = b;
                b = c;
            }
            total += a;
        }
        if (total != 20466831000000L) {
            System.err.println("unexpected: " + total);
            System.exit(1);
        }
    }
}
