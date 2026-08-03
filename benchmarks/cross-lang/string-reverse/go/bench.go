// Phase 31: string-reverse (Go)

package main

import (
	"fmt"
	"os"
)

func main() {
	const iters = 200_000
	text := "The quick brown fox jumps over the lazy dog"
	total := int64(0)
	for it := 0; it < iters; it++ {
		chars := []byte(text)
		lo, hi := 0, len(chars)-1
		for lo < hi {
			chars[lo], chars[hi] = chars[hi], chars[lo]
			lo++
			hi--
		}
		checksum := int64(0)
		for i, c := range chars {
			checksum += int64(c) * int64(i+1)
		}
		total += checksum
	}
	if total != 88994*int64(iters) {
		fmt.Fprintf(os.Stderr, "unexpected: %d\n", total)
		os.Exit(1)
	}
}
