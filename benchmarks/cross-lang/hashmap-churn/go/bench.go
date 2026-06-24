// Phase 31: hashmap-churn (Go)

package main

import (
	"fmt"
	"os"
)

func main() {
	const iters = 2_000
	total := int64(0)
	for it := 0; it < iters; it++ {
		m := make(map[int64]int64)
		for i := 0; i < 500; i++ {
			m[int64(i)] = int64(i) * 2
		}
		for k := 0; k < 500; k++ {
			if k%2 == 1 {
				delete(m, int64(k))
			}
		}
		for j := 0; j < 250; j++ {
			m[int64(500+j)] = int64(500+j) * 2
		}
		var acc int64 = 0
		for x := 0; x < 750; x++ {
			if _, ok := m[int64(x)]; ok {
				acc += int64(x)
			}
		}
		total += acc
	}
	if total != 218375*int64(iters) {
		fmt.Fprintf(os.Stderr, "unexpected: %d\n", total)
		os.Exit(1)
	}
}
