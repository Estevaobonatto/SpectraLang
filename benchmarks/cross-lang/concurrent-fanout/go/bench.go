// Phase 31: concurrent-fanout (Go)

package main

import (
	"fmt"
	"os"
	"sync"
)

func sumSq(lo, hi int64) int64 {
	var s int64 = 0
	for i := lo; i < hi; i++ {
		s += i * i
	}
	return s
}

func main() {
	const iters = 1_000
	total := int64(0)
	for it := 0; it < iters; it++ {
		var wg sync.WaitGroup
		results := make([]int64, 8)
		for k := 0; k < 8; k++ {
			wg.Add(1)
			go func(idx int) {
				defer wg.Done()
				results[idx] = sumSq(int64(idx)*1000, int64(idx+1)*1000)
			}(k)
		}
		wg.Wait()
		var acc int64 = 0
		for _, r := range results {
			acc += r
		}
		total += acc
	}
	if total != 170634668000*int64(iters) {
		fmt.Fprintf(os.Stderr, "unexpected: %d\n", total)
		os.Exit(1)
	}
}
