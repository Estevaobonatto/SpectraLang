// Phase 31: matrix-transpose (Go)

package main

import (
	"fmt"
	"os"
)

func main() {
	const iters = 20_000
	const rows = 16
	const cols = 16
	total := int64(0)
	for it := 0; it < iters; it++ {
		m := make([]int, rows*cols)
		for r := 0; r < rows; r++ {
			for c := 0; c < cols; c++ {
				m[r*cols+c] = r*cols + c
			}
		}
		tChecksum := int64(0)
		for r := 0; r < rows; r++ {
			for c := 0; c < cols; c++ {
				val := m[r*cols+c]
				tPos := c*rows + r
				tChecksum += int64(val) * int64(tPos+1)
			}
		}
		total += tChecksum
	}
	if total != 4368320*int64(iters) {
		fmt.Fprintf(os.Stderr, "unexpected: %d\n", total)
		os.Exit(1)
	}
}
