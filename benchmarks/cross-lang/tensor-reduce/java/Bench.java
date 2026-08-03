// Phase 31: tensor-reduce (Java)

public class Bench {
    public static void main(String[] args) {
        final int iters = 50;
        double total = 0.0;
        for (int i = 0; i < iters; i++) {
            double[] t = new double[100_000];
            for (int j = 0; j < t.length; j++) t[j] = 1.0;
            double s = 0.0;
            for (double v : t) s += v;
            total += s;
        }
        if (total < 4_999_999.0 || total > 5_000_001.0) {
            System.err.println("unexpected: " + total);
            System.exit(1);
        }
    }
}
