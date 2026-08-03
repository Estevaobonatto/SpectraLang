// Phase 31: digit-sum (Go)

package main

import (
	"fmt"
	"os"
)

func main() {
	const iters = 200
	const n = 10_000
	total := int64(0)
	for it := 0; it < iters; it++ {
		acc := int64(0)
		for i := 1; i <= n; i++ {
			x := i
			ds := int64(0)
			for x > 0 {
				ds += int64(x % 10)
				x /= 10
			}
			acc += ds
		}
		total += acc
	}
	if total != 180001*int64(iters) {
		fmt.Fprintf(os.Stderr, "unexpected: %d\n", total)
		os.Exit(1)
	}
}
