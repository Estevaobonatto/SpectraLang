// Phase 31: tensor-elementwise (Java)

public class Bench {
    public static void main(String[] args) {
        final int iters = 50;
        double checksum = 0.0;
        for (int i = 0; i < iters; i++) {
            double[] t = new double[100_000];
            for (int j = 0; j < t.length; j++) t[j] = 0.5;
            for (int j = 0; j < t.length; j++) if (t[j] < 0) t[j] = 0;
            checksum += t[0] + t[99_999];
        }
        if (checksum <= 0) {
            System.err.println("unexpected checksum");
            System.exit(1);
        }
    }
}
