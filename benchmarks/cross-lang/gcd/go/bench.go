// Phase 31: gcd (Go)

package main

import (
	"fmt"
	"os"
)

func main() {
	const iters = 1_000_000
	aVals := []int{48, 56, 1071, 1024, 270, 816, 462, 100, 75, 999}
	bVals := []int{36, 42, 462, 768, 192, 204, 330, 75, 125, 333}
	total := int64(0)
	for it := 0; it < iters; it++ {
		acc := int64(0)
		for p := 0; p < 10; p++ {
			a, b := aVals[p], bVals[p]
			for b != 0 {
				a, b = b, a%b
			}
			acc += int64(a)
		}
		total += acc
	}
	if total != 962*int64(iters) {
		fmt.Fprintf(os.Stderr, "unexpected: %d\n", total)
		os.Exit(1)
	}
}
