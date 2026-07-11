# Benchmarks

## Table of Contents

- [Benchmark Results](#benchmark-results)
    - [group_context](#group_context)

## Benchmark Results

### group_context

|           | `tls_codec`              | `thalassa`                         |
|:----------|:-------------------------|:---------------------------------- |
| **`de`**  | `37.07 us` (✅ **1.00x**) | `41.69 ns` (🚀 **889.15x faster**)  |
| **`ser`** | `9.08 us` (✅ **1.00x**)  | `120.76 ns` (🚀 **75.19x faster**)  |

---
Made with [criterion-table](https://github.com/nu11ptr/criterion-table)
