// Phase 31: hashmap-churn (Java)

import java.util.HashMap;

public class Bench {
    public static void main(String[] args) {
        final int iters = 2_000;
        long total = 0L;
        for (int it = 0; it < iters; it++) {
            HashMap<Long, Long> m = new HashMap<>();
            for (int i = 0; i < 500; i++) {
                m.put((long) i, (long) (i * 2));
            }
            for (int k = 0; k < 500; k++) {
                if (k % 2 == 1) {
                    m.remove((long) k);
                }
            }
            for (int j = 0; j < 250; j++) {
                m.put((long) (500 + j), (long) ((500 + j) * 2));
            }
            long acc = 0L;
            for (int x = 0; x < 750; x++) {
                if (m.containsKey((long) x)) {
                    acc += x;
                }
            }
            total += acc;
        }
        if (total != 218375L * (long) iters) {
            System.err.println("unexpected: " + total);
            System.exit(1);
        }
    }
}
