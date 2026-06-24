// Phase 31: sort-int (Java)

public class Bench {
    public static void main(String[] args) {
        final int iters = 50_000;
        final int n = 16;
        long total = 0L;
        int[] base = {9, 1, 5, 3, 7, 2, 8, 4, 0, 6, 11, 10, 15, 13, 14, 12};
        for (int it = 0; it < iters; it++) {
            int[] values = base.clone();
            for (int outer = 0; outer < n; outer++) {
                for (int inner = 0; inner < n - 1; inner++) {
                    if (values[inner] > values[inner + 1]) {
                        int tmp = values[inner];
                        values[inner] = values[inner + 1];
                        values[inner + 1] = tmp;
                    }
                }
            }
            long checksum = 0L;
            for (int k = 0; k < n; k++) {
                checksum += (long) values[k] * (long) (k + 1);
            }
            total += checksum;
        }
        if (total != 1360L * (long) iters) {
            System.err.println("unexpected: " + total);
            System.exit(1);
        }
    }
}
