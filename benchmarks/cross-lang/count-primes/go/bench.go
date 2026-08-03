// Phase 31: count-primes (Go)

package main

import (
	"fmt"
	"os"
)

func main() {
	const iters = 500
	const n = 500
	total := int64(0)
	for it := 0; it < iters; it++ {
		count := int64(0)
		for i := 2; i <= n; i++ {
			isPrime := int64(1)
			for d := 2; d*d <= i; d++ {
				if i%d == 0 {
					isPrime = 0
				}
			}
			if isPrime == 1 {
				count++
			}
		}
		total += count
	}
	if total != 95*int64(iters) {
		fmt.Fprintf(os.Stderr, "unexpected: %d\n", total)
		os.Exit(1)
	}
}
