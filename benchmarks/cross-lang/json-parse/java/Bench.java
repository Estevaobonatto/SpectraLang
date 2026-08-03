// Phase 31: json-parse (Java)

public class Bench {
    public static void main(String[] args) {
        final int iters = 100_000;
        String doc = "{\"a\":1,\"b\":[2,3,4],\"c\":true,\"d\":\"hi\",\"e\":-7,\"f\":[],\"g\":{}}";
        long total = 0L;
        for (int it = 0; it < iters; it++) {
            int i = 0;
            long tokens = 0L;
            long intsum = 0L;
            while (i < doc.length()) {
                char c = doc.charAt(i);
                if (c == '{' || c == '}' || c == '[' || c == ']' || c == ',' || c == ':') {
                    tokens++;
                    i++;
                } else if (c == '"') {
                    tokens++;
                    i++;
                    while (i < doc.length() && doc.charAt(i) != '"') i++;
                    if (i < doc.length()) i++;
                } else if (c == '-') {
                    tokens++;
                    i++;
                    long neg = 0L;
                    while (i < doc.length() && doc.charAt(i) >= '0' && doc.charAt(i) <= '9') {
                        neg = neg * 10 + (doc.charAt(i) - '0');
                        i++;
                    }
                    intsum += -neg;
                } else if (c >= '0' && c <= '9') {
                    tokens++;
                    long pos = 0L;
                    while (i < doc.length() && doc.charAt(i) >= '0' && doc.charAt(i) <= '9') {
                        pos = pos * 10 + (doc.charAt(i) - '0');
                        i++;
                    }
                    intsum += pos;
                } else if (c == 't') {
                    tokens++;
                    i += 4;
                } else if (c == 'f') {
                    tokens++;
                    i += 5;
                } else if (c == 'n') {
                    tokens++;
                    i += 4;
                } else {
                    i++;
                }
            }
            total += tokens * 1000 + intsum;
        }
        if (total != 37003L * (long) iters) {
            System.err.println("unexpected: " + total);
            System.exit(1);
        }
    }
}
