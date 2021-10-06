# Xilinx AXI DMA Userspace Driver

This crates uses [udmabuf](https://github.com/ikwzm/udmabuf) and a generic
userspace I/O driver (`uio_pdrv_genirq`) to interface Xilinx AXI DMA
controllers. Please see [this blog post](https://www.bastibl.net/futuresdr-2/)
and the [example
directory](https://github.com/FutureSDR/xilinx-dma/tree/main/examples) for
further information.

[![Crates.io][crates-badge]][crates-url]
[![Apache 2.0 licensed][apache-badge]][apache-url]

[crates-badge]: https://img.shields.io/crates/v/xilinx-dma.svg
[crates-url]: https://crates.io/crates/xilinx-dma
[apache-badge]: https://img.shields.io/badge/license-Apache%202-blue
[apache-url]: https://github.com/futuresdr/xilinx-dma/blob/main/LICENSE

## Overview

The project is very much work-in-progress. At the moment, it only supports
register mode transfers (i.e., no scatter gather). The crate supports sync and
async operation.


## Contributions

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the project, shall be licensed as Apache 2.0, without any
additional terms or conditions.
