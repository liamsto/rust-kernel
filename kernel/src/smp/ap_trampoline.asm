; ap_trampoline.asm
; Self-contained trampoline designed to be loaded at physical address 0x8000.
; It reserves data fields that will be patched by the BSP:
;   cr3val:    4 bytes (offset 0)
;   kcode:     8 bytes (offset 4) – kernel entry pointer (64-bit)
;   kstack:    8 bytes (offset 12) – kernel stack pointer for this AP
;   kgsval:    8 bytes (offset 20) – GS base value
;   commword:  4 bytes (offset 28) – communication flag; AP sets to 1 when ready

section .rodata
    cr3val:    dd 0
    kcode:     dq 0
    kstack:    dq 0
    kgsval:    dq 0
    commword:  dd 0

section .text
global trampoline
; Start in 16-bit real mode.
bits 16
trampoline:
    jmp real_start

; Define a minimal GDT.
align 4
gdt:
    dw 0, 0                      ; null descriptor
    dw 0xFFFF, 0                 ; code segment limit
    dd 0, 0x9A00                 ; code segment: base=0, type=code, present=1 (selector 0x08)
    dw 0xFFFF, 0                 ; data segment limit
    dd 0, 0x9200                 ; data segment: base=0, type=data, present=1 (selector 0x10)
gdt_end:
gdt_ptr:
    dw gdt_end - gdt - 1
    dd gdt - trampoline

real_start:
    ; Set DS = CS.
    mov ax, cs
    mov ds, ax
    ; Signal readiness: set commword OR 1.
    or dword [commword], 1
    ; Load the minimal GDT.
    lgdt [gdt_ptr]
    ; Enable protected mode by setting PE in CR0.
    mov eax, cr0
    bts eax, 0
    mov cr0, eax
    ; Far jump into protected mode.
    jmp 0x08:prot_start

; Protected mode code (32-bit)
bits 32
prot_start:
    mov ax, 0x10
    mov ds, ax
    mov es, ax
    mov ss, ax
    ; Load CR3 from the trampoline field.
    mov eax, [cr3val]
    mov cr3, eax
    ; Enable PAE (set CR4.PAE).
    mov eax, cr4
    bts eax, 5
    mov cr4, eax
    ; enable long mode somewhere here?
    mov eax, cr0
    bts eax, 31
    mov cr0, eax
    ; Far jump into long mode.
    jmp 0x18:long_start

; Long mode code (64-bit)
bits 64
long_start:
    mov ax, 0x20
    mov ds, ax
    mov es, ax
    mov ss, ax
    ; Optionally, load GS base from kgsval if needed.
    ; Jump to the kernel entry point stored in kcode.
    mov rax, [kcode]
    jmp rax

trampoline_end:
