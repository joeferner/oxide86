; *** Trans Flag Bootloader (NASM) ***
; Required: CGA-Compatible Card with Composite Output or an emulator that supports this
; 
; copr. CardiganHill 2021 (admin@zazdravnaya.com)

;ORG 7c00h	;Uncomment this if bootloader
ORG 100h	;Uncomment this if you want an MS-DOS .com
BITS 16

; Enable CGA 640x200 mode
xor ah,ah
mov al,6
int 10h
; Flip on CGA colorburst
mov dx,3d8h
mov al,00011010b
out dx,al
; Color Select (Contrary to some info online, bit 5 does affect the monochrome mode)
inc dx
mov al,00101111b
out dx,al
; Prepare for stosw
cld
mov ax,0xB800
mov es,ax
mov di,2000h
; Draw top blue
mov ax,0x3333
call drawbar
; Draw top pink
mov ax,0xEEEE
call drawbar
; Draw white
mov ax,0xFFFF
call drawbar
; Draw bottom pink
mov ax,0xEEEE
call drawbar
; Draw bottom blue
mov ax,0x3333
call drawbar

; Wait for keypress
mov ah, 0x00
int 0x16

; Return to text mode
mov ah, 0x00
mov al, 0x03
int 0x10

; Exit
mov ah, 0x4C
mov al, 0x00
int 0x21
; Draw bar routine
drawbar:
	sub di,2000h
	mov cx,800
	rep stosw
	add di,19C0h
	mov cx,800
	rep stosw
	ret