// Phase 31: sieve (Go)

package main

import (
	"fmt"
	"os"
)

func main() {
	const iters = 2_000
	const n = 200
	total := int64(0)
	for it := 0; it < iters; it++ {
		sieve := make([]int, n+1)
		for p := 2; p*p <= n; p++ {
			if sieve[p] == 0 {
				for multiple := p * p; multiple <= n; multiple += p {
					if sieve[multiple] == 0 {
						sieve[multiple] = 1
					}
				}
			}
		}
		count := int64(0)
		for k := 2; k <= n; k++ {
			if sieve[k] == 0 {
				count++
			}
		}
		total += count
	}
	if total != 46*int64(iters) {
		fmt.Fprintf(os.Stderr, "unexpected: %d\n", total)
		os.Exit(1)
	}
}
