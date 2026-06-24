// Phase 31: gcd (Java)

public class Bench {
    public static void main(String[] args) {
        final int iters = 1_000_000;
        int[] aVals = {48, 56, 1071, 1024, 270, 816, 462, 100, 75, 999};
        int[] bVals = {36, 42, 462, 768, 192, 204, 330, 75, 125, 333};
        long total = 0L;
        for (int it = 0; it < iters; it++) {
            long acc = 0L;
            for (int p = 0; p < 10; p++) {
                int a = aVals[p];
                int b = bVals[p];
                while (b != 0) {
                    int t = b;
                    b = a % b;
                    a = t;
                }
                acc += a;
            }
            total += acc;
        }
        if (total != 962L * (long) iters) {
            System.err.println("unexpected: " + total);
            System.exit(1);
        }
    }
}
