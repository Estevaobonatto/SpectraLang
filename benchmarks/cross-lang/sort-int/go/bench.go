// Phase 31: sort-int (Go)
// Bubble sort on a small int slice, repeated many times.

package main

import (
	"fmt"
	"os"
)

func main() {
	const iters = 50_000
	const n = 16
	total := int64(0)
	for it := 0; it < iters; it++ {
		values := []int{9, 1, 5, 3, 7, 2, 8, 4, 0, 6, 11, 10, 15, 13, 14, 12}
		for outer := 0; outer < n; outer++ {
			for inner := 0; inner < n-1; inner++ {
				if values[inner] > values[inner+1] {
					tmp := values[inner]
					values[inner] = values[inner+1]
					values[inner+1] = tmp
				}
			}
		}
		checksum := int64(0)
		for k := 0; k < n; k++ {
			checksum += int64(values[k]) * int64(k+1)
		}
		total += checksum
	}
	if total != 1360*int64(iters) {
		fmt.Fprintf(os.Stderr, "unexpected: %d\n", total)
		os.Exit(1)
	}
}
