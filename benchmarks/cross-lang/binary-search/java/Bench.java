// Phase 31: binary-search (Java)

public class Bench {
    public static void main(String[] args) {
        final int iters = 1_000_000;
        int[] values = {0, 2, 4, 6, 8, 10, 12, 14, 16, 18, 20, 22, 24, 26, 28, 30};
        final int n = 16;
        int[] targets = {14, 3, 28, 100};
        long total = 0L;
        for (int it = 0; it < iters; it++) {
            long acc = 0L;
            for (int t = 0; t < 4; t++) {
                int target = targets[t];
                int low = 0, high = n - 1;
                long found = -1L;
                while (low <= high) {
                    int mid = (low + high) / 2;
                    if (values[mid] == target) {
                        found = mid;
                        low = high + 1;
                    } else if (values[mid] < target) {
                        low = mid + 1;
                    } else {
                        high = mid - 1;
                    }
                }
                acc += found;
            }
            total += acc;
        }
        if (total != 19L * (long) iters) {
            System.err.println("unexpected: " + total);
            System.exit(1);
        }
    }
}
