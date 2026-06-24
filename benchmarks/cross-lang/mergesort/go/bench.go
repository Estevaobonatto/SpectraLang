// Phase 31: mergesort (Go)

package main

import (
	"fmt"
	"os"
)

func mergeInPlace(arr, scratch []int64, lo, mid, hi int) {
	for i := lo; i < hi; i++ {
		scratch[i] = arr[i]
	}
	l, r, k := lo, mid, lo
	for l < mid {
		if r >= hi {
			for l < mid {
				arr[k] = scratch[l]
				k++
				l++
			}
		} else {
			if scratch[l] <= scratch[r] {
				arr[k] = scratch[l]
				k++
				l++
			} else {
				arr[k] = scratch[r]
				k++
				r++
			}
		}
	}
	for r < hi {
		arr[k] = scratch[r]
		k++
		r++
	}
}

func main() {
	const iters = 30_000
	src := []int64{
		5, 16, 27, 38, 49, 60, 71, 82, 93, 7, 18, 29, 40, 51, 62, 73,
		84, 95, 9, 20, 31, 42, 53, 64, 75, 86, 0, 11, 22, 33, 44, 55,
		66, 77, 88, 2, 13, 24, 35, 46, 57, 68, 79, 90, 4, 15, 26, 37,
		48, 59, 70, 81, 92, 6, 17, 28, 39, 50, 61, 72, 83, 94, 8, 19,
	}
	total := int64(0)
	for it := 0; it < iters; it++ {
		arr := make([]int64, 64)
		copy(arr, src)
		scratch := make([]int64, 64)
		for w := 1; w < 64; w *= 2 {
			step := w * 2
			for lo := 0; lo < 64; lo += step {
				mid := lo + w
				hi := lo + step
				if mid > 64 {
					mid = 64
				}
				if hi > 64 {
					hi = 64
				}
				mergeInPlace(arr, scratch, lo, mid, hi)
			}
		}
		var checksum int64 = 0
		for k := 0; k < 64; k++ {
			checksum += arr[k] * int64(k+1)
		}
		total += checksum
	}
	if total != 130926*int64(iters) {
		fmt.Fprintf(os.Stderr, "unexpected: %d\n", total)
		os.Exit(1)
	}
}
