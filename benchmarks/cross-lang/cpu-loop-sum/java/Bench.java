// Phase 31: cpu-loop-sum (Java)
// Sum 1..N inside a tight loop. Baseline integer arithmetic benchmark.

public class Bench {
    public static void main(String[] args) {
        final int outer = 5;
        final int inner = 200_000;
        long acc = 0L;
        for (int o = 0; o < outer; o++) {
            long local = 0L;
            for (int i = 1; i <= inner; i++) {
                local += i;
            }
            acc += local;
        }
        if (acc != 100000500000L) {
            System.err.println("unexpected: " + acc);
            System.exit(1);
        }
    }
}
