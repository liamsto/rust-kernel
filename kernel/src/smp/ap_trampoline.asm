extern STACK_TOP
extern BSPDONE
extern APPRUNNING
extern ap_startup

section .text.trampoline

;--- Begin 16-bit Code ---
bits 16
_start_trampoline:
ap_init:
    cli
    cld
    db 0xEA               ; Far jump opcode
    dw 0x8040             ; Offset
    dw 0x0000             ; Segment

align 16

_L8010_GDT_table:
    dd 0, 0
    dd 0x0000FFFF, 0x00CF9A00   ; flat code descriptor
    dd 0x0000FFFF, 0x008F9200   ; flat data descriptor
    dd 0x00000068, 0x00CF8900   ; TSS descriptor

_L8030_GDT_value:
    dw _L8030_GDT_value - _L8010_GDT_table - 1
    dd 0x8010
    dd 0, 0

align 64

_L8040:
    xor ax, ax
    mov ds, ax
    lgdt [_L8030_GDT_value]
    mov eax, cr0
    or eax, 1
    mov cr0, eax
    db 0xEA               ; Far jump opcode
    dd _L8060
    dw 0x8

;--- End 16-bit Code; Begin 32-bit Code ---
align 32
bits 32
_L8060:
    mov ax, 16
    mov ds, ax
    mov ss, ax
    mov eax, 1
    cpuid
    shr ebx, 24
    mov edi, ebx
    shl ebx, 15
    mov esp, dword [STACK_TOP]
    sub esp, ebx
    push edi

ap_wait:
    pause
    cmp byte [BSPDONE], 0
    jz ap_wait
    lock inc byte [APPRUNNING]
    db 0xEA               ; Far jump opcode
    dd ap_startup
    dw 0x8

_end_trampoline:
