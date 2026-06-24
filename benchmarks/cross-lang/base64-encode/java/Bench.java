// Phase 31: base64-encode (Java)

public class Bench {
    static final String ALPHABET = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

    static byte[] encode96() {
        byte[] out = new byte[128];
        for (int i = 0; i < 96; i += 3) {
            int b0 = i;
            int b1 = i + 1;
            int b2 = i + 2;
            int n = (b0 << 16) | (b1 << 8) | b2;
            int g = (i / 3) * 4;
            out[g] = (byte) ALPHABET.charAt((n >> 18) & 63);
            out[g + 1] = (byte) ALPHABET.charAt((n >> 12) & 63);
            if (i + 1 < 96) {
                out[g + 2] = (byte) ALPHABET.charAt((n >> 6) & 63);
            } else {
                out[g + 2] = (byte) '=';
            }
            if (i + 2 < 96) {
                out[g + 3] = (byte) ALPHABET.charAt(n & 63);
            } else {
                out[g + 3] = (byte) '=';
            }
        }
        return out;
    }

    public static void main(String[] args) {
        final int iters = 50_000;
        long total = 0L;
        for (int it = 0; it < iters; it++) {
            byte[] out = encode96();
            long checksum = 0L;
            for (int k = 0; k < 128; k++) {
                checksum += (out[k] & 0xFFL) * (k + 1);
            }
            total += checksum;
        }
        if (total != 690549L * (long) iters) {
            System.err.println("unexpected: " + total);
            System.exit(1);
        }
    }
}
