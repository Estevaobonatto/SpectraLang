// Phase 31: quicksort (Go)

package main

import (
	"fmt"
	"os"
)

func partition(arr []int64, lo, hi int) int {
	pivot := arr[hi]
	i := lo
	for j := lo; j < hi; j++ {
		if arr[j] < pivot {
			arr[i], arr[j] = arr[j], arr[i]
			i++
		}
	}
	arr[i], arr[hi] = arr[hi], arr[i]
	return i
}

func qs(arr []int64, lo, hi int) {
	if lo >= hi {
		return
	}
	p := partition(arr, lo, hi)
	qs(arr, lo, p-1)
	qs(arr, p+1, hi)
}

func main() {
	const iters = 50_000
	// Same input as the spectra bench: [(i*7+3) % 100 for i in 0..64]
	src := []int64{
		3, 10, 17, 24, 31, 38, 45, 52, 59, 66, 73, 80, 87, 94, 1, 8,
		15, 22, 29, 36, 43, 50, 57, 64, 71, 78, 85, 92, 99, 6, 13, 20,
		27, 34, 41, 48, 55, 62, 69, 76, 83, 90, 97, 4, 11, 18, 25, 32,
		39, 46, 53, 60, 67, 74, 81, 88, 95, 2, 9, 16, 23, 30, 37, 44,
	}
	total := int64(0)
	for it := 0; it < iters; it++ {
		arr := make([]int64, 64)
		copy(arr, src)
		qs(arr, 0, 63)
		var checksum int64 = 0
		for k := 0; k < 64; k++ {
			checksum += arr[k] * int64(k+1)
		}
		total += checksum
	}
	if total != 131629*int64(iters) {
		fmt.Fprintf(os.Stderr, "unexpected: %d\n", total)
		os.Exit(1)
	}
}
