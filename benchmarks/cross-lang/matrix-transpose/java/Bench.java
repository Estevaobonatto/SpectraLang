// Phase 31: matrix-transpose (Java)

public class Bench {
    public static void main(String[] args) {
        final int iters = 20_000;
        final int rows = 16;
        final int cols = 16;
        long total = 0L;
        int[] m = new int[rows * cols];
        for (int it = 0; it < iters; it++) {
            for (int r = 0; r < rows; r++) {
                for (int c = 0; c < cols; c++) {
                    m[r * cols + c] = r * cols + c;
                }
            }
            long tChecksum = 0L;
            for (int r = 0; r < rows; r++) {
                for (int c = 0; c < cols; c++) {
                    int val = m[r * cols + c];
                    int tPos = c * rows + r;
                    tChecksum += (long) val * (long) (tPos + 1);
                }
            }
            total += tChecksum;
        }
        if (total != 4_368_320L * (long) iters) {
            System.err.println("unexpected: " + total);
            System.exit(1);
        }
    }
}
