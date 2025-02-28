extern STACK_TOP
extern BSPDONE
extern APPRUNNING
extern ap_startup

section .text.trampoline

;--- Begin 16-bit Code ---
bits 16
_start_trampoline:
ap_init:
    mov al, 0xAB          ; Marker 0xAB: GDT loaded in 16-bit mode
    call debug_write_16
    cli
    cld
    ; (No debug calls here – we’ll call debug routines after switching to 32-bit)
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
    ; Debug: GDT loaded
    mov al, 0x41          ; Marker 0x41: GDT loaded in 16-bit mode
    call debug_write_16
    mov eax, cr0
    or eax, 1
    mov cr0, eax
    ; Debug: Protected mode enabled
    mov al, 0xBB         ; Marker 0xBB: CR0 updated in 16-bit mode
    call debug_write_16
    db 0xEA              ; Far jump opcode
    dd _L8060
    dw 0x8

;--- End 16-bit Code; Begin 32-bit Code ---
align 32
bits 32
_L8060:
    mov ax, 16
    mov ds, ax
    mov ss, ax
    ; --- Debug marker: after setting DS/SS ---
    mov al, 0xCC        ; Marker 0xCC: DS/SS set
    call debug_write
    mov eax, 1
    cpuid
    shr ebx, 24
    mov edi, ebx
    shl ebx, 15
    mov esp, dword [STACK_TOP]
    sub esp, ebx
    push edi
    ; --- Debug marker: after setting up stack ---
    mov al, 0xEE        ; Marker 0xDD: stack set up
    call debug_write

ap_wait:
    pause
    cmp byte [BSPDONE], 0
    jz ap_wait
    lock inc byte [APPRUNNING]
    ; --- Debug marker: before jumping to ap_startup ---
    mov al, 0xEE        ; Marker 0xEE: about to jump to ap_startup
    call debug_write
    db 0xEA               ; Far jump opcode
    dd ap_startup
    dw 0x8

_end_trampoline:

; --- Debug output routines in 32-bit mode ---
; wait_serial: waits until COM1’s Transmitter Holding Register is empty.
wait_serial:
    in al, 0x3FD         ; Read LSR from COM1 (0x3F8 + 5)
    test al, 0x20        ; Check THRE (bit 5)
    jz wait_serial
    ret

; write_serial: writes AL to COM1.
write_serial:
    call wait_serial
    mov dx, 0x3F8        ; COM1 port
    out dx, al
    ret

; debug_write: preserves registers and writes the byte in AL.
debug_write:
    push eax
    call write_serial
    pop eax
    ret
bits 16

; 16-bit wait: wait until COM1’s THR is empty.
wait_serial_16:
    in   al, 0x3FD      ; Read LSR (0x3F8 + 5)
    test al, 0x20       ; Check if THR is empty (bit 5)
    jz   wait_serial_16
    ret

; 16-bit write: write AL to COM1.
write_serial_16:
    call wait_serial_16
    mov  dx, 0x3F8      ; COM1 port
    out  dx, al
    ret

; 16-bit debug write: preserve AX, write the marker in AL.
debug_write_16:
    push ax
    call write_serial_16
    pop ax
    ret
