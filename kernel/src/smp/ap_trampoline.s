.extern STACK_TOP
.extern BSPDONE
.extern APPRUNNING
.extern ap_startup
.section .text.trampoline, \"ax\"
.org 0x8000
_start_trampoline:
.code16
ap_init:
    cli
    cld
    .byte 0xEA
    .word 0x8040
    .word 0x0000

.align 16
_L8010_GDT_table:
    .long 0, 0
    .long 0x0000FFFF, 0x00CF9A00
    .long 0x0000FFFF, 0x008F9200
    .long 0x00000068, 0x00CF8900

.set gdt_limit, _L8030_GDT_value - _L8010_GDT_table - 1
_L8030_GDT_value:
    .word gdt_limit
    .long 0x8010
    .long 0, 0
.align 64

_L8040:
    xor ax, ax
    mov ds, ax
    lgdt [_L8030_GDT_value]
    mov eax, cr0
    or eax, 1
    mov cr0, eax
    .byte 0xEA
    .long _L8060
    .word 0x8

.align 32
.code32
_L8060:
    mov ax, 16
    mov ds, ax
    mov ss, ax
    mov eax, 1
    cpuid
    shr ebx, 24
    mov edi, ebx
    shl ebx, 15
    mov esp, STACK_TOP
    sub esp, ebx
    push edi

1:
    pause
    cmp byte ptr [BSPDONE], 0
    jz 1b
    lock inc byte ptr [APPRUNNING]
    .byte 0xEA
    .long ap_startup
    .word 0x8

_end_trampoline:
