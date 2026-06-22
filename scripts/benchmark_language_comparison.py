#!/usr/bin/env python3
"""Compare Spectra algorithm runtime against Python, Rust, Go, and Java.

The report is intentionally explicit about measurement kind:
- Spectra is measured as CLI JIT end-to-end (`spectralang run`).
- Rust/Go/Java are compiled once, then measured as executable/runtime startup.
- Python is measured as interpreter startup plus script execution.
"""

from __future__ import annotations

import argparse
import json
import os
import shutil
import statistics
import subprocess
import sys
import time
from dataclasses import dataclass
from pathlib import Path


SCHEMA = "spectralang.language_comparison_benchmark.v1"


@dataclass(frozen=True)
class CommandSpec:
    language: str
    workload: str
    measurement_kind: str
    command: list[str]
    cwd: Path
    compile_command: list[str] | None = None


def write(path: Path, text: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text, encoding="utf-8")


def spectra_counter() -> str:
    return """module bench_counter;

import { println } from std.io;

pub fn main() -> int {
    let n = 5000000;
    let i = 0;
    let sum = 0;
    while i < n {
        sum = sum + (i % 97);
        i = i + 1;
    }
    println(f"{sum}");
    return 0;
}
"""


def spectra_dp() -> str:
    return """module bench_dp;

import { println } from std.io;

fn run_once() -> int {
    let coins = [1, 2, 5];
    let dp = [0, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99];
    let amount = 1;
    while amount <= 11 {
        let index = 0;
        while index < 3 {
            let coin = coins[index];
            if coin <= amount {
                let candidate = dp[amount - coin] + 1;
                if candidate < dp[amount] {
                    dp[amount] = candidate;
                }
            }
            index = index + 1;
        }
        amount = amount + 1;
    }
    return dp[11];
}

pub fn main() -> int {
    let rounds = 200000;
    let index = 0;
    let acc = 0;
    while index < rounds {
        acc = acc + run_once();
        index = index + 1;
    }
    println(f"{acc}");
    return 0;
}
"""


def spectra_grid() -> str:
    return """module bench_grid;

import { println } from std.io;

fn run_once() -> int {
    let grid = [
        1, 1, 0, 0, 0,
        1, 0, 0, 1, 1,
        0, 0, 1, 0, 0,
        1, 1, 0, 0, 1,
        0, 1, 0, 1, 1
    ];
    let count = 0;
    let index = 0;
    while index < 25 {
        if grid[index] == 1 {
            count = count + 1;
        }
        index = index + 1;
    }
    return count;
}

pub fn main() -> int {
    let rounds = 300000;
    let index = 0;
    let acc = 0;
    while index < rounds {
        acc = acc + run_once();
        index = index + 1;
    }
    println(f"{acc}");
    return 0;
}
"""


def spectra_graph() -> str:
    return """module bench_graph;

import { println } from std.io;

fn edge(weights: [int], row: int, col: int) -> int {
    return weights[row * 6 + col];
}

fn run_once() -> int {
    let weights = [
        0, 7, 9, 0, 0, 14,
        7, 0, 10, 15, 0, 0,
        9, 10, 0, 11, 0, 2,
        0, 15, 11, 0, 6, 0,
        0, 0, 0, 6, 0, 9,
        14, 0, 2, 0, 9, 0
    ];
    let dist = [0, 999, 999, 999, 999, 999];
    let visited = [0, 0, 0, 0, 0, 0];
    let step = 0;

    while step < 6 {
        let best_node = -1;
        let best_dist = 1000;
        let i = 0;
        while i < 6 {
            if visited[i] == 0 && dist[i] < best_dist {
                best_dist = dist[i];
                best_node = i;
            }
            i = i + 1;
        }
        if best_node == -1 {
            break;
        }
        visited[best_node] = 1;

        let neighbor = 0;
        while neighbor < 6 {
            let w = edge(weights, best_node, neighbor);
            if w > 0 && visited[neighbor] == 0 {
                let candidate = dist[best_node] + w;
                if candidate < dist[neighbor] {
                    dist[neighbor] = candidate;
                }
            }
            neighbor = neighbor + 1;
        }
        step = step + 1;
    }

    return dist[0] + dist[1] * 10 + dist[2] * 100 + dist[3] * 1000 + dist[4] * 10000 + dist[5] * 100000;
}

pub fn main() -> int {
    let rounds = 100000;
    let index = 0;
    let acc = 0;
    while index < rounds {
        acc = acc + run_once();
        index = index + 1;
    }
    println(f"{acc}");
    return 0;
}
"""


def spectra_string() -> str:
    return """module bench_string;

import { println } from std.io;
import std.string as str;

fn matching(open: int, close: int) -> bool {
    if open == 40 && close == 41 {
        return true;
    }
    if open == 91 && close == 93 {
        return true;
    }
    if open == 123 && close == 125 {
        return true;
    }
    return false;
}

fn valid_parentheses(text: string) -> bool {
    let stack = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    let top = 0;
    let index = 0;
    let n = str.len(text);
    while index < n {
        let code = str.char_at(text, index);
        if code == 40 || code == 91 || code == 123 {
            stack[top] = code;
            top = top + 1;
        } else {
            if top == 0 {
                return false;
            }
            top = top - 1;
            if !matching(stack[top], code) {
                return false;
            }
        }
        index = index + 1;
    }
    return top == 0;
}

fn parse_positive_int(text: string) -> int {
    let value = 0;
    let index = 0;
    let n = str.len(text);
    while index < n {
        let code = str.char_at(text, index);
        if code < 48 || code > 57 {
            return -1;
        }
        value = value * 10 + (code - 48);
        index = index + 1;
    }
    return value;
}

pub fn main() -> int {
    let rounds = 100000;
    let index = 0;
    let acc = 0;
    while index < rounds {
        if valid_parentheses("{[()()]}") {
            acc = acc + 1;
        }
        if !valid_parentheses("{[(])}") {
            acc = acc + 10;
        }
        acc = acc + (parse_positive_int("314159") % 97);
        index = index + 1;
    }
    println(f"{acc}");
    return 0;
}
"""


PY_TEMPLATE = r"""
import sys

def bench_counter():
    n = 5_000_000
    total = 0
    i = 0
    while i < n:
        total += i % 97
        i += 1
    return total

def dp_once():
    coins = [1, 2, 5]
    dp = [0] + [99] * 11
    amount = 1
    while amount <= 11:
        index = 0
        while index < 3:
            coin = coins[index]
            if coin <= amount:
                candidate = dp[amount - coin] + 1
                if candidate < dp[amount]:
                    dp[amount] = candidate
            index += 1
        amount += 1
    return dp[11]

def bench_dp():
    acc = 0
    index = 0
    while index < 200_000:
        acc += dp_once()
        index += 1
    return acc

def grid_once():
    grid = [
        1, 1, 0, 0, 0,
        1, 0, 0, 1, 1,
        0, 0, 1, 0, 0,
        1, 1, 0, 0, 1,
        0, 1, 0, 1, 1,
    ]
    count = 0
    index = 0
    while index < 25:
        if grid[index] == 1:
            count += 1
        index += 1
    return count

def bench_grid():
    acc = 0
    index = 0
    while index < 300_000:
        acc += grid_once()
        index += 1
    return acc

def edge(weights, row, col):
    return weights[row * 6 + col]

def graph_once():
    weights = [
        0, 7, 9, 0, 0, 14,
        7, 0, 10, 15, 0, 0,
        9, 10, 0, 11, 0, 2,
        0, 15, 11, 0, 6, 0,
        0, 0, 0, 6, 0, 9,
        14, 0, 2, 0, 9, 0,
    ]
    dist = [0, 999, 999, 999, 999, 999]
    visited = [0, 0, 0, 0, 0, 0]
    step = 0
    while step < 6:
        best_node = -1
        best_dist = 1000
        i = 0
        while i < 6:
            if visited[i] == 0 and dist[i] < best_dist:
                best_dist = dist[i]
                best_node = i
            i += 1
        if best_node == -1:
            break
        visited[best_node] = 1
        neighbor = 0
        while neighbor < 6:
            w = edge(weights, best_node, neighbor)
            if w > 0 and visited[neighbor] == 0:
                candidate = dist[best_node] + w
                if candidate < dist[neighbor]:
                    dist[neighbor] = candidate
            neighbor += 1
        step += 1
    return dist[0] + dist[1] * 10 + dist[2] * 100 + dist[3] * 1000 + dist[4] * 10000 + dist[5] * 100000

def bench_graph():
    acc = 0
    index = 0
    while index < 100_000:
        acc += graph_once()
        index += 1
    return acc

def valid_parentheses(text):
    stack = []
    pairs = {")": "(", "]": "[", "}": "{"}
    index = 0
    while index < len(text):
        ch = text[index]
        if ch in "([{":
            stack.append(ch)
        else:
            if not stack or stack.pop() != pairs[ch]:
                return False
        index += 1
    return len(stack) == 0

def parse_positive_int(text):
    value = 0
    index = 0
    while index < len(text):
        code = ord(text[index])
        if code < 48 or code > 57:
            return -1
        value = value * 10 + (code - 48)
        index += 1
    return value

def bench_string():
    acc = 0
    index = 0
    while index < 100_000:
        if valid_parentheses("{[()()]}"):
            acc += 1
        if not valid_parentheses("{[(])}"):
            acc += 10
        acc += parse_positive_int("314159") % 97
        index += 1
    return acc

workload = sys.argv[1]
if workload == "counter":
    print(bench_counter())
elif workload == "dp":
    print(bench_dp())
elif workload == "grid":
    print(bench_grid())
elif workload == "graph":
    print(bench_graph())
elif workload == "string":
    print(bench_string())
else:
    raise SystemExit(2)
"""


RUST_TEMPLATE = r"""
use std::env;

fn bench_counter() -> i64 {
    let n = 5_000_000i64;
    let mut total = 0i64;
    let mut i = 0i64;
    while i < n {
        total += i % 97;
        i += 1;
    }
    total
}

fn dp_once() -> i64 {
    let coins = [1i64, 2, 5];
    let mut dp = [99i64; 12];
    dp[0] = 0;
    let mut amount = 1usize;
    while amount <= 11 {
        let mut index = 0usize;
        while index < 3 {
            let coin = coins[index] as usize;
            if coin <= amount {
                let candidate = dp[amount - coin] + 1;
                if candidate < dp[amount] {
                    dp[amount] = candidate;
                }
            }
            index += 1;
        }
        amount += 1;
    }
    dp[11]
}

fn bench_dp() -> i64 {
    let mut acc = 0i64;
    let mut index = 0;
    while index < 200_000 {
        acc += dp_once();
        index += 1;
    }
    acc
}

fn grid_once() -> i64 {
    let grid = [
        1, 1, 0, 0, 0,
        1, 0, 0, 1, 1,
        0, 0, 1, 0, 0,
        1, 1, 0, 0, 1,
        0, 1, 0, 1, 1,
    ];
    let mut count = 0i64;
    let mut index = 0usize;
    while index < 25 {
        if grid[index] == 1 {
            count += 1;
        }
        index += 1;
    }
    count
}

fn bench_grid() -> i64 {
    let mut acc = 0i64;
    let mut index = 0;
    while index < 300_000 {
        acc += grid_once();
        index += 1;
    }
    acc
}

fn edge(weights: &[i64; 36], row: usize, col: usize) -> i64 {
    weights[row * 6 + col]
}

fn graph_once() -> i64 {
    let weights = [
        0, 7, 9, 0, 0, 14,
        7, 0, 10, 15, 0, 0,
        9, 10, 0, 11, 0, 2,
        0, 15, 11, 0, 6, 0,
        0, 0, 0, 6, 0, 9,
        14, 0, 2, 0, 9, 0,
    ];
    let mut dist = [0, 999, 999, 999, 999, 999];
    let mut visited = [0, 0, 0, 0, 0, 0];
    let mut step = 0;
    while step < 6 {
        let mut best_node = -1i64;
        let mut best_dist = 1000;
        let mut i = 0usize;
        while i < 6 {
            if visited[i] == 0 && dist[i] < best_dist {
                best_dist = dist[i];
                best_node = i as i64;
            }
            i += 1;
        }
        if best_node == -1 {
            break;
        }
        let node = best_node as usize;
        visited[node] = 1;
        let mut neighbor = 0usize;
        while neighbor < 6 {
            let w = edge(&weights, node, neighbor);
            if w > 0 && visited[neighbor] == 0 {
                let candidate = dist[node] + w;
                if candidate < dist[neighbor] {
                    dist[neighbor] = candidate;
                }
            }
            neighbor += 1;
        }
        step += 1;
    }
    dist[0] + dist[1] * 10 + dist[2] * 100 + dist[3] * 1000 + dist[4] * 10000 + dist[5] * 100000
}

fn bench_graph() -> i64 {
    let mut acc = 0;
    let mut index = 0;
    while index < 100_000 {
        acc += graph_once();
        index += 1;
    }
    acc
}

fn valid_parentheses(text: &str) -> bool {
    let bytes = text.as_bytes();
    let mut stack = [0u8; 12];
    let mut top = 0usize;
    let mut index = 0usize;
    while index < bytes.len() {
        let code = bytes[index];
        if code == b'(' || code == b'[' || code == b'{' {
            stack[top] = code;
            top += 1;
        } else {
            if top == 0 {
                return false;
            }
            top -= 1;
            let open = stack[top];
            if !((open == b'(' && code == b')')
                || (open == b'[' && code == b']')
                || (open == b'{' && code == b'}'))
            {
                return false;
            }
        }
        index += 1;
    }
    top == 0
}

fn parse_positive_int(text: &str) -> i64 {
    let bytes = text.as_bytes();
    let mut value = 0i64;
    let mut index = 0usize;
    while index < bytes.len() {
        let code = bytes[index];
        if !(b'0'..=b'9').contains(&code) {
            return -1;
        }
        value = value * 10 + (code - b'0') as i64;
        index += 1;
    }
    value
}

fn bench_string() -> i64 {
    let mut acc = 0;
    let mut index = 0;
    while index < 100_000 {
        if valid_parentheses("{[()()]}") {
            acc += 1;
        }
        if !valid_parentheses("{[(])}") {
            acc += 10;
        }
        acc += parse_positive_int("314159") % 97;
        index += 1;
    }
    acc
}

fn main() {
    let workload = env::args().nth(1).unwrap_or_default();
    let result = match workload.as_str() {
        "counter" => bench_counter(),
        "dp" => bench_dp(),
        "grid" => bench_grid(),
        "graph" => bench_graph(),
        "string" => bench_string(),
        _ => std::process::exit(2),
    };
    println!("{}", result);
}
"""


GO_TEMPLATE = r"""
package main

import (
    "fmt"
    "os"
)

func benchCounter() int64 {
    var total int64 = 0
    var i int64 = 0
    for i < 5000000 {
        total += i % 97
        i++
    }
    return total
}

func dpOnce() int64 {
    coins := [3]int{1, 2, 5}
    dp := [12]int64{0, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99}
    amount := 1
    for amount <= 11 {
        index := 0
        for index < 3 {
            coin := coins[index]
            if coin <= amount {
                candidate := dp[amount - coin] + 1
                if candidate < dp[amount] {
                    dp[amount] = candidate
                }
            }
            index++
        }
        amount++
    }
    return dp[11]
}

func benchDP() int64 {
    var acc int64 = 0
    index := 0
    for index < 200000 {
        acc += dpOnce()
        index++
    }
    return acc
}

func gridOnce() int64 {
    grid := [25]int{
        1, 1, 0, 0, 0,
        1, 0, 0, 1, 1,
        0, 0, 1, 0, 0,
        1, 1, 0, 0, 1,
        0, 1, 0, 1, 1,
    }
    var count int64 = 0
    index := 0
    for index < 25 {
        if grid[index] == 1 {
            count++
        }
        index++
    }
    return count
}

func benchGrid() int64 {
    var acc int64 = 0
    index := 0
    for index < 300000 {
        acc += gridOnce()
        index++
    }
    return acc
}

func edge(weights *[36]int64, row int, col int) int64 {
    return weights[row * 6 + col]
}

func graphOnce() int64 {
    weights := [36]int64{
        0, 7, 9, 0, 0, 14,
        7, 0, 10, 15, 0, 0,
        9, 10, 0, 11, 0, 2,
        0, 15, 11, 0, 6, 0,
        0, 0, 0, 6, 0, 9,
        14, 0, 2, 0, 9, 0,
    }
    dist := [6]int64{0, 999, 999, 999, 999, 999}
    visited := [6]int{0, 0, 0, 0, 0, 0}
    step := 0
    for step < 6 {
        bestNode := -1
        var bestDist int64 = 1000
        i := 0
        for i < 6 {
            if visited[i] == 0 && dist[i] < bestDist {
                bestDist = dist[i]
                bestNode = i
            }
            i++
        }
        if bestNode == -1 {
            break
        }
        visited[bestNode] = 1
        neighbor := 0
        for neighbor < 6 {
            w := edge(&weights, bestNode, neighbor)
            if w > 0 && visited[neighbor] == 0 {
                candidate := dist[bestNode] + w
                if candidate < dist[neighbor] {
                    dist[neighbor] = candidate
                }
            }
            neighbor++
        }
        step++
    }
    return dist[0] + dist[1] * 10 + dist[2] * 100 + dist[3] * 1000 + dist[4] * 10000 + dist[5] * 100000
}

func benchGraph() int64 {
    var acc int64 = 0
    index := 0
    for index < 100000 {
        acc += graphOnce()
        index++
    }
    return acc
}

func validParentheses(text string) bool {
    stack := [12]byte{}
    top := 0
    index := 0
    for index < len(text) {
        code := text[index]
        if code == '(' || code == '[' || code == '{' {
            stack[top] = code
            top++
        } else {
            if top == 0 {
                return false
            }
            top--
            open := stack[top]
            if !((open == '(' && code == ')') || (open == '[' && code == ']') || (open == '{' && code == '}')) {
                return false
            }
        }
        index++
    }
    return top == 0
}

func parsePositiveInt(text string) int64 {
    var value int64 = 0
    index := 0
    for index < len(text) {
        code := text[index]
        if code < '0' || code > '9' {
            return -1
        }
        value = value * 10 + int64(code - '0')
        index++
    }
    return value
}

func benchString() int64 {
    var acc int64 = 0
    index := 0
    for index < 100000 {
        if validParentheses("{[()()]}") {
            acc += 1
        }
        if !validParentheses("{[(])}") {
            acc += 10
        }
        acc += parsePositiveInt("314159") % 97
        index++
    }
    return acc
}

func main() {
    if len(os.Args) < 2 {
        os.Exit(2)
    }
    switch os.Args[1] {
    case "counter":
        fmt.Println(benchCounter())
    case "dp":
        fmt.Println(benchDP())
    case "grid":
        fmt.Println(benchGrid())
    case "graph":
        fmt.Println(benchGraph())
    case "string":
        fmt.Println(benchString())
    default:
        os.Exit(2)
    }
}
"""


JAVA_TEMPLATE = r"""
public class Bench {
    static long benchCounter() {
        long total = 0;
        long i = 0;
        while (i < 5_000_000L) {
            total += i % 97;
            i++;
        }
        return total;
    }

    static long dpOnce() {
        int[] coins = {1, 2, 5};
        long[] dp = {0, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99};
        int amount = 1;
        while (amount <= 11) {
            int index = 0;
            while (index < 3) {
                int coin = coins[index];
                if (coin <= amount) {
                    long candidate = dp[amount - coin] + 1;
                    if (candidate < dp[amount]) {
                        dp[amount] = candidate;
                    }
                }
                index++;
            }
            amount++;
        }
        return dp[11];
    }

    static long benchDP() {
        long acc = 0;
        int index = 0;
        while (index < 200_000) {
            acc += dpOnce();
            index++;
        }
        return acc;
    }

    static long gridOnce() {
        int[] grid = {
            1, 1, 0, 0, 0,
            1, 0, 0, 1, 1,
            0, 0, 1, 0, 0,
            1, 1, 0, 0, 1,
            0, 1, 0, 1, 1
        };
        long count = 0;
        int index = 0;
        while (index < 25) {
            if (grid[index] == 1) {
                count++;
            }
            index++;
        }
        return count;
    }

    static long benchGrid() {
        long acc = 0;
        int index = 0;
        while (index < 300_000) {
            acc += gridOnce();
            index++;
        }
        return acc;
    }

    static long edge(long[] weights, int row, int col) {
        return weights[row * 6 + col];
    }

    static long graphOnce() {
        long[] weights = {
            0, 7, 9, 0, 0, 14,
            7, 0, 10, 15, 0, 0,
            9, 10, 0, 11, 0, 2,
            0, 15, 11, 0, 6, 0,
            0, 0, 0, 6, 0, 9,
            14, 0, 2, 0, 9, 0
        };
        long[] dist = {0, 999, 999, 999, 999, 999};
        int[] visited = {0, 0, 0, 0, 0, 0};
        int step = 0;
        while (step < 6) {
            int bestNode = -1;
            long bestDist = 1000;
            int i = 0;
            while (i < 6) {
                if (visited[i] == 0 && dist[i] < bestDist) {
                    bestDist = dist[i];
                    bestNode = i;
                }
                i++;
            }
            if (bestNode == -1) {
                break;
            }
            visited[bestNode] = 1;
            int neighbor = 0;
            while (neighbor < 6) {
                long w = edge(weights, bestNode, neighbor);
                if (w > 0 && visited[neighbor] == 0) {
                    long candidate = dist[bestNode] + w;
                    if (candidate < dist[neighbor]) {
                        dist[neighbor] = candidate;
                    }
                }
                neighbor++;
            }
            step++;
        }
        return dist[0] + dist[1] * 10 + dist[2] * 100 + dist[3] * 1000 + dist[4] * 10000 + dist[5] * 100000;
    }

    static long benchGraph() {
        long acc = 0;
        int index = 0;
        while (index < 100_000) {
            acc += graphOnce();
            index++;
        }
        return acc;
    }

    static boolean validParentheses(String text) {
        char[] stack = new char[12];
        int top = 0;
        int index = 0;
        while (index < text.length()) {
            char code = text.charAt(index);
            if (code == '(' || code == '[' || code == '{') {
                stack[top] = code;
                top++;
            } else {
                if (top == 0) {
                    return false;
                }
                top--;
                char open = stack[top];
                if (!((open == '(' && code == ')') || (open == '[' && code == ']') || (open == '{' && code == '}'))) {
                    return false;
                }
            }
            index++;
        }
        return top == 0;
    }

    static long parsePositiveInt(String text) {
        long value = 0;
        int index = 0;
        while (index < text.length()) {
            char code = text.charAt(index);
            if (code < '0' || code > '9') {
                return -1;
            }
            value = value * 10 + (code - '0');
            index++;
        }
        return value;
    }

    static long benchString() {
        long acc = 0;
        int index = 0;
        while (index < 100_000) {
            if (validParentheses("{[()()]}")) {
                acc += 1;
            }
            if (!validParentheses("{[(])}")) {
                acc += 10;
            }
            acc += parsePositiveInt("314159") % 97;
            index++;
        }
        return acc;
    }

    public static void main(String[] args) {
        if (args.length < 1) {
            System.exit(2);
        }
        long result;
        switch (args[0]) {
            case "counter":
                result = benchCounter();
                break;
            case "dp":
                result = benchDP();
                break;
            case "grid":
                result = benchGrid();
                break;
            case "graph":
                result = benchGraph();
                break;
            case "string":
                result = benchString();
                break;
            default:
                System.exit(2);
                return;
        }
        System.out.println(result);
    }
}
"""


def percentile(values: list[float], pct: float) -> float:
    if not values:
        return 0.0
    ordered = sorted(values)
    idx = int(round((len(ordered) - 1) * pct))
    return ordered[idx]


def command_name(name: str) -> str | None:
    return shutil.which(name)


def run_command(command: list[str], cwd: Path, timeout: int) -> tuple[int, str, float]:
    start = time.perf_counter()
    completed = subprocess.run(
        command,
        cwd=cwd,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        timeout=timeout,
        check=False,
    )
    elapsed_ms = (time.perf_counter() - start) * 1000.0
    return completed.returncode, completed.stdout.strip(), elapsed_ms


def prepare_sources(root: Path, workdir: Path, binary: Path) -> list[CommandSpec]:
    write(workdir / "spectra_counter.spectra", spectra_counter())
    write(workdir / "spectra_dp.spectra", spectra_dp())
    write(workdir / "spectra_grid.spectra", spectra_grid())
    write(workdir / "spectra_graph.spectra", spectra_graph())
    write(workdir / "spectra_string.spectra", spectra_string())
    write(workdir / "bench.py", PY_TEMPLATE.strip() + "\n")
    write(workdir / "bench.rs", RUST_TEMPLATE.strip() + "\n")
    write(workdir / "bench.go", GO_TEMPLATE.strip() + "\n")
    write(workdir / "Bench.java", JAVA_TEMPLATE.strip() + "\n")

    specs: list[CommandSpec] = []
    spectra_files = {
        "counter": workdir / "spectra_counter.spectra",
        "dp": workdir / "spectra_dp.spectra",
        "grid": workdir / "spectra_grid.spectra",
        "graph": workdir / "spectra_graph.spectra",
        "string": workdir / "spectra_string.spectra",
    }
    for workload, path in spectra_files.items():
        specs.append(
            CommandSpec(
                "spectra",
                workload,
                "cli_jit_end_to_end",
                [str(binary), "run", str(path)],
                root,
            )
        )

    python = command_name("python")
    if python:
        for workload in spectra_files:
            specs.append(
                CommandSpec(
                    "python",
                    workload,
                    "interpreter_startup_plus_execution",
                    [python, str(workdir / "bench.py"), workload],
                    root,
                )
            )

    rustc = command_name("rustc")
    if rustc:
        exe = workdir / ("bench_rust.exe" if os.name == "nt" else "bench_rust")
        specs.extend(
            CommandSpec(
                "rust",
                workload,
                "compiled_binary_startup_plus_execution",
                [str(exe), workload],
                root,
                [rustc, "-C", "opt-level=3", str(workdir / "bench.rs"), "-o", str(exe)],
            )
            for workload in spectra_files
        )

    go = command_name("go")
    if go:
        exe = workdir / ("bench_go.exe" if os.name == "nt" else "bench_go")
        specs.extend(
            CommandSpec(
                "go",
                workload,
                "compiled_binary_startup_plus_execution",
                [str(exe), workload],
                root,
                [go, "build", "-o", str(exe), str(workdir / "bench.go")],
            )
            for workload in spectra_files
        )

    javac = command_name("javac")
    java = command_name("java")
    if javac and java:
        specs.extend(
            CommandSpec(
                "java",
                workload,
                "jvm_startup_plus_execution",
                [java, "-cp", str(workdir), "Bench", workload],
                root,
                [javac, str(workdir / "Bench.java")],
            )
            for workload in spectra_files
        )

    return specs


def compile_once(specs: list[CommandSpec], timeout: int) -> dict[tuple[str, str], dict[str, object]]:
    compiled: dict[tuple[str, str], dict[str, object]] = {}
    seen: set[tuple[str, tuple[str, ...]]] = set()
    for spec in specs:
        if spec.compile_command is None:
            continue
        key = (spec.language, tuple(spec.compile_command))
        if key in seen:
            continue
        seen.add(key)
        exit_code, output, elapsed_ms = run_command(spec.compile_command, spec.cwd, timeout)
        compiled[(spec.language, spec.workload)] = {
            "exit_code": exit_code,
            "elapsed_ms": elapsed_ms,
            "output_tail": "\n".join(output.splitlines()[-20:]),
        }
        for other in specs:
            if other.language == spec.language and other.compile_command == spec.compile_command:
                compiled[(other.language, other.workload)] = compiled[(spec.language, spec.workload)]
    return compiled


def run_spec(spec: CommandSpec, iterations: int, warmups: int, timeout: int) -> dict[str, object]:
    samples: list[float] = []
    outputs: list[str] = []
    failures: list[str] = []
    total_runs = warmups + iterations
    for run_index in range(total_runs):
        try:
            exit_code, output, elapsed_ms = run_command(spec.command, spec.cwd, timeout)
        except subprocess.TimeoutExpired:
            failures.append(f"timeout after {timeout}s")
            continue
        if exit_code != 0:
            failures.append(f"exit {exit_code}: {' '.join(output.splitlines()[-3:])}")
            continue
        if run_index >= warmups:
            samples.append(elapsed_ms)
            outputs.append(output.splitlines()[-1] if output else "")

    return {
        "language": spec.language,
        "workload": spec.workload,
        "measurement_kind": spec.measurement_kind,
        "command": spec.command,
        "status": "passed" if len(samples) == iterations and not failures else "failed",
        "iterations": iterations,
        "warmups": warmups,
        "samples_ms": samples,
        "mean_ms": statistics.fmean(samples) if samples else None,
        "median_ms": statistics.median(samples) if samples else None,
        "p95_ms": percentile(samples, 0.95) if samples else None,
        "min_ms": min(samples) if samples else None,
        "max_ms": max(samples) if samples else None,
        "output": outputs[-1] if outputs else "",
        "failures": failures,
    }


def write_markdown(report: dict[str, object], path: Path) -> None:
    rows = report["results"]
    lines = [
        "# SpectraLang Language Comparison Benchmark",
        "",
        "Measurement is not apples-to-apples:",
        "",
        "- Spectra: CLI JIT end-to-end (`spectralang run`).",
        "- Python: interpreter startup plus execution.",
        "- Rust/Go: release-style compiled binary startup plus execution.",
        "- Java: JVM startup plus execution.",
        "",
        "| workload | language | kind | median ms | mean ms | p95 ms | output | status |",
        "|---|---:|---|---:|---:|---:|---:|---|",
    ]
    for row in sorted(rows, key=lambda r: (r["workload"], r["language"])):
        median = row["median_ms"]
        mean = row["mean_ms"]
        p95 = row["p95_ms"]
        lines.append(
            "| {workload} | {language} | {kind} | {median} | {mean} | {p95} | {output} | {status} |".format(
                workload=row["workload"],
                language=row["language"],
                kind=row["measurement_kind"],
                median="" if median is None else f"{median:.3f}",
                mean="" if mean is None else f"{mean:.3f}",
                p95="" if p95 is None else f"{p95:.3f}",
                output=row["output"],
                status=row["status"],
            )
        )
    lines.append("")
    lines.append(f"JSON report: `{report['output_json']}`")
    write(path, "\n".join(lines) + "\n")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", default=".")
    parser.add_argument("--binary", default="target/debug/spectralang.exe")
    parser.add_argument("--out", default="target/language-comparison/report.json")
    parser.add_argument("--markdown", default="target/language-comparison/report.md")
    parser.add_argument("--iterations", type=int, default=7)
    parser.add_argument("--warmups", type=int, default=1)
    parser.add_argument("--timeout-seconds", type=int, default=20)
    args = parser.parse_args()

    root = Path(args.root).resolve()
    binary = (root / args.binary).resolve()
    if not binary.exists():
        print(f"ERROR: Spectra binary not found: {binary}", file=sys.stderr)
        return 1

    out = (root / args.out).resolve()
    markdown = (root / args.markdown).resolve()
    workdir = out.parent
    specs = prepare_sources(root, workdir, binary)

    compile_reports = compile_once(specs, args.timeout_seconds)
    failed_compilers = {
        language
        for (language, _), report in compile_reports.items()
        if report["exit_code"] != 0
    }

    results = []
    for spec in specs:
        if spec.language in failed_compilers:
            results.append(
                {
                    "language": spec.language,
                    "workload": spec.workload,
                    "measurement_kind": spec.measurement_kind,
                    "status": "compile_failed",
                    "compile": compile_reports.get((spec.language, spec.workload)),
                    "iterations": args.iterations,
                    "warmups": args.warmups,
                    "samples_ms": [],
                    "mean_ms": None,
                    "median_ms": None,
                    "p95_ms": None,
                    "min_ms": None,
                    "max_ms": None,
                    "output": "",
                    "failures": [],
                }
            )
            continue
        results.append(run_spec(spec, args.iterations, args.warmups, args.timeout_seconds))

    available = sorted({spec.language for spec in specs})
    skipped = [
        language
        for language, commands in {
            "python": ["python"],
            "rust": ["rustc"],
            "go": ["go"],
            "java": ["javac", "java"],
        }.items()
        if language not in available and any(command_name(command) is None for command in commands)
    ]
    report = {
        "schema": SCHEMA,
        "generated_at_unix": int(time.time()),
        "host": {
            "platform": sys.platform,
            "python": sys.version.split()[0],
        },
        "notes": [
            "Spectra is measured through spectralang run, including frontend/lowering/codegen/JIT execution.",
            "Rust, Go, and Java are compiled before timing; compile time is not included in samples.",
            "Python samples include interpreter startup.",
        ],
        "iterations": args.iterations,
        "warmups": args.warmups,
        "available_languages": available,
        "skipped_languages": skipped,
        "compile_reports": {
            f"{language}:{workload}": value
            for (language, workload), value in sorted(compile_reports.items())
        },
        "results": results,
        "output_json": str(out.relative_to(root)),
        "output_markdown": str(markdown.relative_to(root)),
    }

    write(out, json.dumps(report, indent=2))
    write_markdown(report, markdown)
    print(f"wrote {out}")
    print(f"wrote {markdown}")

    failures = [row for row in results if row["status"] not in {"passed"}]
    if failures:
        for failure in failures:
            print(
                f"ERROR: {failure['language']} {failure['workload']} => {failure['status']}",
                file=sys.stderr,
            )
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
