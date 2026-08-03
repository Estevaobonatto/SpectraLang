// Phase 31: tensor-matmul (Java)

public class Bench {
    public static void main(String[] args) {
        final int n = 64;
        final int iters = 20;
        double checksum = 0.0;
        for (int i = 0; i < iters; i++) {
            double[] a = new double[n * n];
            double[] b = new double[n * n];
            for (int j = 0; j < a.length; j++) {
                a[j] = 0.5;
                b[j] = 0.25;
            }
            double[] c = new double[n * n];
            for (int r = 0; r < n; r++) {
                for (int col = 0; col < n; col++) {
                    double s = 0.0;
                    for (int k = 0; k < n; k++) {
                        s += a[r * n + k] * b[k * n + col];
                    }
                    c[r * n + col] = s;
                }
            }
            checksum += c[0] + c[n * n - 1];
        }
        if (checksum <= 0) {
            System.err.println("unexpected checksum");
            System.exit(1);
        }
    }
}
