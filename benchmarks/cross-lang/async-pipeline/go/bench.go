// Phase 31: async-pipeline (Go)

package main

import (
	"fmt"
	"os"
)

func main() {
	const iters = 5
	const n = 200
	total := 0
	for it := 0; it < iters; it++ {
		ch := make(chan int, 16)
		done := make(chan int, 1)
		go func() {
			for i := 0; i < n; i++ {
				ch <- i
			}
			close(ch)
		}()
		go func() {
			s := 0
			for v := range ch {
				s += v
			}
			done <- s
		}()
		total += <-done
	}
	// each iter: 0..n sum = n*(n-1)/2 = 19900
	if total < 19900*iters || total > 19900*iters+iters {
		fmt.Fprintf(os.Stderr, "unexpected: %d\n", total)
		os.Exit(1)
	}
}
