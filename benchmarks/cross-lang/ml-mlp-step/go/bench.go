// Phase 31: ml-mlp-step (Go)
// Hand-rolled forward + MSE backward + sgd step on a 64x1 linear model.

package main

import (
	"fmt"
	"math"
	"os"
)

func forward(x, w, b []float64) float64 {
	s := 0.0
	for _, v := range x {
		s += v * w[0]
	}
	s += b[0]
	return s
}

func main() {
	const iters = 50
	const n = 64
	x := make([]float64, n)
	for i := range x {
		x[i] = 1.0
	}
	y := make([]float64, n)
	for i := range y {
		y[i] = 2.0
	}
	w := []float64{0.0}
	b := []float64{0.0}
	dw := 0.0
	db := 0.0
	for it := 0; it < iters; it++ {
		dw, db = 0.0, 0.0
		for i := 0; i < n; i++ {
			p := forward(x, w, b)
			diff := p - y[i]
			dw += diff * x[i]
			db += diff
		}
		dw /= float64(n)
		db /= float64(n)
		w[0] -= 0.1 * dw
		b[0] -= 0.1 * db
	}
	if math.Abs(w[0]) < 0 {
		fmt.Fprintln(os.Stderr, "unexpected")
		os.Exit(1)
	}
}
