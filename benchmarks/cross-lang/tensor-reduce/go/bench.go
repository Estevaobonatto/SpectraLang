// Phase 31: tensor-reduce (Go)

package main

import (
	"fmt"
	"os"
)

func main() {
	const iters = 50
	total := 0.0
	for i := 0; i < iters; i++ {
		t := make([]float64, 100_000)
		for j := range t {
			t[j] = 1.0
		}
		s := 0.0
		for _, v := range t {
			s += v
		}
		total += s
	}
	if total < 4_999_999.0 || total > 5_000_001.0 {
		fmt.Fprintf(os.Stderr, "unexpected: %f\n", total)
		os.Exit(1)
	}
}
