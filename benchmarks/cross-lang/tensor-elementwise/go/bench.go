// Phase 31: tensor-elementwise (Go)

package main

import (
	"fmt"
	"os"
)

func main() {
	const iters = 50
	checksum := 0.0
	for i := 0; i < iters; i++ {
		t := make([]float64, 100_000)
		for j := range t {
			t[j] = 0.5
		}
		for j := range t {
			if t[j] < 0 {
				t[j] = 0
			}
		}
		checksum += t[0] + t[99_999]
	}
	if checksum <= 0 {
		fmt.Fprintln(os.Stderr, "unexpected checksum")
		os.Exit(1)
	}
}
