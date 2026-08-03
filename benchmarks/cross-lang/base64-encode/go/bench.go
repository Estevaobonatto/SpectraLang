// Phase 31: base64-encode (Go)

package main

import (
	"fmt"
	"os"
)

const alphabet = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/"

func encode96() [128]byte {
	var out [128]byte
	for i := 0; i < 96; i += 3 {
		b0 := byte(i)
		b1 := byte(i + 1)
		b2 := byte(i + 2)
		n := uint32(b0)<<16 | uint32(b1)<<8 | uint32(b2)
		g := (i / 3) * 4
		out[g] = alphabet[(n>>18)&63]
		out[g+1] = alphabet[(n>>12)&63]
		if i+1 < 96 {
			out[g+2] = alphabet[(n>>6)&63]
		} else {
			out[g+2] = '='
		}
		if i+2 < 96 {
			out[g+3] = alphabet[n&63]
		} else {
			out[g+3] = '='
		}
	}
	return out
}

func main() {
	const iters = 50_000
	total := int64(0)
	for it := 0; it < iters; it++ {
		out := encode96()
		var checksum int64 = 0
		for k := 0; k < 128; k++ {
			checksum += int64(out[k]) * int64(k+1)
		}
		total += checksum
	}
	if total != 690549*int64(iters) {
		fmt.Fprintf(os.Stderr, "unexpected: %d\n", total)
		os.Exit(1)
	}
}
