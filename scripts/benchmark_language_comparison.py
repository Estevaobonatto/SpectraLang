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


def spectra_euclid() -> str:
    return """module bench_euclid;

import { println } from std.io;

fn gcd(a: int, b: int) -> int {
    let temp_a = a;
    let temp_b = b;
    while temp_b != 0 {
        let temp = temp_b;
        temp_b = temp_a % temp_b;
        temp_a = temp;
    }
    return temp_a;
}

pub fn main() -> int {
    let rounds = 200000;
    let i = 0;
    let acc = 0;
    while i < rounds {
        acc = acc + gcd(48, 18);
        acc = acc + gcd(56, 98);
        acc = acc + gcd(100, 35);
        acc = acc + gcd(270, 192);
        i = i + 1;
    }
    println(f"{acc}");
    return 0;
}
"""


def spectra_binary_search() -> str:
    return """module bench_binary_search;

import { println } from std.io;

fn binary_search(arr: [int], target: int, n: int) -> int {
    let left = 0;
    let right = n - 1;
    while left <= right {
        let mid = (left + right) / 2;
        if arr[mid] == target {
            return mid;
        }
        if arr[mid] < target {
            left = mid + 1;
        } else {
            right = mid - 1;
        }
    }
    return -1;
}

pub fn main() -> int {
    let rounds = 200000;
    let arr = [1, 3, 5, 7, 9, 11, 13, 15, 17, 19];
    let i = 0;
    let acc = 0;
    while i < rounds {
        let target = 0;
        while target < 20 {
            let idx = binary_search(arr, target, 10);
            if idx != -1 {
                acc = acc + idx;
            }
            target = target + 1;
        }
        i = i + 1;
    }
    println(f"{acc}");
    return 0;
}
"""


def spectra_quicksort() -> str:
    return """module bench_quicksort;

import { println } from std.io;

fn swap(arr: [int], i: int, j: int) {
    let tmp = arr[i];
    arr[i] = arr[j];
    arr[j] = tmp;
}

fn partition(arr: [int], low: int, high: int) -> int {
    let pivot = arr[high];
    let i = low - 1;
    let j = low;
    while j < high {
        if arr[j] <= pivot {
            i = i + 1;
            swap(arr, i, j);
        }
        j = j + 1;
    }
    swap(arr, i + 1, high);
    return i + 1;
}

fn quicksort(arr: [int], low: int, high: int) {
    if low < high {
        let p = partition(arr, low, high);
        quicksort(arr, low, p - 1);
        quicksort(arr, p + 1, high);
    }
}

fn run_once() -> int {
    let arr = [3, 6, 8, 10, 1, 2, 1, 9, 7, 5];
    quicksort(arr, 0, 9);
    return arr[0] + arr[4] + arr[9];
}

pub fn main() -> int {
    let rounds = 100000;
    let i = 0;
    let acc = 0;
    while i < rounds {
        acc = acc + run_once();
        i = i + 1;
    }
    println(f"{acc}");
    return 0;
}
"""


def spectra_mergesort() -> str:
    return """module bench_mergesort;

import { println } from std.io;

fn merge(arr: [int], left: int, mid: int, right: int, temp: [int]) {
    let i = left;
    let j = mid + 1;
    let k = left;
    while i <= mid && j <= right {
        if arr[i] <= arr[j] {
            temp[k] = arr[i];
            i = i + 1;
        } else {
            temp[k] = arr[j];
            j = j + 1;
        }
        k = k + 1;
    }
    while i <= mid {
        temp[k] = arr[i];
        i = i + 1;
        k = k + 1;
    }
    while j <= right {
        temp[k] = arr[j];
        j = j + 1;
        k = k + 1;
    }
    let t = left;
    while t <= right {
        arr[t] = temp[t];
        t = t + 1;
    }
}

fn mergesort(arr: [int], left: int, right: int, temp: [int]) {
    if left < right {
        let mid = (left + right) / 2;
        mergesort(arr, left, mid, temp);
        mergesort(arr, mid + 1, right, temp);
        merge(arr, left, mid, right, temp);
    }
}

fn run_once() -> int {
    let arr = [3, 6, 8, 10, 1, 2, 1, 9, 7, 5];
    let temp = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    mergesort(arr, 0, 9, temp);
    return arr[0] + arr[4] + arr[9];
}

pub fn main() -> int {
    let rounds = 100000;
    let i = 0;
    let acc = 0;
    while i < rounds {
        acc = acc + run_once();
        i = i + 1;
    }
    println(f"{acc}");
    return 0;
}
"""


def spectra_union_find() -> str:
    return """module bench_union_find;

import { println } from std.io;

fn find(parent: [int], x: int) -> int {
    let root = x;
    while parent[root] != root {
        parent[root] = parent[parent[root]];
        root = parent[root];
    }
    return root;
}

fn union(parent: [int], rank: [int], x: int, y: int) {
    let rx = find(parent, x);
    let ry = find(parent, y);
    if rx == ry {
        return;
    }
    if rank[rx] < rank[ry] {
        parent[rx] = ry;
    } else {
        if rank[rx] > rank[ry] {
            parent[ry] = rx;
        } else {
            parent[ry] = rx;
            rank[rx] = rank[rx] + 1;
        }
    }
}

fn run_once() -> int {
    let parent = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9];
    let rank = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    union(parent, rank, 0, 1);
    union(parent, rank, 1, 2);
    union(parent, rank, 3, 4);
    union(parent, rank, 4, 5);
    union(parent, rank, 6, 7);
    union(parent, rank, 7, 8);
    union(parent, rank, 8, 9);
    union(parent, rank, 0, 5);
    union(parent, rank, 2, 3);
    let acc = 0;
    let i = 0;
    while i < 10 {
        acc = acc + find(parent, i);
        i = i + 1;
    }
    return acc;
}

pub fn main() -> int {
    let rounds = 200000;
    let i = 0;
    let acc = 0;
    while i < rounds {
        acc = acc + run_once();
        i = i + 1;
    }
    println(f"{acc}");
    return 0;
}
"""


def spectra_bfs_dfs() -> str:
    return """module bench_bfs_dfs;

import { println } from std.io;

fn edge(weights: [int], n: int, u: int, v: int) -> int {
    return weights[u * n + v];
}

fn bfs_sum(weights: [int], n: int, start: int, visited: [int]) -> int {
    let queue = [0, 0, 0, 0, 0, 0];
    let front = 0;
    let back = 0;
    queue[back] = start;
    back = back + 1;
    visited[start] = 1;
    let acc = 0;
    while front < back {
        let u = queue[front];
        front = front + 1;
        acc = acc + u;
        let v = 0;
        while v < n {
            if edge(weights, n, u, v) > 0 && visited[v] == 0 {
                visited[v] = 1;
                queue[back] = v;
                back = back + 1;
            }
            v = v + 1;
        }
    }
    return acc;
}

fn dfs_sum(weights: [int], n: int, u: int, visited: [int]) -> int {
    visited[u] = 1;
    let acc = u;
    let v = 0;
    while v < n {
        if edge(weights, n, u, v) > 0 && visited[v] == 0 {
            acc = acc + dfs_sum(weights, n, v, visited);
        }
        v = v + 1;
    }
    return acc;
}

fn run_once() -> int {
    let weights = [
        0, 1, 1, 0, 0, 0,
        1, 0, 0, 1, 1, 0,
        1, 0, 0, 0, 0, 1,
        0, 1, 0, 0, 0, 0,
        0, 1, 0, 0, 0, 0,
        0, 0, 1, 0, 0, 0
    ];
    let visited1 = [0, 0, 0, 0, 0, 0];
    let visited2 = [0, 0, 0, 0, 0, 0];
    return bfs_sum(weights, 6, 0, visited1) + dfs_sum(weights, 6, 0, visited2);
}

pub fn main() -> int {
    let rounds = 100000;
    let i = 0;
    let acc = 0;
    while i < rounds {
        acc = acc + run_once();
        i = i + 1;
    }
    println(f"{acc}");
    return 0;
}
"""


def spectra_hash_table() -> str:
    return """module bench_hash_table;

import { println } from std.io;

struct Entry {
    key: int,
    value: int,
    used: int
}

fn hash(k: int, cap: int) -> int {
    let h = k % cap;
    if h < 0 {
        h = h + cap;
    }
    return h;
}

fn put(table: [Entry], cap: int, key: int, value: int) {
    let idx = hash(key, cap);
    while table[idx].used == 1 {
        if table[idx].key == key {
            table[idx].value = value;
            return;
        }
        idx = idx + 1;
        if idx == cap {
            idx = 0;
        }
    }
    table[idx].used = 1;
    table[idx].key = key;
    table[idx].value = value;
}

fn get(table: [Entry], cap: int, key: int) -> int {
    let idx = hash(key, cap);
    let probes = 0;
    while probes < cap {
        if table[idx].used == 0 {
            return -1;
        }
        if table[idx].key == key {
            return table[idx].value;
        }
        idx = idx + 1;
        if idx == cap {
            idx = 0;
        }
        probes = probes + 1;
    }
    return -1;
}

fn run_once() -> int {
    let table = [
        Entry { key: 0, value: 0, used: 0 }, Entry { key: 0, value: 0, used: 0 },
        Entry { key: 0, value: 0, used: 0 }, Entry { key: 0, value: 0, used: 0 },
        Entry { key: 0, value: 0, used: 0 }, Entry { key: 0, value: 0, used: 0 },
        Entry { key: 0, value: 0, used: 0 }, Entry { key: 0, value: 0, used: 0 },
        Entry { key: 0, value: 0, used: 0 }, Entry { key: 0, value: 0, used: 0 },
        Entry { key: 0, value: 0, used: 0 }, Entry { key: 0, value: 0, used: 0 },
        Entry { key: 0, value: 0, used: 0 }, Entry { key: 0, value: 0, used: 0 },
        Entry { key: 0, value: 0, used: 0 }, Entry { key: 0, value: 0, used: 0 }
    ];
    let cap = 16;
    let i = 0;
    while i < 10 {
        put(table, cap, i * 7, i * 3);
        i = i + 1;
    }
    let acc = 0;
    i = 0;
    while i < 10 {
        acc = acc + get(table, cap, i * 7);
        i = i + 1;
    }
    return acc;
}

pub fn main() -> int {
    let rounds = 100000;
    let i = 0;
    let acc = 0;
    while i < rounds {
        acc = acc + run_once();
        i = i + 1;
    }
    println(f"{acc}");
    return 0;
}
"""


def spectra_kmp() -> str:
    return """module bench_kmp;

import { println } from std.io;
import std.string as str;

fn build_lps(pattern: string, m: int, lps: [int]) {
    let len = 0;
    lps[0] = 0;
    let i = 1;
    while i < m {
        let c = str.char_at(pattern, i);
        let pc = str.char_at(pattern, len);
        if c == pc {
            len = len + 1;
            lps[i] = len;
            i = i + 1;
        } else {
            if len != 0 {
                len = lps[len - 1];
            } else {
                lps[i] = 0;
                i = i + 1;
            }
        }
    }
}

fn kmp_search(text: string, pattern: string) -> int {
    let n = str.len(text);
    let m = str.len(pattern);
    if m == 0 {
        return 0;
    }
    let lps = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    build_lps(pattern, m, lps);
    let i = 0;
    let j = 0;
    let count = 0;
    while i < n {
        let tc = str.char_at(text, i);
        let pc = str.char_at(pattern, j);
        if tc == pc {
            i = i + 1;
            j = j + 1;
            if j == m {
                count = count + 1;
                j = lps[j - 1];
            }
        } else {
            if j != 0 {
                j = lps[j - 1];
            } else {
                i = i + 1;
            }
        }
    }
    return count;
}

pub fn main() -> int {
    let rounds = 100000;
    let text = "abababababababababababababababab";
    let pattern = "abab";
    let i = 0;
    let acc = 0;
    while i < rounds {
        acc = acc + kmp_search(text, pattern);
        i = i + 1;
    }
    println(f"{acc}");
    return 0;
}
"""


def spectra_dijkstra() -> str:
    return """module bench_dijkstra;

import { println } from std.io;

fn edge(weights: [int], n: int, u: int, v: int) -> int {
    return weights[u * n + v];
}

fn dijkstra(weights: [int], n: int, src: int, dist: [int], visited: [int]) {
    let i = 0;
    while i < n {
        dist[i] = 99999;
        visited[i] = 0;
        i = i + 1;
    }
    dist[src] = 0;
    let step = 0;
    while step < n {
        let best = -1;
        let best_dist = 99999;
        let u = 0;
        while u < n {
            if visited[u] == 0 && dist[u] < best_dist {
                best_dist = dist[u];
                best = u;
            }
            u = u + 1;
        }
        if best == -1 {
            break;
        }
        visited[best] = 1;
        let v = 0;
        while v < n {
            let w = edge(weights, n, best, v);
            if w > 0 && visited[v] == 0 {
                let cand = dist[best] + w;
                if cand < dist[v] {
                    dist[v] = cand;
                }
            }
            v = v + 1;
        }
        step = step + 1;
    }
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
    let dist = [0, 0, 0, 0, 0, 0];
    let visited = [0, 0, 0, 0, 0, 0];
    dijkstra(weights, 6, 0, dist, visited);
    return dist[0] + dist[1] * 10 + dist[2] * 100 + dist[3] * 1000 + dist[4] * 10000 + dist[5] * 100000;
}

pub fn main() -> int {
    let rounds = 100000;
    let i = 0;
    let acc = 0;
    while i < rounds {
        acc = acc + run_once();
        i = i + 1;
    }
    println(f"{acc}");
    return 0;
}
"""


def spectra_huffman() -> str:
    return """module bench_huffman;

import { println } from std.io;

struct Node {
    freq: int,
    left: int,
    right: int,
    is_leaf: int,
    alive: int
}

fn tree_bits(nodes: [Node], idx: int, depth: int) -> int {
    if idx == -1 {
        return 0;
    }
    if nodes[idx].is_leaf == 1 {
        return nodes[idx].freq * depth;
    }
    return tree_bits(nodes, nodes[idx].left, depth + 1) + tree_bits(nodes, nodes[idx].right, depth + 1);
}

fn run_once() -> int {
    let freqs = [45, 13, 12, 16, 9, 5];
    let nodes = [
        Node { freq: 0, left: -1, right: -1, is_leaf: 0, alive: 0 },
        Node { freq: 0, left: -1, right: -1, is_leaf: 0, alive: 0 },
        Node { freq: 0, left: -1, right: -1, is_leaf: 0, alive: 0 },
        Node { freq: 0, left: -1, right: -1, is_leaf: 0, alive: 0 },
        Node { freq: 0, left: -1, right: -1, is_leaf: 0, alive: 0 },
        Node { freq: 0, left: -1, right: -1, is_leaf: 0, alive: 0 },
        Node { freq: 0, left: -1, right: -1, is_leaf: 0, alive: 0 },
        Node { freq: 0, left: -1, right: -1, is_leaf: 0, alive: 0 },
        Node { freq: 0, left: -1, right: -1, is_leaf: 0, alive: 0 },
        Node { freq: 0, left: -1, right: -1, is_leaf: 0, alive: 0 },
        Node { freq: 0, left: -1, right: -1, is_leaf: 0, alive: 0 }
    ];
    let n = 6;
    let count = n;
    let i = 0;
    while i < n {
        nodes[i].freq = freqs[i];
        nodes[i].is_leaf = 1;
        nodes[i].alive = 1;
        i = i + 1;
    }

    let rounds = 0;
    while rounds < n - 1 {
        let min1 = -1;
        let min2 = -1;
        let j = 0;
        while j < count {
            if nodes[j].alive == 1 {
                if min1 == -1 {
                    min1 = j;
                } else {
                    if nodes[j].freq < nodes[min1].freq {
                        min2 = min1;
                        min1 = j;
                    } else {
                        if min2 == -1 {
                            min2 = j;
                        } else {
                            if nodes[j].freq < nodes[min2].freq {
                                min2 = j;
                            }
                        }
                    }
                }
            }
            j = j + 1;
        }
        nodes[min1].alive = 0;
        nodes[min2].alive = 0;
        nodes[count].freq = nodes[min1].freq + nodes[min2].freq;
        nodes[count].left = min1;
        nodes[count].right = min2;
        nodes[count].is_leaf = 0;
        nodes[count].alive = 1;
        count = count + 1;
        rounds = rounds + 1;
    }

    let root = count - 1;
    return tree_bits(nodes, root, 0);
}

pub fn main() -> int {
    let rounds = 100000;
    let i = 0;
    let acc = 0;
    while i < rounds {
        acc = acc + run_once();
        i = i + 1;
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


def bench_euclid():
    return gcd(48, 18) + gcd(56, 98) + gcd(100, 35) + gcd(270, 192)

def gcd(a, b):
    while b != 0:
        a, b = b, a % b
    return a

def binary_search(arr, target):
    left, right = 0, len(arr) - 1
    while left <= right:
        mid = (left + right) // 2
        if arr[mid] == target:
            return mid
        if arr[mid] < target:
            left = mid + 1
        else:
            right = mid - 1
    return -1

def bench_binary_search():
    arr = [1, 3, 5, 7, 9, 11, 13, 15, 17, 19]
    acc = 0
    for target in range(20):
        idx = binary_search(arr, target)
        if idx != -1:
            acc += idx
    return acc

def bench_quicksort():
    arr = [3, 6, 8, 10, 1, 2, 1, 9, 7, 5]
    arr.sort()
    return arr[0] + arr[4] + arr[9]

def bench_mergesort():
    arr = [3, 6, 8, 10, 1, 2, 1, 9, 7, 5]
    arr.sort()
    return arr[0] + arr[4] + arr[9]

def find(parent, x):
    while parent[x] != x:
        parent[x] = parent[parent[x]]
        x = parent[x]
    return x

def union(parent, rank, x, y):
    rx, ry = find(parent, x), find(parent, y)
    if rx == ry:
        return
    if rank[rx] < rank[ry]:
        parent[rx] = ry
    elif rank[rx] > rank[ry]:
        parent[ry] = rx
    else:
        parent[ry] = rx
        rank[rx] += 1

def bench_union_find():
    parent = list(range(10))
    rank = [0] * 10
    union(parent, rank, 0, 1)
    union(parent, rank, 1, 2)
    union(parent, rank, 3, 4)
    union(parent, rank, 4, 5)
    union(parent, rank, 6, 7)
    union(parent, rank, 7, 8)
    union(parent, rank, 8, 9)
    union(parent, rank, 0, 5)
    union(parent, rank, 2, 3)
    return sum(find(parent, i) for i in range(10))

def edge2(weights, n, u, v):
    return weights[u * n + v]

def bfs_sum(weights, n, start, visited):
    queue = [start]
    visited[start] = True
    acc = 0
    while queue:
        u = queue.pop(0)
        acc += u
        for v in range(n):
            if edge2(weights, n, u, v) > 0 and not visited[v]:
                visited[v] = True
                queue.append(v)
    return acc

def dfs_sum(weights, n, u, visited):
    visited[u] = True
    acc = u
    for v in range(n):
        if edge2(weights, n, u, v) > 0 and not visited[v]:
            acc += dfs_sum(weights, n, v, visited)
    return acc

def bench_bfs_dfs():
    weights = [
        0, 1, 1, 0, 0, 0,
        1, 0, 0, 1, 1, 0,
        1, 0, 0, 0, 0, 1,
        0, 1, 0, 0, 0, 0,
        0, 1, 0, 0, 0, 0,
        0, 0, 1, 0, 0, 0
    ]
    visited1 = [False] * 6
    visited2 = [False] * 6
    return bfs_sum(weights, 6, 0, visited1) + dfs_sum(weights, 6, 0, visited2)

def bench_hash_table():
    table = {}
    for i in range(10):
        table[i * 7] = i * 3
    return sum(table.get(i * 7, -1) for i in range(10))

def build_lps(pattern):
    m = len(pattern)
    lps = [0] * m
    length = 0
    i = 1
    while i < m:
        if pattern[i] == pattern[length]:
            length += 1
            lps[i] = length
            i += 1
        else:
            if length != 0:
                length = lps[length - 1]
            else:
                lps[i] = 0
                i += 1
    return lps

def kmp_search(text, pattern):
    n, m = len(text), len(pattern)
    if m == 0:
        return 0
    lps = build_lps(pattern)
    i = j = count = 0
    while i < n:
        if text[i] == pattern[j]:
            i += 1
            j += 1
            if j == m:
                count += 1
                j = lps[j - 1]
        else:
            if j != 0:
                j = lps[j - 1]
            else:
                i += 1
    return count

def bench_kmp():
    return kmp_search("abababababababababababababababab", "abab")

def dijkstra(weights, n, src):
    dist = [99999] * n
    visited = [0] * n
    dist[src] = 0
    for _ in range(n):
        best = -1
        best_dist = 99999
        for u in range(n):
            if visited[u] == 0 and dist[u] < best_dist:
                best_dist = dist[u]
                best = u
        if best == -1:
            break
        visited[best] = 1
        for v in range(n):
            w = weights[best * n + v]
            if w > 0 and visited[v] == 0:
                cand = dist[best] + w
                if cand < dist[v]:
                    dist[v] = cand
    return dist

def bench_dijkstra():
    weights = [
        0, 7, 9, 0, 0, 14,
        7, 0, 10, 15, 0, 0,
        9, 10, 0, 11, 0, 2,
        0, 15, 11, 0, 6, 0,
        0, 0, 0, 6, 0, 9,
        14, 0, 2, 0, 9, 0
    ]
    dist = dijkstra(weights, 6, 0)
    return dist[0] + dist[1] * 10 + dist[2] * 100 + dist[3] * 1000 + dist[4] * 10000 + dist[5] * 100000

class HuffmanNode:
    __slots__ = ("freq", "left", "right", "is_leaf")
    def __init__(self, freq, left=None, right=None, is_leaf=False):
        self.freq = freq
        self.left = left
        self.right = right
        self.is_leaf = is_leaf

def huffman_bits(root, depth=0):
    if root is None:
        return 0
    if root.is_leaf:
        return root.freq * depth
    return huffman_bits(root.left, depth + 1) + huffman_bits(root.right, depth + 1)

def bench_huffman():
    freqs = [45, 13, 12, 16, 9, 5]
    nodes = [HuffmanNode(f, is_leaf=True) for f in freqs]
    import heapq
    heap = [(n.freq, i, n) for i, n in enumerate(nodes)]
    heapq.heapify(heap)
    while len(heap) > 1:
        f1, _, left = heapq.heappop(heap)
        f2, _, right = heapq.heappop(heap)
        merged = HuffmanNode(f1 + f2, left, right)
        heapq.heappush(heap, (merged.freq, 0, merged))
    return huffman_bits(heap[0][2])

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
elif workload == "euclid":
    acc = 0
    for _ in range(200000):
        acc += bench_euclid()
    print(acc)
elif workload == "binary_search":
    acc = 0
    for _ in range(200000):
        acc += bench_binary_search()
    print(acc)
elif workload == "quicksort":
    acc = 0
    for _ in range(100000):
        acc += bench_quicksort()
    print(acc)
elif workload == "mergesort":
    acc = 0
    for _ in range(100000):
        acc += bench_mergesort()
    print(acc)
elif workload == "union_find":
    acc = 0
    for _ in range(200000):
        acc += bench_union_find()
    print(acc)
elif workload == "bfs_dfs":
    acc = 0
    for _ in range(100000):
        acc += bench_bfs_dfs()
    print(acc)
elif workload == "hash_table":
    acc = 0
    for _ in range(100000):
        acc += bench_hash_table()
    print(acc)
elif workload == "kmp":
    acc = 0
    for _ in range(100000):
        acc += bench_kmp()
    print(acc)
elif workload == "dijkstra":
    acc = 0
    for _ in range(100000):
        acc += bench_dijkstra()
    print(acc)
elif workload == "huffman":
    acc = 0
    for _ in range(100000):
        acc += bench_huffman()
    print(acc)
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


fn gcd(mut a: i64, mut b: i64) -> i64 {
    while b != 0 {
        let tmp = b;
        b = a % b;
        a = tmp;
    }
    a
}

fn bench_euclid() -> i64 {
    let mut acc = 0i64;
    let mut i = 0;
    while i < 200_000 {
        acc += gcd(48, 18);
        acc += gcd(56, 98);
        acc += gcd(100, 35);
        acc += gcd(270, 192);
        i += 1;
    }
    acc
}

fn binary_search(arr: &[i64], target: i64) -> i64 {
    let mut left = 0usize;
    let mut right = arr.len() - 1;
    while left <= right {
        let mid = (left + right) / 2;
        if arr[mid] == target {
            return mid as i64;
        }
        if arr[mid] < target {
            left = mid + 1;
        } else {
            if right == 0 {
                break;
            }
            right = mid - 1;
        }
    }
    -1
}

fn bench_binary_search() -> i64 {
    let arr = [1i64, 3, 5, 7, 9, 11, 13, 15, 17, 19];
    let mut acc = 0i64;
    let mut i = 0;
    while i < 200_000 {
        let mut target = 0i64;
        while target < 20 {
            let idx = binary_search(&arr, target);
            if idx != -1 {
                acc += idx;
            }
            target += 1;
        }
        i += 1;
    }
    acc
}

fn bench_quicksort() -> i64 {
    let mut acc = 0i64;
    let mut i = 0;
    while i < 100_000 {
        let mut arr = [3i64, 6, 8, 10, 1, 2, 1, 9, 7, 5];
        arr.sort();
        acc += arr[0] + arr[4] + arr[9];
        i += 1;
    }
    acc
}

fn bench_mergesort() -> i64 {
    let mut acc = 0i64;
    let mut i = 0;
    while i < 100_000 {
        let mut arr = [3i64, 6, 8, 10, 1, 2, 1, 9, 7, 5];
        arr.sort();
        acc += arr[0] + arr[4] + arr[9];
        i += 1;
    }
    acc
}

fn find(parent: &mut [i64], x: usize) -> usize {
    let mut root = x;
    while parent[root] != root as i64 {
        parent[root] = parent[parent[root] as usize];
        root = parent[root] as usize;
    }
    root
}

fn union(parent: &mut [i64], rank: &mut [i64], x: usize, y: usize) {
    let rx = find(parent, x);
    let ry = find(parent, y);
    if rx == ry {
        return;
    }
    if rank[rx] < rank[ry] {
        parent[rx] = ry as i64;
    } else if rank[rx] > rank[ry] {
        parent[ry] = rx as i64;
    } else {
        parent[ry] = rx as i64;
        rank[rx] += 1;
    }
}

fn bench_union_find() -> i64 {
    let mut acc = 0i64;
    let mut i = 0;
    while i < 200_000 {
        let mut parent = [0i64, 1, 2, 3, 4, 5, 6, 7, 8, 9];
        let mut rank = [0i64; 10];
        union(&mut parent, &mut rank, 0, 1);
        union(&mut parent, &mut rank, 1, 2);
        union(&mut parent, &mut rank, 3, 4);
        union(&mut parent, &mut rank, 4, 5);
        union(&mut parent, &mut rank, 6, 7);
        union(&mut parent, &mut rank, 7, 8);
        union(&mut parent, &mut rank, 8, 9);
        union(&mut parent, &mut rank, 0, 5);
        union(&mut parent, &mut rank, 2, 3);
        for j in 0..10 {
            acc += find(&mut parent, j) as i64;
        }
        i += 1;
    }
    acc
}

fn edge2(weights: &[i64], n: usize, u: usize, v: usize) -> i64 {
    weights[u * n + v]
}

fn bfs_sum(weights: &[i64], n: usize, start: usize, visited: &mut [i64]) -> i64 {
    let mut queue = [0usize; 6];
    let mut front = 0usize;
    let mut back = 0usize;
    queue[back] = start;
    back += 1;
    visited[start] = 1;
    let mut acc = 0i64;
    while front < back {
        let u = queue[front];
        front += 1;
        acc += u as i64;
        for v in 0..n {
            if edge2(weights, n, u, v) > 0 && visited[v] == 0 {
                visited[v] = 1;
                queue[back] = v;
                back += 1;
            }
        }
    }
    acc
}

fn dfs_sum(weights: &[i64], n: usize, u: usize, visited: &mut [i64]) -> i64 {
    visited[u] = 1;
    let mut acc = u as i64;
    for v in 0..n {
        if edge2(weights, n, u, v) > 0 && visited[v] == 0 {
            acc += dfs_sum(weights, n, v, visited);
        }
    }
    acc
}

fn bench_bfs_dfs() -> i64 {
    let weights = [
        0, 1, 1, 0, 0, 0,
        1, 0, 0, 1, 1, 0,
        1, 0, 0, 0, 0, 1,
        0, 1, 0, 0, 0, 0,
        0, 1, 0, 0, 0, 0,
        0, 0, 1, 0, 0, 0
    ];
    let mut acc = 0i64;
    let mut i = 0;
    while i < 100_000 {
        let mut visited1 = [0i64; 6];
        let mut visited2 = [0i64; 6];
        acc += bfs_sum(&weights, 6, 0, &mut visited1);
        acc += dfs_sum(&weights, 6, 0, &mut visited2);
        i += 1;
    }
    acc
}

fn bench_hash_table() -> i64 {
    let mut acc = 0i64;
    let mut i = 0;
    while i < 100_000 {
        use std::collections::HashMap;
        let mut map = HashMap::new();
        for k in 0..10 {
            map.insert(k * 7, k * 3);
        }
        for k in 0..10 {
            acc += map.get(&(k * 7)).copied().unwrap_or(-1) as i64;
        }
        i += 1;
    }
    acc
}

fn build_lps(pattern: &[u8], lps: &mut [usize]) {
    let mut len = 0usize;
    lps[0] = 0;
    let mut i = 1usize;
    while i < pattern.len() {
        if pattern[i] == pattern[len] {
            len += 1;
            lps[i] = len;
            i += 1;
        } else {
            if len != 0 {
                len = lps[len - 1];
            } else {
                lps[i] = 0;
                i += 1;
            }
        }
    }
}

fn kmp_search(text: &[u8], pattern: &[u8], lps: &mut [usize]) -> i64 {
    let n = text.len();
    let m = pattern.len();
    let mut i = 0usize;
    let mut j = 0usize;
    let mut count = 0i64;
    build_lps(pattern, lps);
    while i < n {
        if text[i] == pattern[j] {
            i += 1;
            j += 1;
            if j == m {
                count += 1;
                j = lps[j - 1];
            }
        } else {
            if j != 0 {
                j = lps[j - 1];
            } else {
                i += 1;
            }
        }
    }
    count
}

fn bench_kmp() -> i64 {
    let text = b"abababababababababababababababab";
    let pattern = b"abab";
    let mut lps = [0usize; 16];
    let mut acc = 0i64;
    let mut i = 0;
    while i < 100_000 {
        acc += kmp_search(text, pattern, &mut lps);
        i += 1;
    }
    acc
}

fn dijkstra(weights: &[i64], n: usize, src: usize, dist: &mut [i64], visited: &mut [i64]) {
    for i in 0..n {
        dist[i] = 99999;
        visited[i] = 0;
    }
    dist[src] = 0;
    for _ in 0..n {
        let mut best = -1i64;
        let mut best_dist = 99999i64;
        for u in 0..n {
            if visited[u] == 0 && dist[u] < best_dist {
                best_dist = dist[u];
                best = u as i64;
            }
        }
        if best == -1 {
            break;
        }
        let best = best as usize;
        visited[best] = 1;
        for v in 0..n {
            let w = weights[best * n + v];
            if w > 0 && visited[v] == 0 {
                let cand = dist[best] + w;
                if cand < dist[v] {
                    dist[v] = cand;
                }
            }
        }
    }
}

fn bench_dijkstra() -> i64 {
    let weights = [
        0, 7, 9, 0, 0, 14,
        7, 0, 10, 15, 0, 0,
        9, 10, 0, 11, 0, 2,
        0, 15, 11, 0, 6, 0,
        0, 0, 0, 6, 0, 9,
        14, 0, 2, 0, 9, 0
    ];
    let mut acc = 0i64;
    let mut i = 0;
    while i < 100_000 {
        let mut dist = [0i64; 6];
        let mut visited = [0i64; 6];
        dijkstra(&weights, 6, 0, &mut dist, &mut visited);
        acc += dist[0] + dist[1] * 10 + dist[2] * 100 + dist[3] * 1000 + dist[4] * 10000 + dist[5] * 100000;
        i += 1;
    }
    acc
}

struct HuffmanNode {
    freq: i64,
    left: Option<Box<HuffmanNode>>,
    right: Option<Box<HuffmanNode>>,
    is_leaf: bool,
}

fn tree_bits(node: &HuffmanNode, depth: i64) -> i64 {
    if node.is_leaf {
        return node.freq * depth;
    }
    let mut acc = 0i64;
    if let Some(ref left) = node.left {
        acc += tree_bits(left, depth + 1);
    }
    if let Some(ref right) = node.right {
        acc += tree_bits(right, depth + 1);
    }
    acc
}

fn bench_huffman() -> i64 {
    use std::collections::BinaryHeap;
    use std::cmp::Reverse;
    let freqs = [45i64, 13, 12, 16, 9, 5];
    let mut acc = 0i64;
    for _ in 0..100_000 {
        let mut heap: BinaryHeap<Reverse<(i64, Box<HuffmanNode>)>> = BinaryHeap::new();
        for f in freqs {
            heap.push(Reverse((f, Box::new(HuffmanNode { freq: f, left: None, right: None, is_leaf: true }))));
        }
        while heap.len() > 1 {
            let Reverse((f1, left)) = heap.pop().unwrap();
            let Reverse((f2, right)) = heap.pop().unwrap();
            let merged = HuffmanNode {
                freq: f1 + f2,
                left: Some(left),
                right: Some(right),
                is_leaf: false,
            };
            heap.push(Reverse((merged.freq, Box::new(merged))));
        }
        let Reverse((_, root)) = heap.pop().unwrap();
        acc += tree_bits(&root, 0);
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
        "euclid" => bench_euclid(),
        "binary_search" => bench_binary_search(),
        "quicksort" => bench_quicksort(),
        "mergesort" => bench_mergesort(),
        "union_find" => bench_union_find(),
        "bfs_dfs" => bench_bfs_dfs(),
        "hash_table" => bench_hash_table(),
        "kmp" => bench_kmp(),
        "dijkstra" => bench_dijkstra(),
        "huffman" => bench_huffman(),
        _ => std::process::exit(2),
    };
    println!("{}", result);
}
"""


GO_TEMPLATE = r"""
package main

import (
    "container/heap"
    "fmt"
    "os"
    "sort"
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


func gcd(a int64, b int64) int64 {
    for b != 0 {
        a, b = b, a%b
    }
    return a
}

func benchEuclid() int64 {
    var acc int64 = 0
    for i := 0; i < 200000; i++ {
        acc += gcd(48, 18)
        acc += gcd(56, 98)
        acc += gcd(100, 35)
        acc += gcd(270, 192)
    }
    return acc
}

func binarySearch(arr []int64, target int64) int64 {
    left := 0
    right := len(arr) - 1
    for left <= right {
        mid := (left + right) / 2
        if arr[mid] == target {
            return int64(mid)
        }
        if arr[mid] < target {
            left = mid + 1
        } else {
            right = mid - 1
        }
    }
    return -1
}

func benchBinarySearch() int64 {
    arr := []int64{1, 3, 5, 7, 9, 11, 13, 15, 17, 19}
    var acc int64 = 0
    for i := 0; i < 200000; i++ {
        for target := int64(0); target < 20; target++ {
            idx := binarySearch(arr, target)
            if idx != -1 {
                acc += idx
            }
        }
    }
    return acc
}

func benchQuicksort() int64 {
    var acc int64 = 0
    for i := 0; i < 100000; i++ {
        arr := []int64{3, 6, 8, 10, 1, 2, 1, 9, 7, 5}
        sort.Slice(arr, func(a, b int) bool { return arr[a] < arr[b] })
        acc += arr[0] + arr[4] + arr[9]
    }
    return acc
}

func benchMergesort() int64 {
    var acc int64 = 0
    for i := 0; i < 100000; i++ {
        arr := []int64{3, 6, 8, 10, 1, 2, 1, 9, 7, 5}
        sort.Slice(arr, func(a, b int) bool { return arr[a] < arr[b] })
        acc += arr[0] + arr[4] + arr[9]
    }
    return acc
}

func find(parent []int, x int) int {
    root := x
    for parent[root] != root {
        parent[root] = parent[parent[root]]
        root = parent[root]
    }
    return root
}

func union(parent []int, rank []int, x int, y int) {
    rx := find(parent, x)
    ry := find(parent, y)
    if rx == ry {
        return
    }
    if rank[rx] < rank[ry] {
        parent[rx] = ry
    } else if rank[rx] > rank[ry] {
        parent[ry] = rx
    } else {
        parent[ry] = rx
        rank[rx]++
    }
}

func benchUnionFind() int64 {
    var acc int64 = 0
    for i := 0; i < 200000; i++ {
        parent := []int{0, 1, 2, 3, 4, 5, 6, 7, 8, 9}
        rank := []int{0, 0, 0, 0, 0, 0, 0, 0, 0, 0}
        union(parent, rank, 0, 1)
        union(parent, rank, 1, 2)
        union(parent, rank, 3, 4)
        union(parent, rank, 4, 5)
        union(parent, rank, 6, 7)
        union(parent, rank, 7, 8)
        union(parent, rank, 8, 9)
        union(parent, rank, 0, 5)
        union(parent, rank, 2, 3)
        for j := 0; j < 10; j++ {
            acc += int64(find(parent, j))
        }
    }
    return acc
}

func edge2(weights []int64, n int, u int, v int) int64 {
    return weights[u*n+v]
}

func bfsSum(weights []int64, n int, start int, visited []int) int64 {
    queue := make([]int, 0, n)
    queue = append(queue, start)
    visited[start] = 1
    var acc int64 = 0
    for len(queue) > 0 {
        u := queue[0]
        queue = queue[1:]
        acc += int64(u)
        for v := 0; v < n; v++ {
            if edge2(weights, n, u, v) > 0 && visited[v] == 0 {
                visited[v] = 1
                queue = append(queue, v)
            }
        }
    }
    return acc
}

func dfsSum(weights []int64, n int, u int, visited []int) int64 {
    visited[u] = 1
    var acc int64 = int64(u)
    for v := 0; v < n; v++ {
        if edge2(weights, n, u, v) > 0 && visited[v] == 0 {
            acc += dfsSum(weights, n, v, visited)
        }
    }
    return acc
}

func benchBfsDfs() int64 {
    weights := []int64{
        0, 1, 1, 0, 0, 0,
        1, 0, 0, 1, 1, 0,
        1, 0, 0, 0, 0, 1,
        0, 1, 0, 0, 0, 0,
        0, 1, 0, 0, 0, 0,
        0, 0, 1, 0, 0, 0,
    }
    var acc int64 = 0
    for i := 0; i < 100000; i++ {
        visited1 := []int{0, 0, 0, 0, 0, 0}
        visited2 := []int{0, 0, 0, 0, 0, 0}
        acc += bfsSum(weights, 6, 0, visited1)
        acc += dfsSum(weights, 6, 0, visited2)
    }
    return acc
}

func benchHashTable() int64 {
    var acc int64 = 0
    for i := 0; i < 100000; i++ {
        m := make(map[int]int)
        for k := 0; k < 10; k++ {
            m[k*7] = k * 3
        }
        for k := 0; k < 10; k++ {
            if v, ok := m[k*7]; ok {
                acc += int64(v)
            }
        }
    }
    return acc
}

func buildLps(pattern []byte, lps []int) {
    length := 0
    lps[0] = 0
    i := 1
    for i < len(pattern) {
        if pattern[i] == pattern[length] {
            length++
            lps[i] = length
            i++
        } else {
            if length != 0 {
                length = lps[length-1]
            } else {
                lps[i] = 0
                i++
            }
        }
    }
}

func kmpSearch(text []byte, pattern []byte, lps []int) int64 {
    n := len(text)
    m := len(pattern)
    buildLps(pattern, lps)
    i := 0
    j := 0
    var count int64 = 0
    for i < n {
        if text[i] == pattern[j] {
            i++
            j++
            if j == m {
                count++
                j = lps[j-1]
            }
        } else {
            if j != 0 {
                j = lps[j-1]
            } else {
                i++
            }
        }
    }
    return count
}

func benchKmp() int64 {
    text := []byte("abababababababababababababababab")
    pattern := []byte("abab")
    lps := make([]int, 16)
    var acc int64 = 0
    for i := 0; i < 100000; i++ {
        acc += kmpSearch(text, pattern, lps)
    }
    return acc
}

func dijkstra(weights []int64, n int, src int, dist []int64, visited []int) {
    for i := 0; i < n; i++ {
        dist[i] = 99999
        visited[i] = 0
    }
    dist[src] = 0
    for step := 0; step < n; step++ {
        best := -1
        var bestDist int64 = 99999
        for u := 0; u < n; u++ {
            if visited[u] == 0 && dist[u] < bestDist {
                bestDist = dist[u]
                best = u
            }
        }
        if best == -1 {
            break
        }
        visited[best] = 1
        for v := 0; v < n; v++ {
            w := weights[best*n+v]
            if w > 0 && visited[v] == 0 {
                cand := dist[best] + w
                if cand < dist[v] {
                    dist[v] = cand
                }
            }
        }
    }
}

func benchDijkstra() int64 {
    weights := []int64{
        0, 7, 9, 0, 0, 14,
        7, 0, 10, 15, 0, 0,
        9, 10, 0, 11, 0, 2,
        0, 15, 11, 0, 6, 0,
        0, 0, 0, 6, 0, 9,
        14, 0, 2, 0, 9, 0,
    }
    var acc int64 = 0
    for i := 0; i < 100000; i++ {
        dist := []int64{0, 0, 0, 0, 0, 0}
        visited := []int{0, 0, 0, 0, 0, 0}
        dijkstra(weights, 6, 0, dist, visited)
        acc += dist[0] + dist[1]*10 + dist[2]*100 + dist[3]*1000 + dist[4]*10000 + dist[5]*100000
    }
    return acc
}

type HuffmanNode struct {
    freq    int64
    left    *HuffmanNode
    right   *HuffmanNode
    isLeaf  bool
}

type HuffmanItem struct {
    freq int64
    node *HuffmanNode
}

type HuffmanHeap []HuffmanItem

func (h HuffmanHeap) Len() int { return len(h) }
func (h HuffmanHeap) Less(i, j int) bool { return h[i].freq < h[j].freq }
func (h HuffmanHeap) Swap(i, j int) { h[i], h[j] = h[j], h[i] }
func (h *HuffmanHeap) Push(x interface{}) { *h = append(*h, x.(HuffmanItem)) }
func (h *HuffmanHeap) Pop() interface{} {
    old := *h
    n := len(old)
    item := old[n-1]
    *h = old[:n-1]
    return item
}

func treeBits(node *HuffmanNode, depth int64) int64 {
    if node == nil {
        return 0
    }
    if node.isLeaf {
        return node.freq * depth
    }
    return treeBits(node.left, depth+1) + treeBits(node.right, depth+1)
}

func benchHuffman() int64 {
    freqs := []int64{45, 13, 12, 16, 9, 5}
    var acc int64 = 0
    for i := 0; i < 100000; i++ {
        h := &HuffmanHeap{}
        heap.Init(h)
        for _, f := range freqs {
            heap.Push(h, HuffmanItem{f, &HuffmanNode{freq: f, isLeaf: true}})
        }
        for h.Len() > 1 {
            a := heap.Pop(h).(HuffmanItem)
            b := heap.Pop(h).(HuffmanItem)
            merged := &HuffmanNode{freq: a.freq + b.freq, left: a.node, right: b.node}
            heap.Push(h, HuffmanItem{merged.freq, merged})
        }
        root := heap.Pop(h).(HuffmanItem).node
        acc += treeBits(root, 0)
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
    case "euclid":
        fmt.Println(benchEuclid())
    case "binary_search":
        fmt.Println(benchBinarySearch())
    case "quicksort":
        fmt.Println(benchQuicksort())
    case "mergesort":
        fmt.Println(benchMergesort())
    case "union_find":
        fmt.Println(benchUnionFind())
    case "bfs_dfs":
        fmt.Println(benchBfsDfs())
    case "hash_table":
        fmt.Println(benchHashTable())
    case "kmp":
        fmt.Println(benchKmp())
    case "dijkstra":
        fmt.Println(benchDijkstra())
    case "huffman":
        fmt.Println(benchHuffman())
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


    static long gcd(long a, long b) {
        while (b != 0) {
            long tmp = b;
            b = a % b;
            a = tmp;
        }
        return a;
    }

    static long benchEuclid() {
        long acc = 0;
        for (int i = 0; i < 200_000; i++) {
            acc += gcd(48, 18);
            acc += gcd(56, 98);
            acc += gcd(100, 35);
            acc += gcd(270, 192);
        }
        return acc;
    }

    static int binarySearch(int[] arr, int target) {
        int left = 0;
        int right = arr.length - 1;
        while (left <= right) {
            int mid = (left + right) / 2;
            if (arr[mid] == target) {
                return mid;
            }
            if (arr[mid] < target) {
                left = mid + 1;
            } else {
                right = mid - 1;
            }
        }
        return -1;
    }

    static long benchBinarySearch() {
        int[] arr = {1, 3, 5, 7, 9, 11, 13, 15, 17, 19};
        long acc = 0;
        for (int i = 0; i < 200_000; i++) {
            for (int target = 0; target < 20; target++) {
                int idx = binarySearch(arr, target);
                if (idx != -1) {
                    acc += idx;
                }
            }
        }
        return acc;
    }

    static long benchQuicksort() {
        long acc = 0;
        for (int i = 0; i < 100_000; i++) {
            int[] arr = {3, 6, 8, 10, 1, 2, 1, 9, 7, 5};
            java.util.Arrays.sort(arr);
            acc += arr[0] + arr[4] + arr[9];
        }
        return acc;
    }

    static long benchMergesort() {
        long acc = 0;
        for (int i = 0; i < 100_000; i++) {
            int[] arr = {3, 6, 8, 10, 1, 2, 1, 9, 7, 5};
            java.util.Arrays.sort(arr);
            acc += arr[0] + arr[4] + arr[9];
        }
        return acc;
    }

    static int find(int[] parent, int x) {
        int root = x;
        while (parent[root] != root) {
            parent[root] = parent[parent[root]];
            root = parent[root];
        }
        return root;
    }

    static void union(int[] parent, int[] rank, int x, int y) {
        int rx = find(parent, x);
        int ry = find(parent, y);
        if (rx == ry) {
            return;
        }
        if (rank[rx] < rank[ry]) {
            parent[rx] = ry;
        } else if (rank[rx] > rank[ry]) {
            parent[ry] = rx;
        } else {
            parent[ry] = rx;
            rank[rx]++;
        }
    }

    static long benchUnionFind() {
        long acc = 0;
        for (int i = 0; i < 200_000; i++) {
            int[] parent = {0, 1, 2, 3, 4, 5, 6, 7, 8, 9};
            int[] rank = {0, 0, 0, 0, 0, 0, 0, 0, 0, 0};
            union(parent, rank, 0, 1);
            union(parent, rank, 1, 2);
            union(parent, rank, 3, 4);
            union(parent, rank, 4, 5);
            union(parent, rank, 6, 7);
            union(parent, rank, 7, 8);
            union(parent, rank, 8, 9);
            union(parent, rank, 0, 5);
            union(parent, rank, 2, 3);
            for (int j = 0; j < 10; j++) {
                acc += find(parent, j);
            }
        }
        return acc;
    }

    static int edge2(int[] weights, int n, int u, int v) {
        return weights[u * n + v];
    }

    static long bfsSum(int[] weights, int n, int start, boolean[] visited) {
        int[] queue = new int[n];
        int front = 0;
        int back = 0;
        queue[back++] = start;
        visited[start] = true;
        long acc = 0;
        while (front < back) {
            int u = queue[front++];
            acc += u;
            for (int v = 0; v < n; v++) {
                if (edge2(weights, n, u, v) > 0 && !visited[v]) {
                    visited[v] = true;
                    queue[back++] = v;
                }
            }
        }
        return acc;
    }

    static long dfsSum(int[] weights, int n, int u, boolean[] visited) {
        visited[u] = true;
        long acc = u;
        for (int v = 0; v < n; v++) {
            if (edge2(weights, n, u, v) > 0 && !visited[v]) {
                acc += dfsSum(weights, n, v, visited);
            }
        }
        return acc;
    }

    static long benchBfsDfs() {
        int[] weights = {
            0, 1, 1, 0, 0, 0,
            1, 0, 0, 1, 1, 0,
            1, 0, 0, 0, 0, 1,
            0, 1, 0, 0, 0, 0,
            0, 1, 0, 0, 0, 0,
            0, 0, 1, 0, 0, 0
        };
        long acc = 0;
        for (int i = 0; i < 100_000; i++) {
            boolean[] visited1 = new boolean[6];
            boolean[] visited2 = new boolean[6];
            acc += bfsSum(weights, 6, 0, visited1);
            acc += dfsSum(weights, 6, 0, visited2);
        }
        return acc;
    }

    static long benchHashTable() {
        long acc = 0;
        for (int i = 0; i < 100_000; i++) {
            java.util.HashMap<Integer, Integer> map = new java.util.HashMap<>();
            for (int k = 0; k < 10; k++) {
                map.put(k * 7, k * 3);
            }
            for (int k = 0; k < 10; k++) {
                acc += map.getOrDefault(k * 7, -1);
            }
        }
        return acc;
    }

    static void buildLps(String pattern, int[] lps) {
        int length = 0;
        lps[0] = 0;
        int i = 1;
        while (i < pattern.length()) {
            if (pattern.charAt(i) == pattern.charAt(length)) {
                length++;
                lps[i] = length;
                i++;
            } else {
                if (length != 0) {
                    length = lps[length - 1];
                } else {
                    lps[i] = 0;
                    i++;
                }
            }
        }
    }

    static long kmpSearch(String text, String pattern, int[] lps) {
        int n = text.length();
        int m = pattern.length();
        buildLps(pattern, lps);
        int i = 0;
        int j = 0;
        long count = 0;
        while (i < n) {
            if (text.charAt(i) == pattern.charAt(j)) {
                i++;
                j++;
                if (j == m) {
                    count++;
                    j = lps[j - 1];
                }
            } else {
                if (j != 0) {
                    j = lps[j - 1];
                } else {
                    i++;
                }
            }
        }
        return count;
    }

    static long benchKmp() {
        String text = "abababababababababababababababab";
        String pattern = "abab";
        int[] lps = new int[16];
        long acc = 0;
        for (int i = 0; i < 100_000; i++) {
            acc += kmpSearch(text, pattern, lps);
        }
        return acc;
    }

    static void dijkstra(int[] weights, int n, int src, int[] dist, int[] visited) {
        for (int i = 0; i < n; i++) {
            dist[i] = 99999;
            visited[i] = 0;
        }
        dist[src] = 0;
        for (int step = 0; step < n; step++) {
            int best = -1;
            int bestDist = 99999;
            for (int u = 0; u < n; u++) {
                if (visited[u] == 0 && dist[u] < bestDist) {
                    bestDist = dist[u];
                    best = u;
                }
            }
            if (best == -1) {
                break;
            }
            visited[best] = 1;
            for (int v = 0; v < n; v++) {
                int w = weights[best * n + v];
                if (w > 0 && visited[v] == 0) {
                    int cand = dist[best] + w;
                    if (cand < dist[v]) {
                        dist[v] = cand;
                    }
                }
            }
        }
    }

    static long benchDijkstra() {
        int[] weights = {
            0, 7, 9, 0, 0, 14,
            7, 0, 10, 15, 0, 0,
            9, 10, 0, 11, 0, 2,
            0, 15, 11, 0, 6, 0,
            0, 0, 0, 6, 0, 9,
            14, 0, 2, 0, 9, 0
        };
        long acc = 0;
        for (int i = 0; i < 100_000; i++) {
            int[] dist = new int[6];
            int[] visited = new int[6];
            dijkstra(weights, 6, 0, dist, visited);
            acc += dist[0] + dist[1] * 10L + dist[2] * 100L + dist[3] * 1000L + dist[4] * 10000L + dist[5] * 100000L;
        }
        return acc;
    }

    static class HuffmanNode {
        long freq;
        HuffmanNode left, right;
        boolean isLeaf;
        HuffmanNode(long freq, boolean isLeaf) {
            this.freq = freq;
            this.isLeaf = isLeaf;
        }
    }

    static long treeBits(HuffmanNode node, int depth) {
        if (node == null) {
            return 0;
        }
        if (node.isLeaf) {
            return node.freq * depth;
        }
        return treeBits(node.left, depth + 1) + treeBits(node.right, depth + 1);
    }

    static long benchHuffman() {
        int[] freqs = {45, 13, 12, 16, 9, 5};
        long acc = 0;
        for (int i = 0; i < 100_000; i++) {
            java.util.PriorityQueue<HuffmanNode> pq = new java.util.PriorityQueue<>(java.util.Comparator.comparingLong(n -> n.freq));
            for (int f : freqs) {
                pq.add(new HuffmanNode(f, true));
            }
            while (pq.size() > 1) {
                HuffmanNode left = pq.poll();
                HuffmanNode right = pq.poll();
                HuffmanNode merged = new HuffmanNode(left.freq + right.freq, false);
                merged.left = left;
                merged.right = right;
                pq.add(merged);
            }
            acc += treeBits(pq.poll(), 0);
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
            case "euclid":
                result = benchEuclid();
                break;
            case "binary_search":
                result = benchBinarySearch();
                break;
            case "quicksort":
                result = benchQuicksort();
                break;
            case "mergesort":
                result = benchMergesort();
                break;
            case "union_find":
                result = benchUnionFind();
                break;
            case "bfs_dfs":
                result = benchBfsDfs();
                break;
            case "hash_table":
                result = benchHashTable();
                break;
            case "kmp":
                result = benchKmp();
                break;
            case "dijkstra":
                result = benchDijkstra();
                break;
            case "huffman":
                result = benchHuffman();
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
    write(workdir / "spectra_euclid.spectra", spectra_euclid())
    write(workdir / "spectra_binary_search.spectra", spectra_binary_search())
    write(workdir / "spectra_quicksort.spectra", spectra_quicksort())
    write(workdir / "spectra_mergesort.spectra", spectra_mergesort())
    write(workdir / "spectra_union_find.spectra", spectra_union_find())
    write(workdir / "spectra_bfs_dfs.spectra", spectra_bfs_dfs())
    write(workdir / "spectra_hash_table.spectra", spectra_hash_table())
    write(workdir / "spectra_kmp.spectra", spectra_kmp())
    write(workdir / "spectra_dijkstra.spectra", spectra_dijkstra())
    write(workdir / "spectra_huffman.spectra", spectra_huffman())
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
        "euclid": workdir / "spectra_euclid.spectra",
        "binary_search": workdir / "spectra_binary_search.spectra",
        "quicksort": workdir / "spectra_quicksort.spectra",
        "mergesort": workdir / "spectra_mergesort.spectra",
        "union_find": workdir / "spectra_union_find.spectra",
        "bfs_dfs": workdir / "spectra_bfs_dfs.spectra",
        "hash_table": workdir / "spectra_hash_table.spectra",
        "kmp": workdir / "spectra_kmp.spectra",
        "dijkstra": workdir / "spectra_dijkstra.spectra",
        "huffman": workdir / "spectra_huffman.spectra",
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
