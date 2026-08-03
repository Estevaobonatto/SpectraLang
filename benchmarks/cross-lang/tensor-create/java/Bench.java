// Phase 31: tensor-create (Java)

public class Bench {
    public static void main(String[] args) {
        final int iters = 20;
        long total = 0L;
        for (int i = 0; i < iters; i++) {
            double[] t = new double[1_048_576];
            for (int j = 0; j < t.length; j++) {
                t[j] = 1.0;
            }
            total += t.length;
        }
        if (total != 20_971_520L) {
            System.err.println("unexpected: " + total);
            System.exit(1);
        }
    }
}
