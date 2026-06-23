// Phase 31: ml-mlp-step (Java)

public class Bench {
    public static void main(String[] args) {
        final int iters = 50;
        final int n = 64;
        double[] x = new double[n];
        for (int i = 0; i < n; i++) x[i] = 1.0;
        double[] y = new double[n];
        for (int i = 0; i < n; i++) y[i] = 2.0;
        double[] w = {0.0};
        double[] b = {0.0};
        for (int it = 0; it < iters; it++) {
            double dw = 0.0, db = 0.0;
            for (int i = 0; i < n; i++) {
                double p = w[0] * x[i] + b[0];
                double diff = p - y[i];
                dw += diff * x[i];
                db += diff;
            }
            dw /= n;
            db /= n;
            w[0] -= 0.1 * dw;
            b[0] -= 0.1 * db;
        }
        if (Double.isNaN(w[0])) {
            System.err.println("unexpected");
            System.exit(1);
        }
    }
}
