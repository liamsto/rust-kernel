# A Work-in-Progress OS Written in Rust

This is a work in progress OS written in Rust. The goal is to create a simple OS that can run on a real machine. The OS will be written in Rust for x86_64 and is mainly for educational purposes and to experiment with interesting OS features.


The foundation of the OS is build upon this [amazing tutorial](https://os.phil-opp.com/) by Phillip Oppermann. After laying that groundwork, I have shifted towards implementing unique features and optimizations.

## Current Status
As of now I am all caught up with that tutorial, so have begun to implement my own features. Some of them are completed, currently being worked on or simply planned for the future - you can track the progress below. 

## To-Do List:
### ✔️ Fixed Block-Size Allocator Optimizations
- **Status**: Completed.
- Currently, the OS uses a fixed-block size allocator for (almost) all of its heap allocations. I added optimizations such as pre - allocation of blocks (improves initial allocation performance) and limiting the maximum list length. I will also add other block sizes and alignments based on memory profiling in the future, but this is pretty trivial to add.  

### 🔄 Page Allocation System
- **Status**: In Progress.
- Currently the OS will fall back on a linked-list allocator provided by a crate from the tutorial. I am in the process of phasing this out - for allocations > 4 KiB, a page allocator will be used instead to reduce fragmentation. Then we can drop the linked list allocator (will also increase performance due to a better worst-case)

### 🔄 Rework Frame Allocator
- **Status**: In Progress.
- The current frame allocator is very basic and just follows the tutorial (it cannot free frames and does not track in use frames, it simply bumps a pointer to the next frame each time). I am in the process of creating a more advanced frame allocator (`BitmapFrameAllocator`). This will go hand in hand with the page allocator, since it requires a more advanced frame allocator to work. 

### ❓ Higher Half Kernel Setup
- **Status**: Planned.
- Transition the kernel to run in the higher half of the address space.
- **Possible enhancement**: Switch the bootloader from the `bootloader` crate to GRUB.

### ❓ Address Space Layout Randomization (ASLR) Implementation
- **Status**: Planned.
- Introduce ASLR to improve security.

### ❓ Network Stack
- **Status**: Planned.
- Implement a network stack. This is going to be a lot of work so is further down the line. 

