// Phase 31: 3n-plus-1 (Go)

package main

import (
	"fmt"
	"os"
)

func main() {
	const iters = 1_000
	total := int64(0)
	for it := 0; it < iters; it++ {
		var acc int64 = 0
		for n := int64(1); n <= 1000; n++ {
			x := n
			var steps int64 = 0
			for x != 1 {
				if x%2 == 0 {
					x /= 2
				} else {
					x = 3*x + 1
				}
				steps++
			}
			acc += steps
		}
		total += acc
	}
	if total != 59542*int64(iters) {
		fmt.Fprintf(os.Stderr, "unexpected: %d\n", total)
		os.Exit(1)
	}
}
