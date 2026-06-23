// Phase 31: cpu-loop-sum (Go)
// Sum 1..N inside a tight loop. Baseline integer arithmetic benchmark.

package main

import (
	"fmt"
	"os"
)

func main() {
	const outer = 5
	const inner = 200_000
	acc := int64(0)
	for o := 0; o < outer; o++ {
		local := int64(0)
		for i := 1; i <= inner; i++ {
			local += int64(i)
		}
		acc += local
	}
	if acc != 100000500000 {
		fmt.Fprintf(os.Stderr, "unexpected: %d\n", acc)
		os.Exit(1)
	}
}
