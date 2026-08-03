// R-1603 reference (Go): ml-mlp-step-gpu
// Hand-rolled 2-layer MLP forward + MSE backward + SGD on a
// 128x128 hidden layer. Mirrors the structure of the Spectra
// bench so the two can be compared apples-to-apples for the
// R-1603 speedup gate.

package main

import (
	"fmt"
	"math"
	"os"
)

const (
	batch      = 128
	inFeatures = 128
	hidden     = 128
	iters      = 10
)

func linearForward(x, w []float64, b []float64, out []float64) {
	// x: batch x inFeatures, w: inFeatures x outFeatures, b: outFeatures
	rows := len(x) / inFeatures
	cols := len(b)
	for i := 0; i < rows; i++ {
		for j := 0; j < cols; j++ {
			s := b[j]
			for k := 0; k < inFeatures; k++ {
				s += x[i*inFeatures+k] * w[k*cols+j]
			}
			out[i*cols+j] = s
		}
	}
}

func reluInPlace(buf []float64) {
	for i := range buf {
		if buf[i] < 0 {
			buf[i] = 0
		}
	}
}

func mseBackward(pred, target []float64, grad []float64) {
	n := float64(len(pred))
	for i := range pred {
		grad[i] = 2.0 * (pred[i] - target[i]) / n
	}
}

func linearBackward(x, w, gradOut []float64, gradX, gradW, gradB []float64) {
	// x: rows x inFeatures, w: inFeatures x outFeatures, gradOut: rows x outFeatures
	rows := len(x) / inFeatures
	outCols := len(gradB)
	for j := 0; j < outCols; j++ {
		s := 0.0
		for i := 0; i < rows; i++ {
			s += gradOut[i*outCols+j]
		}
		gradB[j] = s
	}
	for i := 0; i < rows; i++ {
		for j := 0; j < outCols; j++ {
			g := gradOut[i*outCols+j]
			for k := 0; k < inFeatures; k++ {
				gradW[k*outCols+j] += g * x[i*inFeatures+k]
				gradX[i*inFeatures+k] += g * w[k*outCols+j]
			}
		}
	}
}

func sgdStep(weights, grads []float64, lr float64) {
	for i := range weights {
		weights[i] -= lr * grads[i]
	}
}

func main() {
	x := make([]float64, batch*inFeatures)
	for i := range x {
		x[i] = 0.5
	}
	y := make([]float64, batch)
	for i := range y {
		y[i] = 1.0
	}
	w1 := make([]float64, inFeatures*hidden)
	for i := range w1 {
		w1[i] = 0.1
	}
	b1 := make([]float64, hidden)
	w2 := make([]float64, hidden)
	for i := range w2 {
		w2[i] = 0.1
	}
	b2 := make([]float64, 1)

	h := make([]float64, batch*hidden)
	pred := make([]float64, batch)
	gradP := make([]float64, batch)
	gradH := make([]float64, batch*hidden)
	gradW1 := make([]float64, inFeatures*hidden)
	gradB1 := make([]float64, hidden)
	gradW2 := make([]float64, hidden)
	gradB2 := make([]float64, 1)

	for it := 0; it < iters; it++ {
		// forward layer 1
		linearForward(x, w1, b1, h)
		reluInPlace(h)
		// forward layer 2
		linearForward(h, w2, b2, pred)
		// loss + backward
		mseBackward(pred, y, gradP)
		// layer 2 backward: gradB2, gradW2, gradH
		linearBackward(h, w2, gradP, gradH, gradW2, gradB2)
		// relu backward (in place: gradH *= (h > 0))
		for i := range gradH {
			if h[i] == 0 {
				gradH[i] = 0
			}
		}
		// layer 1 backward
		linearBackward(x, w1, gradH, make([]float64, len(x)), gradW1, gradB1)
		// sgd
		sgdStep(w1, gradW1, 0.01)
		sgdStep(b1, gradB1, 0.01)
		sgdStep(w2, gradW2, 0.01)
		sgdStep(b2, gradB2, 0.01)
	}

	if math.IsNaN(pred[0]) {
		fmt.Fprintln(os.Stderr, "nan")
		os.Exit(1)
	}
	if len(pred) != batch {
		fmt.Fprintln(os.Stderr, "wrong length")
		os.Exit(1)
	}
}
