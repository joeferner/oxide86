PUBLIC mode7, hgc_out
EXTRN wait_key:near, print_str:near, set_cursor:near
EXTRN findmono:near, find6845:near, load_file:near
EXTRN err_open_file_str:byte, err_read_file_str:byte
EXTRN mem_alloc_err_str:byte, mem_free_err_str:byte
EXTRN init_video_mode:byte, set_video_mode:byte
EXTRN file_handle:word, allocpara:word, mem_block_addr:word

MDAHGC_VBUF_SEG   EQU     0B000h

data	segment para public 'DATA'
; Attributes for MDA mode
mda_attr	db	07h, 0Fh, 01h, 09h, 70h, 87h, 8Fh, 81h, 89h, 0F0h
MDA_QUANTITY	EQU	($-MDA_ATTR) ; quantity of MDA attributes for showing

; Strings for MDA mode
normal_str	db "Normal Text                      ", 0Dh, 0Ah, 0
br_str		db "Bright Text                      ", 0Dh, 0Ah, 0
und_str		db "Underlined Text                  ", 0Dh, 0Ah, 0
und_br_str	db "Underlined Bright Text           ", 0Dh, 0Ah, 0
neg_str		db "Negative Text                    ", 0Dh, 0Ah, 0

bl_str		db "Blinking Text                    ", 0Dh, 0Ah, 0
bl_br_str	db "Blinking Bright Text             ", 0Dh, 0Ah, 0
bl_und_str	db "Blinking Underlined Text         ", 0Dh, 0Ah, 0
bl_und_br_str	db "Blinking Underlined Bright Text  ", 0Dh, 0Ah, 0
bl_neg_str	db "Blinking Negative Text           ", 0Dh, 0Ah, 0


; Hercules CRT params for 720x348
; each symbol consist of 16pixel and 4 rows of deflection
HGC_param       db      00h, 35h	; hor common 54 sym (864 pixel)
		db	01h, 2Dh	; hor view 45 sym (720 pixel)
		db	02h, 2Eh	; hor sync pos 46 sym (736 pixel)
		db	03h, 07h	; hor sync pulse with 7 sym (112 pixel)
		db	04h, 5Bh	; vert common 92 sym (368 rows)
		db	05h, 02h	; vert tune (2 rows)	
		db	06h, 57h	; vert view 87 sym (348 rows)
		db	07h, 57h	; vert sync position (after 87 sym row)
		db	09h, 03h	; max str per symbol (4)
HGC_len   EQU ($ - HGC_param)/2

no_hgc_str      db      "HGC adapter was not found!", 0Dh, 0Ah, 0 
mda_found_str   db      "Found MDA adapter, but HGC was not detected!", 0Dh, 0Ah, 0 
data	ends

code	segment	byte public 'CODE'
assume  cs:code, ds:data
;******************************MDA 7 mode test**********************************
; mode in AL
mode7 proc near
	mov	ah, 00h		; set mode function
	mov	set_video_mode, al
	int	10h

; show all symbols from char ROM
; write symbols to screen by BIOS functions

	mov	dh, 02		; cursor row
	mov	dl, 02		; cursor column
	call	set_cursor	; set cursor for symbol table
	mov	al, 00h		; starting symbol
        mov     bh, 00h         ; page 0
        mov     bl, 07h         ; attribute

write_mda_row:
	mov	cx, 16
@@:
	push	dx
	push	ax
	mov	ah, 09h
	int	10h	; - VIDEO - WRITE CHARACTER AND	ADVANCE	CURSOR (TTY WRITE)
			; AL = character, BH = display page
			; BL = foreground color	(graphics modes)
	pop	ax
	pop	dx

	inc	al
	inc	dl
	call	set_cursor	; set cursor for symbol table
	loop	@B

	inc	dh
	mov	dl, 02
	call	set_cursor	; set cursor for symbol table
	cmp	dh, (16+2)
	jb	write_mda_row

; hide cursor
	mov	dh, 25		; cursor row
	mov	dl, 00		; cursor column
	call	set_cursor
	call	wait_key

;---------------- show all attributes combinations for MDA -----------------
; set mode for clear screen
	mov	ah, 00h
	mov	al, set_video_mode
	int	10h

	mov	dh, 04		; cursor row
	mov	dl, 00h		; cursor column
	mov	bl, 00h		; attribute
	mov	cx, MDA_QUANTITY
	mov	si, offset mda_attr
	
bw_txt_loop:
	push	cx
	push	dx

	call	set_cursor
	mov	cx, 40		; 40 positions for string
        mov     bh, 00h         ; page 0
	mov	al, 00h		; empty symbol

	mov	bl, byte ptr ds:[si]
	inc	si
	mov	ah, 09h		; write str with attributes
	int	10h

	pop	dx
	pop	cx

	inc	dh		; inc row number
	loop	bw_txt_loop


	mov	dh, 04h		; cursor row
	mov	dl, 00h		; cursor column
	call	set_cursor

	mov	si, offset normal_str
	call	print_str
	mov	si, offset br_str
	call	print_str
	mov	si, offset und_str
	call	print_str
	mov	si, offset und_br_str
	call	print_str
	mov	si, offset neg_str
	call	print_str

	mov	si, offset bl_str
	call	print_str
	mov	si, offset bl_br_str
	call	print_str
	mov	si, offset bl_und_str
	call	print_str
	mov	si, offset bl_und_br_str
	call	print_str
	mov	si, offset bl_neg_str
	call	print_str

; hide cursor
	mov	dh, 25		; cursor row
	mov	dl, 00		; cursor column
	call	set_cursor
	call	wait_key

        mov     ah, 00h
	mov	al, init_video_mode	; return old video mode
	int	10h

	retn
mode7   endp

;***********************Write screen for Hercules card**********************
hgc_out proc near
        push    dx	; save file name str DS:DX
	call	findmono	; (CF == 1) -> no HGC, no MDA
        pop     dx
	jnc	@F
	mov	si, offset no_hgc_str
        call    print_str
        retn
@@:
	cmp	al, 01h		; ((AL == 01h) && (CF==0)) -> MDA found
	jne	@F
	mov	si, offset mda_found_str
        call    print_str
        retn
@@:
;------------------------------------------------------------------------
; load file into mem
        mov     allocpara, 4000
        call    load_file
        jnc     @F      ; exit, if CF was set
        retn
@@:

; set hercules controller's registers
if 1
	mov	dx, 3BFh	; set config
        mov     al, 00000001b   ; bit0-> enable graphics mode 
        out     dx, al          ; bit1-> delete second 32K from video buffer

	mov	dx, 3B8h
        xor     al, al          ; bit3-> disable video signal
	out	dx, al

; set HGC params
        mov     dx, 3B4h
        mov     si, offset HGC_param
        mov     cx, HGC_len
@@:
	lodsw
	out	dx, ax
	loop	@B

        mov     dx, 3B8h
        mov     al, 00001010b   ; bit1-> enable 720x348 graphics mode
        out     dx, al          ; bit3-> enable video signal
endif

; set ES to video buffer
        mov     ax, MDAHGC_VBUF_SEG
	mov	es, ax
; clear screen
	mov	cx, 16384
	mov	di, 0000h
	mov	ax, 0000h
rep	stos	word ptr es:[di]

; set DS to memory buffer
	push	ds
        mov     ax, mem_block_addr
        mov     ds, ax            ; DS -> seg of mem buf
	mov	bx, (165*100)	; vertical offset in bytes from end of image
	mov	dx, 05		; horizontal offset
hgc_00:
	call	hgc_bmp_view	; the bottom of image (ROWS * 100)
	call	wait_key
	cmp	ax, 5000h	; "DOWN ARROW" was pressed
	jne	@F
	add	bx, 100
	cmp	bx, ((600-348)*100)
	jbe	hgc_00
	mov	bx, ((600-348)*100)
	jmp	hgc_00
@@:
	cmp	ax, 4800h	; "UP ARROW" was pressed
	jne	@F
	sub	bx, 100
	jnc	hgc_00
	mov	bx, 00
	jmp	hgc_00
@@:
	cmp	ax, 5100h	; "PAGE DOWN" was pressed
	jne	@F
	add	bx, (100*20)
	cmp	bx, ((600-348)*100)
	jbe	hgc_00
	mov	bx, ((600-348)*100)
	jmp	hgc_00
@@:	
	cmp	ax, 4900h	; "PAGE UP" was pressed
	jne	@F
	sub	bx, (100*20)
	jnc	hgc_00
	mov	bx, 00
	jmp	hgc_00
@@:
	cmp	ax, 4F00h	; "END" was pressed
	jne	@F
	mov	bx, ((600-348)*100)
	jmp	hgc_00
@@:	
	cmp	ax, 4700h	; "HOME" was pressed
	jne	@F
	mov	bx, 00
	jmp	hgc_00
@@:
	cmp	ax, 4B00h	; "Left Arrow" was pressed
	jne	@F
	add	dx, 1
	cmp	dx, (100-90)
	jbe	hgc_00
	mov	dx, (100-90)
	jmp	hgc_00
@@:
	cmp	ax, 4D00h	; "Right Arrow" was pressed
	jne	@F
	sub	dx, 1
	jnc	hgc_00
	mov	dx, 00
	jmp	hgc_00
@@:
	pop	ds

        mov     ah, 00h
	mov	al, init_video_mode	; return old video mode
	int	10h

        mov     ax, mem_block_addr
	mov     es, ax
	mov     ah, 49h
	int     21h
	jnc     @F
	mov     si, offset mem_free_err_str
	call    print_str
@@:
	retn
hgc_out endp

;*******************Hercules BMP file preview subroutine**********************
;720x348	90 bytes row
;-------------------------- preview BMP file----------------------------------
; macros for filling image (one bit per pixel!) from
; BMP 800x600 file to HGC video segments
; Filling will be from ending rows to begining, since
; in the this BMP file rows was written from bottom to top!
; Don't forget CLD instruction before first use macros
; for vertical shift (100-90=10) we add before and after copying
; DL->before, after we calculate as DH=DL-10 and then NEG DH
WR_BMP_800X600 MACRO
        mov     cx, 87		; 87 rows per segment
@@:
        push    cx
; 90 bytes (720/8) for will be moved to Hercules screen buffer
; 10 byte ((800-720)/8) will be added before and after moving
	mov	cx, 90		; 90 bytes per row (720/8)
	add	si, dx
rep	movs	byte ptr es:[di], ds:[si]	; write one HGC row
	mov	ax, 10
	sub	ax, dx
	add	si, ax

; then	add (100*3) bytes (3 BMP rows), for next read from BMP fit to same HGC seg
        add si, ((100*3)+0)	; add 10 (100 per row bytes in BMP - 90 in HGC)
        pop     cx
	sub	di, (90*2)	; 90 bytes already was read + 90 for set to begin of row
        loop    @B
ENDM

; BMP file 0x0A -> dword pointer to offset pixel data

hgc_bmp_view proc near
; BMP image seg->DS
; HGC video RAM seg->ES

; vertical offset in bytes from the bottom of image (ROWS * 100) -> BX
; horizontal offset in bytes from vert border of file -> DX
; calculation from begin of BMP data
	cld
	mov	si, word ptr ds:[0Ah]
; write 3, 7, 11,... rows
        add     si, bx
        mov     di, (6000h+(86*90))
	WR_BMP_800X600

	mov	si, word ptr ds:[0Ah]
; write 2, 6, 10,... rows
	;        add     si, ((100*VERT_OFFSET)+100) 
	add	si, bx
	add	si, 100
        mov     di, (4000h + (86*90))
	WR_BMP_800X600

	mov	si, word ptr ds:[0Ah]
; write 1, 5, 9,... rows
	;        add     si, ((100*VERT_OFFSET)+(100*2)) 
	add	si, bx
	add	si, (100*2)
	mov     di, (2000h +(86*90))
	WR_BMP_800X600

	mov	si, word ptr ds:[0Ah]
; write 0, 4, 8,... rows
	;        add     si, ((100*VERT_OFFSET)+(100*3)) 
	add	si, bx
	add	si, (100*3)
        mov     di, (0000h + (86*90))
	WR_BMP_800X600

	retn
hgc_bmp_view endp

code    ends            ; end code segment
end
