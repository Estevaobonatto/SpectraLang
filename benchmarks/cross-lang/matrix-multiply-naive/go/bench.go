// Phase 31: matrix-multiply-naive (Go)

package main

import (
	"fmt"
	"os"
)

func main() {
	const iters = 20_000
	const n = 16
	total := int64(0)
	for it := 0; it < iters; it++ {
		a := make([]int64, n*n)
		b := make([]int64, n*n)
		c := make([]int64, n*n)
		for i := 0; i < n; i++ {
			for j := 0; j < n; j++ {
				v := i + j
				a[i*n+j] = int64(v - (v/100)*100)
			}
		}
		for i := 0; i < n; i++ {
			for j := 0; j < n; j++ {
				v := i*2 + j
				b[i*n+j] = int64(v - (v/100)*100)
			}
		}
		for i := 0; i < n; i++ {
			for k := 0; k < n; k++ {
				aik := a[i*n+k]
				for j := 0; j < n; j++ {
					c[i*n+j] += aik * b[k*n+j]
				}
			}
		}
		var checksum int64 = 0
		for i := 0; i < n; i++ {
			for j := 0; j < n; j++ {
				checksum += c[i*n+j] * int64(i*n+j+1)
			}
		}
		total += checksum
	}
	if total != 232647680*int64(iters) {
		fmt.Fprintf(os.Stderr, "unexpected: %d\n", total)
		os.Exit(1)
	}
}
