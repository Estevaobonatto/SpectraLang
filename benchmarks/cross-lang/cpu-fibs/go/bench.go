// Phase 31: cpu-fibs (Go)

package main

import (
	"fmt"
	"os"
)

func main() {
	const iters = 200_000
	total := int64(0)
	for k := 0; k < iters; k++ {
		a, b := int64(0), int64(1)
		for i := 0; i < 40; i++ {
			c := a + b
			a = b
			b = c
		}
		total += a
	}
	if total != 20466831000000 {
		fmt.Fprintf(os.Stderr, "unexpected: %d\n", total)
		os.Exit(1)
	}
}
