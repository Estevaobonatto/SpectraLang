// Phase 31: producer-consumer-bounded (Java)

import java.util.concurrent.*;

public class Bench {
    static long process(long n) {
        long v = n * n * n;
        return v - (v / 1000) * 1000;
    }

    public static void main(String[] args) throws Exception {
        final int iters = 200;
        long total = 0L;
        for (int it = 0; it < iters; it++) {
            BlockingQueue<Long> ch = new ArrayBlockingQueue<>(4);
            // Producer
            Thread producer = new Thread(() -> {
                for (long i = 0; i < 500; i++) {
                    try {
                        ch.put(i);
                    } catch (InterruptedException e) {
                        return;
                    }
                }
            });
            // Consumer
            long[] result = new long[1];
            Thread consumer = new Thread(() -> {
                long acc = 0L;
                for (long i = 0; i < 500; i++) {
                    try {
                        Long v = ch.take();
                        acc += process(v);
                    } catch (InterruptedException e) {
                        return;
                    }
                }
                result[0] = acc;
            });
            producer.start();
            consumer.start();
            producer.join();
            consumer.join();
            total += result[0];
        }
        if (total != 228500L * (long) iters) {
            System.err.println("unexpected: " + total);
            System.exit(1);
        }
    }
}
