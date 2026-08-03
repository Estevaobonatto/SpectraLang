// Phase 31: tensor-matmul (Go)

package main

import (
	"fmt"
	"os"
)

func main() {
	const n = 64
	const iters = 20
	checksum := 0.0
	for i := 0; i < iters; i++ {
		a := make([]float64, n*n)
		b := make([]float64, n*n)
		for j := range a {
			a[j] = 0.5
			b[j] = 0.25
		}
		c := make([]float64, n*n)
		for r := 0; r < n; r++ {
			for col := 0; col < n; col++ {
				s := 0.0
				for k := 0; k < n; k++ {
					s += a[r*n+k] * b[k*n+col]
				}
				c[r*n+col] = s
			}
		}
		checksum += c[0] + c[n*n-1]
	}
	if checksum <= 0 {
		fmt.Fprintln(os.Stderr, "unexpected checksum")
		os.Exit(1)
	}
}
