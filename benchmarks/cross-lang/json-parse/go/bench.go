// Phase 31: json-parse (Go)

package main

import (
	"fmt"
	"os"
)

func main() {
	const iters = 100_000
	doc := `{"a":1,"b":[2,3,4],"c":true,"d":"hi","e":-7,"f":[],"g":{}}`
	total := int64(0)
	for it := 0; it < iters; it++ {
		i := 0
		tokens := int64(0)
		intsum := int64(0)
		for i < len(doc) {
			c := doc[i]
			switch c {
			case '{', '}', '[', ']', ',', ':':
				tokens++
				i++
			case '"':
				tokens++
				i++
				for i < len(doc) && doc[i] != '"' {
					i++
				}
				if i < len(doc) {
					i++ // skip closing "
				}
			case '-':
				tokens++
				i++
				neg := int64(0)
				for i < len(doc) && doc[i] >= '0' && doc[i] <= '9' {
					neg = neg*10 + int64(doc[i]-'0')
					i++
				}
				intsum += -neg
			case '0', '1', '2', '3', '4', '5', '6', '7', '8', '9':
				tokens++
				pos := int64(0)
				for i < len(doc) && doc[i] >= '0' && doc[i] <= '9' {
					pos = pos*10 + int64(doc[i]-'0')
					i++
				}
				intsum += pos
			case 't':
				tokens++
				i += 4
			case 'f':
				tokens++
				i += 5
			case 'n':
				tokens++
				i += 4
			default:
				i++
			}
		}
		total += tokens*1000 + intsum
	}
	if total != 37003*int64(iters) {
		fmt.Fprintf(os.Stderr, "unexpected: %d\n", total)
		os.Exit(1)
	}
}
