// Phase 31: word-count (Go)

package main

import (
	"fmt"
	"os"
)

func main() {
	const iters = 200_000
	text := "The quick brown fox jumps over the lazy dog and runs away"
	total := int64(0)
	for it := 0; it < iters; it++ {
		count := int64(0)
		inWord := int64(0)
		for _, c := range text {
			isSpace := int64(0)
			if c == ' ' {
				isSpace = 1
			}
			if isSpace == 0 {
				if inWord == 0 {
					count++
					inWord = 1
				}
			} else {
				inWord = 0
			}
		}
		total += count
	}
	if total != 12*int64(iters) {
		fmt.Fprintf(os.Stderr, "unexpected: %d\n", total)
		os.Exit(1)
	}
}
