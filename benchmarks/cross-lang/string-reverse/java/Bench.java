// Phase 31: string-reverse (Java)

public class Bench {
    public static void main(String[] args) {
        final int iters = 200_000;
        String text = "The quick brown fox jumps over the lazy dog";
        long total = 0L;
        for (int it = 0; it < iters; it++) {
            char[] chars = text.toCharArray();
            int lo = 0, hi = chars.length - 1;
            while (lo < hi) {
                char tmp = chars[lo];
                chars[lo] = chars[hi];
                chars[hi] = tmp;
                lo++;
                hi--;
            }
            long checksum = 0L;
            for (int i = 0; i < chars.length; i++) {
                checksum += (long) chars[i] * (long) (i + 1);
            }
            total += checksum;
        }
        if (total != 88_994L * (long) iters) {
            System.err.println("unexpected: " + total);
            System.exit(1);
        }
    }
}
