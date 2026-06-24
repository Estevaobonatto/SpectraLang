// Phase 31: word-count (Java)

public class Bench {
    public static void main(String[] args) {
        final int iters = 200_000;
        String text = "The quick brown fox jumps over the lazy dog and runs away";
        long total = 0L;
        for (int it = 0; it < iters; it++) {
            long count = 0L;
            long inWord = 0L;
            for (int i = 0; i < text.length(); i++) {
                long isSpace = (text.charAt(i) == ' ') ? 1L : 0L;
                if (isSpace == 0) {
                    if (inWord == 0) {
                        count++;
                        inWord = 1;
                    }
                } else {
                    inWord = 0;
                }
            }
            total += count;
        }
        if (total != 12L * (long) iters) {
            System.err.println("unexpected: " + total);
            System.exit(1);
        }
    }
}
