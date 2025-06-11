; ap_trampoline.asm
; Self-contained trampoline designed to be loaded at physical address 0x8000.
; It reserves data fields that will be patched by the BSP:
;   cr3val:    4 bytes (offset 0)
;   kcode:     8 bytes (offset 4) – kernel entry pointer (64-bit)
;   kstack:    8 bytes (offset 12) – kernel stack pointer for this AP
;   kgsval:    8 bytes (offset 20) – GS base value
;   commword:  4 bytes (offset 28) – communication flag; AP sets to 1 when ready

section .rodata
    cr3val:    dq 0
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

;------------------------------------------------------------------------------ 
; GDT at physical 0x8000:
;   0x00: null
;   0x08: 32‑bit code
;   0x10: 32‑bit data
;   0x18: 64‑bit code (L=1)
;------------------------------------------------------------------------------ 
align 8
gdt:
    ; 0x00: null
    dq 0x0000000000000000

    ; 0x08: 32‑bit code @ physical 0x8000
    ;   limit=0xFFFFF, base=0x8000, access=0x9A, flags=0xC
    dq 0x00CF9A008000FFFF

    ; 0x10: 32‑bit data @ physical 0x8000
    ;   limit=0xFFFFF, base=0x8000, access=0x92, flags=0xC
    dq 0x00CF920008000FFFF

    ; 0x18: 64‑bit code (base is ignored in long mode)
    ;   limit=0xFFFFF, access=0x9A, L=1, G=1 → flags+limit = 0xAF
    dq 0x00AF9A000000FFFF

gdt_end:
gdt_ptr:
    dw gdt_end - gdt - 1
    dd gdt

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

    ; Load CR3
    mov eax, [cr3val]
    mov cr3, eax

    ; Enable PAE
    mov eax, cr4
    bts eax, 5
    mov cr4, eax

    ; Enable LME in EFER
    mov  ecx, 0xC0000080
    rdmsr
    or   eax, 1
    wrmsr

    ; Enable paging → enter long‑mode
    mov  eax, cr0
    bts  eax, 31
    mov  cr0, eax

    ; Far jump into your 64‑bit code segment (selector 0x18)
    jmp 0x18:long_start

; Long mode code (64-bit)
bits 64
long_start:
    mov ax, 0x10     ; Use the valid data segment selector from the GDT.
    mov ds, ax
    mov es, ax
    mov ss, ax
    ; Load the AP stack pointer from the trampoline’s kstack field.
    mov rax, [kstack]
    mov rsp, rax
    ; Jump to the kernel entry point stored in kcode.
    mov rax, [kcode]
    jmp rax


trampoline_end:
