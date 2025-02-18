# A Work-in-Progress OS Written in Rust

This is a work in progress OS written in Rust. The goal is to create a simple OS that can run on a real machine. The OS will be written in Rust for x86_64 and is mainly for educational purposes and to experiment with interesting OS features.


The foundation of the OS is build upon this [amazing tutorial](https://os.phil-opp.com/) by Phillip Oppermann. After laying that groundwork, I have shifted towards implementing unique features and optimizations.

## Current Status
Currently in the process of migrating the a newer version of the bootloader crate, which brings with it a number of large necessary changes (VGA replaced with a framebuffer, UEFI booting and ACIP, possibly changes to GDT). This will be going on a `migration` branch rather than `main`.

### Migration To-Do
- [x] Replace `bootloader` with `bootloader-api`
- [x] Redesign `kernel_main` to support the `BootloaderConfig` setup
- [X] Move from VGA text mode to a pixel-based framebuffer
- [ ] Switch to UEFI (set up APIC)

## To-Do List:
| Feature                                    | Status       | Description                                                                                                                                  |
|--------------------------------------------|--------------|----------------------------------------------------------------------------------------------------------------------------------------------|
| **Fixed Block Size Allocator Optimizations** | Completed  | The OS uses a fixed-block size allocator for heap allocations that are less than half a page. Some optimizations include pre-allocating a number of blocks to reduce the initial memory latency. This was originally done just by carving up some memory, however, it integrated nicely with the page allocation system (see below). We can now easily pre-allocate pages of small-block memory based on common block sizes.   |
| **Page Allocation System**                 | Completed | Phased out the old fallback system (which used a linked-list allocator from crates.io) in favour of a page allocator. Fully integrated to work alongside the fixed block allocator (which interestingly, gave a nice boost in performance for small allocations as well). Now, any allocations that are large enough are handed a given a number of pages by the allocator, and any small allocations that do not have a block in the free list will be given a page to be carved up for small allocations.         |
| **Rework Frame Allocator**                 | Completed  | Replaced basic frame allocator with a bitmap-based allocator to track and free frames, improving flexibility and performance. Previously, the frame allocator just incremented a pointer every time a frame was allocated, but now we can track and deallocate frames as needed               |                    |
| **Address Space Layout Randomization (ASLR)** | Planned    | Introduce ASLR as a bit of a side project to play around with randomization. |
| **Network Stack**                          | Planned    | Implement a network stack, which will take a while so later down the line.|
| **Threads**                          | Planned    | Extend the existing executor system to implement multithreading.|
| **Shell System**                          | Planned    | Allow keyboard input to be used to run commands on the system and display the output to the screen|
| **File system**                          | Planned    | Create a proper, persistent file system.|

