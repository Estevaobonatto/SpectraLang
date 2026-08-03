// Phase 31: producer-consumer-bounded (Go)

package main

import (
	"fmt"
	"os"
)

func process(n int64) int64 {
	v := n * n * n
	return v - (v/1000)*1000
}

func main() {
	const iters = 200
	total := int64(0)
	for it := 0; it < iters; it++ {
		ch := make(chan int64, 4) // bounded capacity 4
		// Producer
		go func() {
			for i := int64(0); i < 500; i++ {
				ch <- i
			}
			close(ch)
		}()
		// Consumer
		var acc int64 = 0
		for v := range ch {
			acc += process(v)
		}
		total += acc
	}
	if total != 228500*int64(iters) {
		fmt.Fprintf(os.Stderr, "unexpected: %d\n", total)
		os.Exit(1)
	}
}
