// Phase 31: async-pipeline (Java)

import java.util.concurrent.ArrayBlockingQueue;
import java.util.concurrent.BlockingQueue;

public class Bench {
    public static void main(String[] args) throws Exception {
        final int iters = 5;
        final int n = 200;
        long total = 0L;
        for (int it = 0; it < iters; it++) {
            BlockingQueue<Integer> ch = new ArrayBlockingQueue<>(16);
            Thread producer = new Thread(() -> {
                try {
                    for (int i = 0; i < n; i++) ch.put(i);
                    ch.put(-1);
                } catch (InterruptedException e) {
                    Thread.currentThread().interrupt();
                }
            });
            int[] sum = {0};
            Thread consumer = new Thread(() -> {
                try {
                    int v;
                    while ((v = ch.take()) != -1) {
                        sum[0] += v;
                    }
                } catch (InterruptedException e) {
                    Thread.currentThread().interrupt();
                }
            });
            producer.start();
            consumer.start();
            producer.join();
            consumer.join();
            total += sum[0];
        }
        // each iter: 0..n sum = n*(n-1)/2 = 200*199/2 = 19900
        if (total < 19900L * iters || total > 19900L * iters + iters) {
            System.err.println("unexpected: " + total);
            System.exit(1);
        }
    }
}
