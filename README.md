# A work in progress OS written in Rust
This is a work in progress OS written in Rust. The goal is to create a simple OS that can run on a real machine. The OS will be written in Rust and is mainly for educational purposes.

The basis of the OS will follow the [amazing tutorial](https://os.phil-opp.com/) by Phillip Oppermann. From there I have many plans to add on some interesting features. 

Update - as of now I am all caught up with that tutorial, so have begun to implement my own features:

Near future to-do list:
- [x] Fixed block-size allocators optimizations
- [ ] Page allocation system (in progress)
- [ ] Rework frame allocator
- [ ] Higher half kernel setup (possibly switch bootloader to GRUB)
- [ ] ASLR implementation