// Phase 31: binary-search (Go)

package main

import (
	"fmt"
	"os"
)

func main() {
	const iters = 1_000_000
	values := []int{0, 2, 4, 6, 8, 10, 12, 14, 16, 18, 20, 22, 24, 26, 28, 30}
	const n = 16
	targets := []int{14, 3, 28, 100}
	total := int64(0)
	for it := 0; it < iters; it++ {
		acc := int64(0)
		for _, target := range targets {
			low, high := 0, n-1
			found := int64(-1)
			for low <= high {
				mid := (low + high) / 2
				if values[mid] == target {
					found = int64(mid)
					low = high + 1
				} else if values[mid] < target {
					low = mid + 1
				} else {
					high = mid - 1
				}
			}
			acc += found
		}
		total += acc
	}
	if total != 19*int64(iters) {
		fmt.Fprintf(os.Stderr, "unexpected: %d\n", total)
		os.Exit(1)
	}
}
