; CGA video card modes test program
; compiled with MASM 5.10 assembler (before 4.20 ver - MASM6.00)

;0.96 patch for JUKO EGA card. The BIOS of this cards
	; destroy AX after call int10,2 (cursor positioning)
;1.00 first try for direct 6845 controlling
;1.05 first implementation direct 6845 controlling mode 4 and 5
;1.15 first implementation direct 6845 controlling mode 6
;1.20 add switching foreground colors into mode 6
;1.30 try to add composite test for NTSC out
;1.31 first more or less working variant for NTSC out
;1.32 add additiobal colors for NTSC out
;1.35 add additional test mode 4 for NTSC out
;2.00 new conception for NTSC test screen writing
;2.05 new procedure for NTSC write screen , mode 6
;2.10 new procedure for NTSC write screen , mode 5
;2.11 new help text
;2.30 first try for new code of write screen (int 10h)
;2.40 a lot of bag fix
;2.50 bag fix
;2.60 write NTSC composite screen function code optimisation
;2.70 new sequense in NTSC test, combined int10h and direct I/O in mode4/5 test
;2.71 combined int10h and direct I/O in mode6 test
;2.75 new help tekst
;2.80 new text strings for mode 6
;2.85 new help text, bug fix
;2.86 first try to add 160 x 100 mode
;2.87 delete foreground color swithing in mode6 due the in10h
;			foreground color set bug
;2.88 try to workaround foreground/background colors assign,
;	 but fail, it is unreliably
;2.89 optimising graphic's procedures
;2.90 code optimising, help text correctons
;2.92 lines quantity in the mode6 test were increased to 10
;2.94 text strings in mode 4 were added
;2.95 text strings in mode 4 were added
;3.00 unused code was deleted, bug fix in the color text rows show code
;3.10 correct the color text rows show code, hide cursor in mode 1,2,3,4
;3.15 try 2 make the 160x100 mode working
;3.20 try 3 make the 160x100 mode working
;3.25 try 4 make the 160x100 mode working
;3.30 update help text and fix serious bug with DS segment assign in 160x100 code
;3.35 minor changes
;3.40 cursor was hidden in text modes
;3.50 fix sync problem with G3101; add experimental mode 7, minor changes
;3.60 fix bug with JUKO EGA cards
;3.70 experimental mode 7 was deleted while
;3.80 addreses of 6845 regs have been obtained from BDA
;3.90 first try to switch 640x200 composite NTSC out to direct mode switch
;3.95 sync writing color/mode CGA regs to BDA in direct I/O modes
;4.00 add black color as background to tests 4 and 5
;4.01 minor changes, new help string
;4.05 corrections in the mode 1,2,3 and 4 routine
;4.10 sync writing color/mode CGA regs in the NTSC mode 6 test was deleted
;4.20 fix bug in cmdloop, compatibility with MASM 5.1 compiler
;4.30 first try to add MDA test
;4.40 second try to add MDA test
;4.50 first work implementation of MDA test
;4.60 subroutines have been moved to subr.asm
;4.70 a lot code optimization
;4.80 fix bugs in cmd parcer
;4.90 first try to add simple benchmark to NTSC test	
;5.00 second try to add benchmark, max val in ms = 655359ms, ~10.9 min
;5.10 minor correction bencmark code
;5.20 enable color on NTSC out during TV test mode6
;5.30 new text strings about modes in the NTSC out test
;5.40 first try to add Hercules test
;5.50 second try of HGC test. MDA and HGC tests were moved to mdahgc.asm file.
;		 All tests currently declared as subroutunes
;5.51 fix new bug in cmd parser, delete unused declarations
;5.55 third try of HGC test
;5.60 forth  try of HGC test, minor bugs have been fixed
;5.70 more or less normal variant of HGC test
;5.80 HGC test optimisation, bugs fix, minor changes
;5.90 add MDA/HGC card detection
;5.95 bug fix, minor changes
;6.00 hard 6845 port set for 160x100 CGA test,
;	more optimal proc for 160x100 mode, minor changes
;6.05 minor changes, more detailed commments
;6.10 first variant with one image segment and load test's images from files
;6.20 switching to initial video mode has been moved
;	 to every test procedure, minor changes
;6.30 first version with dynamic memory allocation for image buffer
;6.40 bug fix, minor changes
;6.50 first try to add cards switching,
;        add protection for running in the DOS 1.X
;6.55 new keys for cards switching and code optimizing
;6.60 update help topic, first try to add switching to EGA card
;6.70 macros for loading file to memory, a lot of bug fix
;6.80 Video HW test was added
;6.90 add EGA RAM size detection, fixed bug in HW test
;7.00 modify help text, new mode set cmd
;7.00LF new load_file subroutine instead macros, bug fix
;7.05LF new wait_key code, first implementation for moving image in H test
;7.10LF addings limits for vertical moving image in H test
;7.15LF	first variant with load image from file name that was written in CMD
;7.20LF second variant with load image from file, bugs fix
;7.25LF first variant with horizontal moving image in H test
;7.30LF	serious bugs were fixed
;7.35LF minor changes
;7.40LF new code for realloc mem, fixed serious bug with release mem
;7.50R  switching (conditional compiling) between strategies of reallocation
        ; memory in code and during linking (link /CP:1), minor changes
;7.55R new text strings in mode5 test, green/HI foreground screens have been
        ; added to mode6 test, help text has been added



PUBLIC indx_6845, set_video_mode, init_video_mode, file_handle
PUBLIC err_open_file_str, err_read_file_str
PUBLIC mem_alloc_err_str, mem_free_err_str, mem_block_addr, allocpara 

EXTRN   mode7:near, hgc_out:near, load_file:near
EXTRN	wait_key:near, print_str:near, set_cursor:near
EXTRN	write_mode4:near, write_6845:near, sync_fix:near
EXTRN   findmono:near, find6845:near
EXTRN   show_dx_ax:near

CGA_BLACK	EQU	00h
CGA_WHITE	EQU	01h
CGA_VBUF_SEG    EQU     0B800h

EGA	EQU	01h
CGA	EQU	02h
MONO	EQU	03h

;MODE_6845_BDA	EQU	65h ; addr of value in the 6845 mode control reg in BDA
COLOR_6845_BDA	EQU	66h ; addr of value in the 6845 color select reg in BDA

MEM_REALLOC     EQU     0
;------------------------------------------------------------------------
stack   segment para stack 'STACK'
        db 200h dup ('?')
stack  ends

;------------------------------------------------------------------------
data	segment para public 'DATA'
init_video_mode	db	00h
set_video_mode	db	00h

indx_6845       dw      03D4h	; addr of 6845 index reg
mode_6845	dw      03D8h	; addr of 6845 mode control reg
color_6845	dw	03D9h	; addr of 6845 color select reg
status_6845	dw	03DAh	; addr of 6845 status reg

usage_str db " CGA/MDA/HGC card modes test v7.55R, (c) 2023 Emil Mammadov", 0Dh, 0Ah
        db "    usage: CT [mode|C|L|H|SM|SC(mode)|SE(mode)|T]", 0Dh, 0Ah
        db "    mode - CGA/MDA BIOS video mode (0-7)", 0Dh, 0Ah
        db " C - composite NTSC out test, mode switching sequence:", 0Dh, 0Ah
        db "    mode4 palette 0; mode4 palette 0 HI;", 0Dh, 0Ah
        db "    mode4 palette 1; mode4 palette 1 HI;", 0Dh, 0Ah
        db "    mode6 foreground white; mode6 foreground white HI", 0Dh, 0Ah
        db " MODE4/5 tests, color switching sequence:", 0Dh, 0Ah
        db "    background black/palette 0; background blue/palette 0;", 0Dh, 0Ah
        db "    background blue HI/palette 0; background blue HI/palette 0 HI;", 0Dh, 0Ah
        db "    background black/palette 1; background blue/palette 1;", 0Dh, 0Ah
        db "    background blue HI/palette 1; background blue HI/palette 1 HI;", 0Dh, 0Ah
        db "    then same, but with direct write instead int10h routine", 0Dh, 0Ah
        db " MODE6 test, color switching sequence:", 0Dh, 0Ah
        db "    foreground white; foreground green; foreground green HI", 0Dh, 0Ah
        db "    then same, but with direct write instead int10h routine", 0Dh, 0Ah
        db " L - non-standard CGA color 160 x 100 mode test", 0Dh, 0Ah
        db " H [filename]- hercules card test", 0Dh, 0Ah
        db "    moving image - Arrow U/D; Arrow L/R; Page U/D; Home/End", 0Dh, 0Ah
	db " SM  - switch to MDA/HGC card", 0Dh, 0Ah
        db " SCmode - switch to CGA card; mode (0 - 3)", 0Dh, 0Ah
        db " SEmode - switch to EGA card; mode (0 - 3)", 0Dh, 0Ah
        db " T - test video hardware", 0Dh, 0Ah, '$'

error_dosver_str	db " DOS version must be 2.00 or above!", 0Dh, 0Ah, '$'

mode6_s			db "CGA mode 6 B/W                       ", 0
mode6_green_s		db "CGA mode 6, Foreground Color Green   ", 0
mode6_green_hi_s	db "CGA mode 6, Foreground Color Green HI", 0
mode6_dw_s              db "CGA mode 6 B/W                         Direct write", 0
mode6_green_dw_s        db "CGA mode 6, Foreground Color Green     Direct write", 0
mode6_green_dw_hi_s     db "CGA mode 6, Foreground Color Green HI  Direct write", 0

; CGA 160x100 mode params
C160X100_param  db      00,  113  ; horizontal total
                db      01,  80   ; horizontal displayed
                db      02,  90   ; horizontal sync pos
                db      03,  10   ; horizontal sync with
                db      04, 127   ; vertical total *127 max, else no hor sync
                db      05,  06   ; vertical total adjust 06
                db      06, 100   ; vertical displayed 25 100
                db      07, 112   ; vertical sync pos 28  135
                db      08,  02   ; interlace mode
                db      09,  01   ; max scan line 07
                db      0Ah, 00100110b  ; cursor start 06 and hide cursor (bit5)
                db      0Bh, 07   ; cursor ebd
                db      0Ch, 00   ; start addr H 
                db      0Dh,  00   ; start addr L
C160X100_len    EQU ($ - C160X100_param)/2

file_handle	dw	0000h
c160100d_file_s	db	"C160100D.IMG", 0
h_bmp_file_s	db      "800600BW.BMP", 0

err_open_file_str       db      " file open error!", 0Dh, 0Ah, 0
err_read_file_str       db      " file read error!", 0Dh, 0Ah, 0

allocpara               dw      0000h
mem_block_addr          dw      0000h   ; addr of mem block address in para
mem_alloc_err_str       db      "Memory allocation error!", 0Dh, 0Ah, 0
mem_free_err_str        db      "Memory freeing error!", 0Dh, 0Ah, 0
if MEM_REALLOC
mem_realloc_err_str	db	"Memory reallocation error!", 0Dh, 0Ah, 0
endif

no_mono_str	db	"MDA adapter was not found!", 0Dh, 0Ah, 0
no_cga_str	db	"CGA adapter was not found!", 0Dh, 0Ah, 0
no_ega_str	db	"EGA adapter was not found!", 0Dh, 0Ah, 0
data	ends

;---------------------------------------------------------------------------
code	segment	byte public 'CODE'
if MEM_REALLOC
assume cs:code, ds:data, es:zseg, ss:stack
else
assume cs:code, ds:data, es:nothing, ss:stack
endif

main    proc    far
;---------------------------------------------------------------------------
; in this section, for checking DOS version we use old stile
; functions, for compatibility with DOS 1.X
	mov	ah, 30h		; get DOS version in AL
	int	21h
	cmp	al, 02
	jae @F			; if DOS version above or equal 2.00, work
	push	ds		; move to stack the para of PSP
	mov	ax, 0		; for return to DOS by RETF
	push	ax		; move to stack zero offset of address
	mov	ax, data	; init ds, for print string function
	mov	ds, ax
	mov	dx, offset error_dosver_str
	mov	ah, 09h		; print error string
	int	21h
	retf			; old style return to DOS

;---------------------------------------------------------------------------
@@:
	mov	ax, data
	mov	ds, ax

if MEM_REALLOC 
; release unused memory
        mov     bx, SEG zseg	; get number ending para of program + 1
	mov	ax, es		; get number  begining para of program
	sub	bx, ax		; calc size of program in para
	mov	ah, 4Ah		; realloc memory
	int	21h
        jnc     @F
	mov     si, offset mem_realloc_err_str
	call    print_str
	jmp	exit
@@:
endif

; get initial video mode
	mov	ah, 0Fh
	int	10h
    	mov	init_video_mode, al	; save current video mode

	push	es
; get base port address for active 6845 CRT controller
	mov	ax, 0040h
	mov	es, ax
	mov	ax, word ptr es:[63h]
	mov	indx_6845, ax

	add	ax, 4
	mov	mode_6845, ax	; addr of 6845 mode control reg

	inc	ax
	mov	color_6845, ax	; addr of 6845 color select reg

	inc	ax
	mov	status_6845, ax	; addr of 6845 status reg
	pop	es

;****************** get command line argument ********************************
	mov	al, byte ptr es:[80h]
	or	al, al		; if length of string is equal zero,
				; show usage and exit
	jnz	@F
	jmp	show_usage
@@:
	mov	cl, al		; mov length of string to cx
	mov	ch, 00h

	mov	si, 81h		; get addr of first letter to cx
	cld
cmdloop:
	lods	byte ptr es:[si]
	cmp	al, ' '		; if space or TAB (09h), loop
	je	@F
	cmp	al, 09h		; 09h - TAB code
@@:
	loope	cmdloop

	cmp	al, 0Dh		; if 0Dh exit
	jne	@F
	jmp	show_usage
@@:
	cmp	al, 0		; if 0 exit
	jne	@F
	jmp	show_usage
@@:
	or	al, 20h		; convert symbol to lower case

	cmp	al, '6'
	jne	@F
	and	al, 0Fh		; get mode number (0-F) in AL
	call	mode6
        jmp     exit
@@:
	cmp	al, '7'
	jne	@F
	and	al, 0Fh		; get mode number (0-F) in AL
	call	mode7
        jmp     exit
@@:
	cmp	al, 'l'
	jne	@F
        lea     dx, c160100d_file_s
        call    c160_100         ; 160 x 100 color mode test
        jmp     exit
@@:
	cmp	al, 'h'
	jne	@F
        call    get_cmd_fname   ; get file name from cmd str to h_bmp_file_s
        lea     dx, h_bmp_file_s
        call    hgc_out				; Hercules test
        jmp     exit
@@:
	cmp	al, 's'
	jne	show_usage
; take next symbol
	lods	byte ptr es:[si]
	or	al, 20h		; convert new symbol to lower case
	cmp	al, 'm'
	jne	@F
        call    mda_sw         ; switch to MDA/HGC
        jmp     exit
@@:         
        cmp     al, 'e'
	jne	@F
        lods    byte ptr es:[si] ; load new symbol
        and     al, 0Fh          ; convert to digit
        cmp     al, 3          ; check new symbol, if 0-1, set 40x25, 2-3 80x25
        ja      show_usage
        call    ega_sw          ; switch to EGA mode1-3
	jmp	exit

@@:
	cmp	al, 'c'
	jne	show_usage
	lods	byte ptr es:[si] ; load new symbol
        and     al, 0Fh          ; convert to digit
        cmp     al, 3          ; check new symbol, if 0-1, set 40x25, 2-3 80x25
        ja      show_usage
        call    cga_sw          ; switch to CGA mode1-3
	jmp	exit

; if valid modes were not found, then to print usage and exit
show_usage:
	mov	ah, 09h		; show message "usage"
	mov	dx, offset usage_str
	int	21h
	jmp	exit


;*********************Video cards switching subroutines*********************
mda_sw proc near
	call	findmono	; (CF == 1) -> no HGC, no MDA
	jnc	@F
        mov     si, offset no_mono_str
        call    print_str
        retn

@@:
        mov     bx, 40h
        mov     es, bx	; set EX to BDA
	or	byte ptr es:[10h], 00110000b	; set bit 5 and 4

        mov     ax, 0007h
        int     10h		; set mode 7
        retn
mda_sw endp

;----------------------------------------------------------------------------
cga_sw proc near
; new screen mode in AL, 0-1 for 40x25, 2-3 for 80x25
        mov     dx, 3D4h        ; DX address of video controller port
        push    ax              ; save AL
	call	find6845
        pop     ax              ; restore AL
	jnc	@F
        mov     si, offset no_cga_str
        call    print_str
        retn

@@:
        mov     bx, 40h
        mov     es, bx  ; set BX to BDA
        and	byte ptr es:[10h], 11001111b	; reset bit 4 and 5
        cmp     al, 01h  
        ja      @F
	or	byte ptr es:[10h], 00010000b	; set bit 4 (40x25)
	jmp	short set_cga_modesw
@@:
	or	byte ptr es:[10h], 00100000b	; set bit 5 (80x25)
  
set_cga_modesw:
        mov     ah, 00h                ; set mode, in AL->mode
        int     10h
        retn
cga_sw endp

;-------------------------------------------------------------------------
ega_sw proc near
; new screen mode in AL, 0-1 for 40x25, 2-3 for 80x25
        mov     bx, 40h
        mov     es, bx          ; set ES to BDA
        cmp     byte ptr es:[87h], 00h  ; if zero, no EGA
	jne	@F
        mov     si, offset no_ega_str
        call    print_str
        retn

@@:
        and     byte ptr es:[10h], 11001111b    ; reset bit4 and 5 (EGA)
        mov     ah, 00h         ; set mode, in AL->mode
        int     10h
        retn
ega_sw endp

;******************Non-standard 160 x 100 16 color mode*********************
c160_100 proc near
; CGA mode 3	(80 x 25, 16 color)
; since this mode is not supported by BIOS, we use direct operations
; and also use hard settings the addresses of 6845 I/O ports

; load file into mem
        mov     allocpara, 1000
        call    load_file
        jnc     @F      ; exit, if CF was set
        retn

@@:
        mov     dx, 3D9h        ; CGA color select reg
        mov     al, 00000000b
        out     dx, al

        mov     dx, 3D8h                ; mode control reg of 6845
        mov     al, 00000001b           ; disable video signal
	out	dx, al

; 6845 controller init
; AL - reg number (index)
; AH - value to vrite
; DX -> 6845 index port , 3D4h
        mov     dx, 3D4h        ; 6845 index port
        mov     si, offset C160X100_param
        mov     cx, C160X100_len
@@:
	lodsw
        call    write_6845
        loop    @B

        mov     dx, 3D8h        ; mode control reg of 6845
        mov     al, 00001001b	; enable video signal
	out	dx, al

        mov     ax, CGA_VBUF_SEG
	mov	es, ax
        mov     di, 0000h       ; zero offset in seg

        push    ds
        mov     ax, mem_block_addr
	mov	ds, ax
	cld
        mov     cx, 8000
        mov     si, 0000h       ; offset of image data
rep     movs    word ptr es:[di], word ptr ds:[si]
        pop     ds
	call	wait_key

        mov     ah, 00h
	mov	al, init_video_mode	; return old video mode
	int	10h
        call    sync_fix        ; ??? for G3101

	mov     ax, mem_block_addr
	mov     es, ax
        mov     ah, 49h
        int     21h
        jnc     @F
        mov     si, offset mem_free_err_str
        call    print_str
@@:
	retn
c160_100 endp

;**********************Write screen for CGA mode 06***************************
; mode in AL
mode6 proc near
	mov	ah, 00h		; set mode function
	mov	set_video_mode, al
	int	10h

	mov	ah, 0Ch		; write point function
	mov	al, CGA_WHITE
	mov	cx, 00h		; column
	mov	dx, 00h		; row

mode6_loop:
 	push	ax
	int	10h
	pop	ax
	inc	cx
        cmp     cx, 640
	jne	mode6_loop
	mov	cx, 00h
	inc	dx

	cmp	dx, 20
	jne	@F
	mov	dx, 40
@@:
	cmp	dx, 60
	jne	@F
	mov	dx, 80
@@:
	cmp	dx, 100
	jne	@F
	mov	dx, 120
@@:
	cmp	dx, 140
	jne	@F
	mov	dx, 160
@@:
	cmp	dx, 180
	jb	mode6_loop

; print string
        mov     dx, 0000h
	call	set_cursor
        mov     si, offset mode6_s
        call    print_str
	call	wait_key

	mov     bh, 00h
	mov     bl, 00000010b   ; set green as forefround color
	mov	ah, 0Bh
	int     10h
; print string
	mov     dx, 0000h
	call    set_cursor
	mov     si, offset mode6_green_s
	call    print_str
	call    wait_key

	mov     bh, 00h
	mov     bl, 00001010b   ; set geen HI as foreground color
	mov	ah, 0Bh
	int     10h
; print string
	mov     dx, 0000h
	call    set_cursor
	mov     si, offset mode6_green_hi_s
	call    print_str
	call	wait_key

;----------------same mode6 test, but with direct I/O-----------------------
; 640x200
; bit0 set blue  foreground color
; bit1 set green foreground color
; bit2 set red   foreground color
; bit3 set intensity for foreground's colors
; bit4 set intensity colors
; bit5 - set palette  (N/A in this mode)
; bit6 - not used
; bit7 - not used

; set white HI as foreground color
	mov	ax, 0040h
	mov	es, ax			; ES > BDA segment
	mov	dx, color_6845		; 3d9h
	mov	al, 00001111b
	out	dx, al
	mov	byte ptr ES:[COLOR_6845_BDA], al

 ; write point
        mov     ax, CGA_VBUF_SEG
        mov     es, ax
        mov     al, 0ffh	; init color 0
        mov     si, 00000h	; mem addr for even rows
	mov	di, 02000h	; mem addr for odd rows

mode6d_loop:
        mov     byte ptr es:[si], al
	mov     byte ptr es:[di], al
        inc     si
	inc	di
        cmp     si, 800          ; 100str / 10 = 10
	jne	@F		 ; 10*640/8=800
	xor	al, 0ffh
@@:
        cmp     si, 1600
	jne	@F
	xor	al, 0ffh
@@:
	cmp	si, 2400
	jne	@F
	xor	al, 0ffh
@@:
	cmp	si, 3200
	jne	@F
	xor	al, 0ffh
@@:
	cmp	si, 4000
	jne	@F
	xor	al, 0ffh
@@:
	cmp	si, 4800
	jne	@F
	xor	al, 0ffh
@@:
	cmp	si, 5600
	jne	@F
	xor	al, 0ffh
@@:
	cmp	si, 6400
	jne	@F
	xor	al, 0ffh
@@:
	cmp	si, 7200
	jne	@F
	xor	al, 0ffh
@@:
	cmp	si, 8000
	jb	mode6d_loop

; print string
        mov     dx, 0000h
	call	set_cursor
        mov     si, offset mode6_dw_s
        call    print_str
	call	wait_key

; set green as foreground color
	mov	dx, color_6845
	mov     al, 00000010b
	out	dx, al
; print string
	mov     dx, 0000h
	call	set_cursor
	mov	si, offset mode6_green_dw_s
	call	print_str
	call	wait_key

; set green HI as foreground color
	mov	dx, color_6845
	mov     al, 00001010b
	out	dx, al
; print string
	mov     dx, 0000h
	call    set_cursor
	mov     si, offset mode6_green_dw_hi_s
	call    print_str
	call	wait_key


        mov     ah, 00h
	mov	al, init_video_mode	; return old video mode
	int	10h
        call    sync_fix        ; ??? for G3101

	retn
mode6 endp

;-----------------------------------------------------------------------
exit:
        mov     ax, 4C00h
	int	21h
main	endp

;********************get file name from cmd string*********************
get_cmd_fname proc near
; cmd symbols in es:si
        inc     cx	; add 1 to cmd string lenght for correct parsing
			; if only SPACE/TAB in CMD string, like "CT H "
gf_loop:
        lods    byte ptr es:[si]	; load new symbol
	cmp	al, ' '			; if space or TAB (09h), loop
	je	@F
	cmp	al, 09h		; 09h - TAB code
@@:
        loope   gf_loop

	cmp	al, 0Dh		; if 0Dh exit
	jne	@F
	retn
@@:
	cmp	al, 0		; if 0 exit
	jne	@F
	retn

@@:
	push	es
	push	ds

	push	es
	pop	ds		; DS ->PSP
	mov	bx, data
	mov	es, bx		; ES->DATA

	dec	si
	cmp	cx, 12
	jbe	@F
	mov	cx, 12		; max (8 + dot + 3)
@@:
        lea     di, h_bmp_file_s
gf_loop2:	
	lods	byte ptr ds:[si]
	cmp	al, ' '		; if space or TAB (09h), write 0 to end of file name
	je	@F
	cmp	al, 09h		; 09h - TAB code
	je	@F
	cmp	al, 0Dh		; if 0Dh exit
	je	@F
	stos	byte ptr es:[di]
	loop	gf_loop2
@@:
	mov	byte ptr es:[di], 0

gf_exit:
	pop	ds
	pop	es
	retn
get_cmd_fname endp
code	ends

if MEM_REALLOC
;--------------------------------------------------------------------------
zseg	segment para 'zseg'
zseg    ends

;--------------------------------------------------------------------------
endif

end	main		;end program
