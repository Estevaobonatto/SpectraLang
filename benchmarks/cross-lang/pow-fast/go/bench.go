// Phase 31: pow-fast (Go)

package main

import (
	"fmt"
	"os"
)

func main() {
	const iters = 50_000
	bases := []int{2, 3, 5, 7, 10, 13, 2, 4, 6, 8}
	exps := []int{10, 8, 6, 5, 4, 3, 20, 15, 12, 10}
	total := int64(0)
	for it := 0; it < iters; it++ {
		acc := int64(0)
		for p := 0; p < 10; p++ {
			base, exp := int64(bases[p]), exps[p]
			result := int64(1)
			for exp > 0 {
				if exp%2 == 1 {
					result *= base
				}
				base *= base
				exp /= 2
			}
			acc += result
		}
		total += acc
	}
	if total != 4325366774*int64(iters) {
		fmt.Fprintf(os.Stderr, "unexpected: %d\n", total)
		os.Exit(1)
	}
}
