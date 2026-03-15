;-------------------subroutines for CGA MDA HGC tests---------------------------

PUBLIC wait_key, print_str, set_cursor, write_mode4, write_6845, sync_fix
PUBLIC show_dx_ax, print_symbol, find6845, findmono, load_file
EXTRN  file_handle:word, allocpara:word, mem_block_addr:word
EXTRN  err_open_file_str:byte, err_read_file_str:byte
EXTRN  mem_alloc_err_str:byte, mem_free_err_str:byte

code	segment	byte public 'CODE'
assume	cs:code

; ллллллллллллллллллл wait for any key will be pressed лллллллллллллллллллллл
wait_key proc near
; first flush kbd buffer
; then wait when key will be presed
; AH->scan code, AL->ASCII

; clear kbd buffer
	push	es
	cli
	mov	ax, 40h
	mov	es, ax
	mov 	al, es:[1Ah]	; get pointer to head of buffer
	mov	es:[1Ch], al	; move it to pointer of tail 
	sti
	pop	es
	
	mov	ah, 00h		; function 1, get status
	int 	16h		; wait for key pressing

	retn
wait_key endp

; ллллллллллллллллллллллллллллл Print String лллллллллллллллллллллллллллллллл
print_str proc near
; str for print in ds:[si]
        push    ax
@@:
	lodsb
	cmp	al, 0
	je	@F		; ending zero was found
	push	si
        mov     bx, 0001h         ; page 0, color 1
	mov	ah, 0Eh
	int	10h		; - VIDEO - WRITE CHARACTER AND	ADVANCE	CURSOR (TTY WRITE)
				; AL = character, BH = display page (alpha modes)
				; BL = foreground color	(graphics modes)
	pop	si
	jmp	short @B
@@:
        pop     ax
	retn
print_str endp

; ллллллллллллллллллллллллллллл Set Cursor лллллллллллллллллллллллллллллллл
set_cursor proc near
; dh - row; dl - column
; AX needs to save for JUKO EGA CARD
	push	ax
	mov	ah, 2	; set cursor position; dh - row; dl - column
	mov	bh, 0	; page number
	int	10h
	pop	ax
	retn
set_cursor endp

;************************ Write Screen for mode 4 ****************************
; write all graphic screen in mode 4
; ES:SI - video data SEGMENT:OFFSET
; almost all registers will be destroyed!

write_mode4 proc near
        xor     bh, bh		; page0
	xor     dx, dx		; row 0
	xor     cx, cx		; column 0
        mov     ah, 0Ch         ; write point BIOS function (int 10h)

@@:
        mov     al, byte ptr es:[si]    ; write 4 points to AL
        inc     si                      ; addr of byte for next write

REPT 4
        rol     al, 1
        rol     al, 1
        mov     di, ax          ; save AX in DI (push ax)
        and     al, 00000011b
        int     10h		; write point BIOS function (AH = 0Ch)
        mov     ax, di          ; restore AX from DI (pop ax)
	inc     cx		; column addr
ENDM

        cmp     cx, 320
        jb	@B
	xor    cx, cx

        inc     dx              ; inc row addr
        cmp     dx, 200
        jb      @B

        retn
write_mode4 endp

;лллллллллллллллллллллллWrite to 6845 registerллллллллллллллллллллллллллллллл
; AL - reg number (index)
; AH - value to vrite
; DX -> 6845 index port , 3D4h for CGA; 3B4h for MDA/HGC
write_6845 proc near
        push    dx
	cli
	out	dx, al

        inc     dl              ; DX = 3D5h or 3B5h
	mov	al, ah		; value of reg to AL
	out	dx, al
	sti
        pop     dx
	retn
write_6845 endp

;лллллллллллллллллллллллллллsync fix for G3101ллллллллллллллллллллллллллллллл
sync_fix proc near
        mov     ah,12h          ; EGA installation check
        mov     bx,0FF10h       ; for supress sync problem after test on G3101
        int     10h
        retn
sync_fix endp

;ллллллллллллллллShow decimal value from DX:AX to screenллллллллллллллллл
; convert bin value in DX:AX to ASCII CODE and show them to cursor position
; max DX:AX value is 0009:FFFF
show_dx_ax proc near
        push    ax
        push    dx
        push    si

        mov     si, 10
       	div	si
        add     dx, '0'         ; convert remandier to ASCII
        mov     byte ptr CS:X01,  dl          
        or      ax, ax          ; all done?
        jz      show_1d

	xor	dx, dx
	div	si
        add     dx, '0'          ; convert remandier to ASCII
        mov     byte ptr CS:X10,  dl          
        or      ax, ax          ; all done?
        jz      show_2d

	xor	dx, dx
	div	si
        add     dx, '0'          ; convert remandier to ASCII
        mov     byte ptr CS:X100,  dl          
        or      ax, ax           ; all done?
        jz      show_3d

	xor	dx, dx
	div	si
        add     dx, '0'          ; convert remandier to ASCII
        mov     byte ptr CS:X1000,  dl          
        or      ax, ax          ; all done?
        jz      show_4d

	xor	dx, dx
	div	si
        add     dx, '0'          ; convert remandier to ASCII
        mov     byte ptr CS:X10000,  dl          
        or      ax, ax           ; all done?
        jz      show_5d

	xor	dx, dx
	div	si
        add     dx, '0'          ; convert remandier to ASCII
        mov     byte ptr CS:X100000,  dl          

; show values on screen in cursor position
        mov     al, CS:X100000
	call	print_symbol
show_5d:
        mov     al, CS:X10000
	call	print_symbol
show_4d:
        mov     al, CS:X1000
	call	print_symbol
show_3d:
        mov     al, CS:X100
	call	print_symbol
show_2d:
        mov     al, CS:X10
	call	print_symbol
show_1d:
        mov     al, CS:X01
	call	print_symbol

        pop     si
        pop     dx
        pop     ax
        retn

; ASCII digit code for show
X01             DB      00h
X10             DB      00h
X100            DB      00h
X1000           DB      00h
X10000          DB      00h
X100000         DB      00h

show_dx_ax endp

; лллллллллллллллллллллллллллPrint Symbol in ALллллллллллллллллллллллллллллл
;		S u b r	o u t i	n e
print_symbol	proc near
	push	dx
	push	bx
	mov	ah, 0Eh

        mov     bx, 01h

	int	10h		; - VIDEO - WRITE CHARACTER AND	ADVANCE	CURSOR (TTY WRITE)
				; AL = character, BH = display page (alpha modes)

	pop	bx		; BL = foreground color	(graphics modes)
	pop	dx
	retn	
print_symbol	endp

; лллллллллллллллллллллллллллFind MDA/HGC adapterлллллллллллллллллллллллллллллл
findmono proc near
; return values
; if MDA/HGC adapter doesn't found, CF will be set
; if MDA adapter was found,     AL -> 01h
; if HGC adapter was found,     AL -> 00h
; if HGC+ adapter was found,    AL -> 10h
; if InColor adapter was found, AL -> 50h
	
	mov	dx, 3B4h	; DX address of video controller port
	call	find6845
	jc	exit_findmono
	
	mov	dx, 3BAh	; DX -> status port
	in	al, dx
	and	al, 80h
	mov	ah, al		; AH = AL (vert sync in HGC)

	mov	cx, 8000h	; repeat 8000h
@@:
	in	al, dx
	and	al, 80h		; select bit7
	cmp	ah, al
	loope	@B		; wait, for bit 7 will be changed

	jne	@F		; if bit7 was changed, we have HGC adapter	
	mov	al, 01h		; if bit7 was not chabged, we have MDA
	jmp	short exit_findmono

@@:
	in	al, dx		; read status port
	and	al, 01110000b

exit_findmono:
	retn
findmono endp

; лллллллллллллллллллллллллллFind 6845 chipлллллллллллллллллллллллллллллл
; DX -> address of port
; CF set, if no 6845 chip
find6845 proc near
	mov	al, 0Fh		; reg low byte of cursor
	out	dx, al
	inc	dx
	in	al, dx
	mov	ah, al		; save reg value in AH
	mov	al, 55h		; AL -> any val
	out	dx, al		; try to write

	mov	cx, 100h	; wait 6845 reaction
@@:
	loop	@B

	in	al, dx
	xchg	ah, al		; AH -> return val, AL -> initial val
	out	dx, al		; restore initial val
	cmp	ah, 55h
	je	@F
	stc			; set CF, if no 6845
@@:
	retn
find6845 endp

; лллллллллллллллллллллллллллLoad file into memлллллллллллллллллллллллллллллл
; load file into memory
; The global variable are used by subroutine:
; file_handle ->  handle of file with image
; allocpara -> number of mem paragraph
; mem_block_addr -> SEG of mem block
; Global text strings:
; err_open_file_str
; err_read_file
; mem_alloc_err_str

; Input variable:
; DS:DX->ASCIIZ string with the file name for show
; if error will be happened, subroutine will print message(s), set CF and retn

load_file proc near
; Open file as read-only
	mov	ax, 3D00h	; open file as Read Only, DS:DX->filename
        push    dx
        int	21h
        pop     dx
	jnc	@F
        mov     si, dx          ; print file name in error str
        call    print_str
	mov	si, offset err_open_file_str
        call    print_str
        stc
        retn

;------------------------dynamic memory allocation----------------------------
@@:
        mov     file_handle, ax          ; save file handle
	mov	ah, 48h
        mov     bx, allocpara
        push    dx
        int     21h             ; return mem block in AX:0000
        pop     dx
	jnc	@F
	mov	si, offset mem_alloc_err_str
        call    print_str
        mov     bx, file_handle
        mov     ah, 3Eh         ; close file, its handle in BX
	int	21h
        stc
        retn

;-----------------------------------------------------------------------
@@:
; load file with image into memory
	push    ds	; save DS
	push    dx	; save pointer to file name
        mov     mem_block_addr, ax  ; save addr of mem block from AX
        mov     bx, file_handle	    ; copy file handle from file_handle to BX
        mov     ax, allocpara
        mov     cx, 16
        mul     cx
        mov     cx, ax  ; CX->(allocpara*16) number of bytes for copy from file
        mov     ax, mem_block_addr
	mov	ds, ax
        mov     dx, 0000h	; DS:DX -> mem_block_addr:0000 buffer in mem
	mov	ah, 3Fh
	int	21h
        pop     dx	; restore pointer to file name
        pop     ds	; restore DS

	pushf			; save CF flag
	mov     bx, file_handle
	mov     ah, 3Eh         ; close file, its handle in BX
	int	21h
 	popf			; restore CF flag from previous function
	jc	@F
	retn			; exit without error

@@:
        mov     si, dx  ; print file name
        call    print_str
	mov	si, offset err_read_file_str
        call    print_str

        mov     ax, mem_block_addr
 	mov     es, ax
        mov     ah, 49h		; free memory
	int     21h
        jnc     @F
	mov     si, offset mem_free_err_str
	call    print_str
@@:
        stc	; exit with error
        retn
load_file endp

code    ends            ; end code segment
end
