// Phase 31: lru-cache (Go)

package main

import (
	"fmt"
	"os"
)

type LRU struct {
	cap  int
	m    map[int64]int
	keys []int64
	prev []int
	next []int
	head int
	tail int
}

func (l *LRU) addHead(k int64) int {
	nid := len(l.keys)
	l.keys = append(l.keys, k)
	l.prev = append(l.prev, -1)
	l.next = append(l.next, l.head)
	if l.head != -1 {
		l.prev[l.head] = nid
	}
	l.head = nid
	if l.tail == -1 {
		l.tail = nid
	}
	return nid
}

func (l *LRU) remove(nid int) {
	p := l.prev[nid]
	n := l.next[nid]
	if p != -1 {
		l.next[p] = n
	} else {
		l.head = n
	}
	if n != -1 {
		l.prev[n] = p
	} else {
		l.tail = p
	}
}

func (l *LRU) get(k int64) bool {
	nid, ok := l.m[k]
	if !ok {
		return false
	}
	if nid != l.head {
		l.remove(nid)
		newNid := l.addHead(k)
		l.m[k] = newNid
	}
	return true
}

func (l *LRU) put(k int64) {
	if _, ok := l.m[k]; ok {
		l.get(k)
		return
	}
	if len(l.m) >= l.cap {
		evict := l.keys[l.tail]
		delete(l.m, evict)
		l.remove(l.tail)
	}
	nid := l.addHead(k)
	l.m[k] = nid
}

func main() {
	const iters = 5_000
	const cap = 16
	const ops = 1_000
	totalHits := int64(0)
	for it := 0; it < iters; it++ {
		lru := &LRU{cap: cap, m: make(map[int64]int), head: -1, tail: -1}
		hits := 0
		for t := 0; t < ops; t++ {
			var k int64
			if t%2 == 0 {
				k = int64(t % 16)
			} else {
				k = 16 + int64((t*3)%64)
			}
			if lru.get(k) {
				hits++
			} else {
				lru.put(k)
			}
		}
		totalHits += int64(hits)
	}
	if totalHits != 492*int64(iters) {
		fmt.Fprintf(os.Stderr, "unexpected: %d\n", totalHits)
		os.Exit(1)
	}
}
