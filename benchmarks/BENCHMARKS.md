# Benchmarks

## Table of Contents

- [Benchmark Results](#benchmark-results)
    - [group_context(1184046)](#group_context(1184046))
    - [vlbytes(100004)](#vlbytes(100004))

## Benchmark Results

Ran on `Linux 7.2.11-gentoo-x86_64 #1 SMP PREEMPT_DYNAMIC x86_64 AMD Ryzen 9 5950X 16-Core Processor AuthenticAMD GNU/Linux`

Inputs are randomized to prevent the compiler from optimizing out everything.
Before that, `thalassa` was getting a flat 20ns on deserialization no matter the size of the input, giving out impossible 500TB/s data rates on 22MB GroupContext payloads. Lol.
For context, most modern CPUs L1 cache tops out at around 5TB/s, so that would be pretty much impossible, even with all the optimizations in place.

### group_context(1184046)

|           | `tls_codec`               | `thalassa`                           |
|:----------|:--------------------------|:------------------------------------ |
| **`de`**  | `392.82 us` (✅ **1.00x**) | `178.40 ns` (🚀 **2201.90x faster**)  |
| **`ser`** | `3.18 ms` (✅ **1.00x**)   | `44.75 us` (🚀 **70.99x faster**)     |

### vlbytes(100004)

|           | `tls_codec`             | `thalassa`                        |
|:----------|:------------------------|:--------------------------------- |
| **`de`**  | `3.69 us` (✅ **1.00x**) | `193.33 ns` (🚀 **19.09x faster**) |
| **`ser`** | `2.78 us` (✅ **1.00x**) | `2.30 us` (✅ **1.21x faster**)    |

---
Made with [criterion-table](https://github.com/nu11ptr/criterion-table)
