; VGA Graphics Mode 0x13 Test
; 320x200, 256 Colors (linear framebuffer at A000:0000)
; 1 byte per pixel, offset = y * 320 + x
; Displays all 256 colors as a 16x16 grid of color blocks
;
; Palette layout (hardcoded DAC table):
;   0-15:   Standard EGA/VGA 16 colors
;   16-231: 6x6x6 RGB cube (xterm-256color compatible), 36 per R-level
;           R levels: 0,15,26,36,46,57 (in 6-bit 0-63)
;   232-255: 24 grayscale shades

[CPU 8086]
org 0x100

start:
    ; Switch to VGA mode 0x13 (320x200, 256 colors)
    mov ah, 0x00
    mov al, 0x13
    int 0x10

    ; --- Program all 256 DAC entries from hardcoded table ---
    ; Each entry is 3 bytes: R, G, B in 6-bit format (0-63)
    mov dx, 0x3C8
    xor al, al          ; start at DAC index 0
    out dx, al
    mov dx, 0x3C9
    mov si, dac_table
    mov cx, 768         ; 256 colors x 3 bytes
.dac_loop:
    lodsb
    out dx, al
    loop .dac_loop

    ; Set up video segment for direct memory access
    mov ax, 0xA000
    mov es, ax

    ; --- Draw 16x16 grid of color blocks ---
    ; Each block is 20 pixels wide and 12 pixels tall
    ; Color index N at column (N & 0x0F), row (N >> 4)
    xor bx, bx          ; bx = color index (0..255)
.block_outer:
    mov ax, bx
    and al, 0x0F
    mov [block_col], al
    mov ax, bx
    mov cl, 4
    shr ax, cl
    mov [block_row], al

    xor ax, ax
    mov al, [block_col]
    mov cx, 20
    mul cx
    mov [start_x], ax

    xor ax, ax
    mov al, [block_row]
    mov cx, 12
    mul cx
    mov [start_y], ax

    mov dx, [start_y]
    mov cx, 12
.draw_row:
    push cx
    push dx
    mov ax, dx
    mov cx, 320
    mul cx
    add ax, [start_x]
    mov di, ax
    pop dx

    mov cx, 20
    mov al, bl
.draw_pixel:
    stosb
    loop .draw_pixel

    inc dx
    pop cx
    loop .draw_row

    inc bx
    cmp bx, 256
    jb .block_outer

    ; Wait for keypress
    mov ah, 0x00
    int 0x16

    ; Return to text mode 0x03
    mov ah, 0x00
    mov al, 0x03
    int 0x10

    ; Exit
    mov ah, 0x4C
    int 0x21

; Variables
block_col db 0
block_row db 0
start_x   dw 0
start_y   dw 0

; Hardcoded VGA DAC palette table: 256 entries x 3 bytes (R, G, B), 6-bit (0-63)
dac_table:
    ; --- 0-15: Standard EGA colors ---
    db  0,  0,  0   ; 0  Black
    db  0,  0, 42   ; 1  Dark Blue
    db  0, 42,  0   ; 2  Dark Green
    db  0, 42, 42   ; 3  Dark Cyan
    db 42,  0,  0   ; 4  Dark Red
    db 42,  0, 42   ; 5  Dark Magenta
    db 42, 21,  0   ; 6  Brown
    db 42, 42, 42   ; 7  Light Gray
    db 21, 21, 21   ; 8  Dark Gray
    db 21, 21, 63   ; 9  Bright Blue
    db 21, 63, 21   ; 10 Bright Green
    db 21, 63, 63   ; 11 Bright Cyan
    db 63, 21, 21   ; 12 Bright Red
    db 63, 21, 63   ; 13 Bright Magenta
    db 63, 63, 21   ; 14 Yellow
    db 63, 63, 63   ; 15 White

    ; --- 16-231: 6x6x6 RGB color cube ---
    ; Index = 16 + 36*r + 6*g + b, levels: 0,15,26,36,46,57
    ; r=0
    db  0,  0,  0   ; 16
    db  0,  0, 15   ; 17
    db  0,  0, 26   ; 18
    db  0,  0, 36   ; 19
    db  0,  0, 46   ; 20
    db  0,  0, 57   ; 21
    db  0, 15,  0   ; 22
    db  0, 15, 15   ; 23
    db  0, 15, 26   ; 24
    db  0, 15, 36   ; 25
    db  0, 15, 46   ; 26
    db  0, 15, 57   ; 27
    db  0, 26,  0   ; 28
    db  0, 26, 15   ; 29
    db  0, 26, 26   ; 30
    db  0, 26, 36   ; 31
    db  0, 26, 46   ; 32
    db  0, 26, 57   ; 33
    db  0, 36,  0   ; 34
    db  0, 36, 15   ; 35
    db  0, 36, 26   ; 36
    db  0, 36, 36   ; 37
    db  0, 36, 46   ; 38
    db  0, 36, 57   ; 39
    db  0, 46,  0   ; 40
    db  0, 46, 15   ; 41
    db  0, 46, 26   ; 42
    db  0, 46, 36   ; 43
    db  0, 46, 46   ; 44
    db  0, 46, 57   ; 45
    db  0, 57,  0   ; 46
    db  0, 57, 15   ; 47
    db  0, 57, 26   ; 48
    db  0, 57, 36   ; 49
    db  0, 57, 46   ; 50
    db  0, 57, 57   ; 51
    ; r=1
    db 15,  0,  0   ; 52
    db 15,  0, 15   ; 53
    db 15,  0, 26   ; 54
    db 15,  0, 36   ; 55
    db 15,  0, 46   ; 56
    db 15,  0, 57   ; 57
    db 15, 15,  0   ; 58
    db 15, 15, 15   ; 59
    db 15, 15, 26   ; 60
    db 15, 15, 36   ; 61
    db 15, 15, 46   ; 62
    db 15, 15, 57   ; 63
    db 15, 26,  0   ; 64
    db 15, 26, 15   ; 65
    db 15, 26, 26   ; 66
    db 15, 26, 36   ; 67
    db 15, 26, 46   ; 68
    db 15, 26, 57   ; 69
    db 15, 36,  0   ; 70
    db 15, 36, 15   ; 71
    db 15, 36, 26   ; 72
    db 15, 36, 36   ; 73
    db 15, 36, 46   ; 74
    db 15, 36, 57   ; 75
    db 15, 46,  0   ; 76
    db 15, 46, 15   ; 77
    db 15, 46, 26   ; 78
    db 15, 46, 36   ; 79
    db 15, 46, 46   ; 80
    db 15, 46, 57   ; 81
    db 15, 57,  0   ; 82
    db 15, 57, 15   ; 83
    db 15, 57, 26   ; 84
    db 15, 57, 36   ; 85
    db 15, 57, 46   ; 86
    db 15, 57, 57   ; 87
    ; r=2
    db 26,  0,  0   ; 88
    db 26,  0, 15   ; 89
    db 26,  0, 26   ; 90
    db 26,  0, 36   ; 91
    db 26,  0, 46   ; 92
    db 26,  0, 57   ; 93
    db 26, 15,  0   ; 94
    db 26, 15, 15   ; 95
    db 26, 15, 26   ; 96
    db 26, 15, 36   ; 97
    db 26, 15, 46   ; 98
    db 26, 15, 57   ; 99
    db 26, 26,  0   ; 100
    db 26, 26, 15   ; 101
    db 26, 26, 26   ; 102
    db 26, 26, 36   ; 103
    db 26, 26, 46   ; 104
    db 26, 26, 57   ; 105
    db 26, 36,  0   ; 106
    db 26, 36, 15   ; 107
    db 26, 36, 26   ; 108
    db 26, 36, 36   ; 109
    db 26, 36, 46   ; 110
    db 26, 36, 57   ; 111
    db 26, 46,  0   ; 112
    db 26, 46, 15   ; 113
    db 26, 46, 26   ; 114
    db 26, 46, 36   ; 115
    db 26, 46, 46   ; 116
    db 26, 46, 57   ; 117
    db 26, 57,  0   ; 118
    db 26, 57, 15   ; 119
    db 26, 57, 26   ; 120
    db 26, 57, 36   ; 121
    db 26, 57, 46   ; 122
    db 26, 57, 57   ; 123
    ; r=3
    db 36,  0,  0   ; 124
    db 36,  0, 15   ; 125
    db 36,  0, 26   ; 126
    db 36,  0, 36   ; 127
    db 36,  0, 46   ; 128
    db 36,  0, 57   ; 129
    db 36, 15,  0   ; 130
    db 36, 15, 15   ; 131
    db 36, 15, 26   ; 132
    db 36, 15, 36   ; 133
    db 36, 15, 46   ; 134
    db 36, 15, 57   ; 135
    db 36, 26,  0   ; 136
    db 36, 26, 15   ; 137
    db 36, 26, 26   ; 138
    db 36, 26, 36   ; 139
    db 36, 26, 46   ; 140
    db 36, 26, 57   ; 141
    db 36, 36,  0   ; 142
    db 36, 36, 15   ; 143
    db 36, 36, 26   ; 144
    db 36, 36, 36   ; 145
    db 36, 36, 46   ; 146
    db 36, 36, 57   ; 147
    db 36, 46,  0   ; 148
    db 36, 46, 15   ; 149
    db 36, 46, 26   ; 150
    db 36, 46, 36   ; 151
    db 36, 46, 46   ; 152
    db 36, 46, 57   ; 153
    db 36, 57,  0   ; 154
    db 36, 57, 15   ; 155
    db 36, 57, 26   ; 156
    db 36, 57, 36   ; 157
    db 36, 57, 46   ; 158
    db 36, 57, 57   ; 159
    ; r=4
    db 46,  0,  0   ; 160
    db 46,  0, 15   ; 161
    db 46,  0, 26   ; 162
    db 46,  0, 36   ; 163
    db 46,  0, 46   ; 164
    db 46,  0, 57   ; 165
    db 46, 15,  0   ; 166
    db 46, 15, 15   ; 167
    db 46, 15, 26   ; 168
    db 46, 15, 36   ; 169
    db 46, 15, 46   ; 170
    db 46, 15, 57   ; 171
    db 46, 26,  0   ; 172
    db 46, 26, 15   ; 173
    db 46, 26, 26   ; 174
    db 46, 26, 36   ; 175
    db 46, 26, 46   ; 176
    db 46, 26, 57   ; 177
    db 46, 36,  0   ; 178
    db 46, 36, 15   ; 179
    db 46, 36, 26   ; 180
    db 46, 36, 36   ; 181
    db 46, 36, 46   ; 182
    db 46, 36, 57   ; 183
    db 46, 46,  0   ; 184
    db 46, 46, 15   ; 185
    db 46, 46, 26   ; 186
    db 46, 46, 36   ; 187
    db 46, 46, 46   ; 188
    db 46, 46, 57   ; 189
    db 46, 57,  0   ; 190
    db 46, 57, 15   ; 191
    db 46, 57, 26   ; 192
    db 46, 57, 36   ; 193
    db 46, 57, 46   ; 194
    db 46, 57, 57   ; 195
    ; r=5
    db 57,  0,  0   ; 196
    db 57,  0, 15   ; 197
    db 57,  0, 26   ; 198
    db 57,  0, 36   ; 199
    db 57,  0, 46   ; 200
    db 57,  0, 57   ; 201
    db 57, 15,  0   ; 202
    db 57, 15, 15   ; 203
    db 57, 15, 26   ; 204
    db 57, 15, 36   ; 205
    db 57, 15, 46   ; 206
    db 57, 15, 57   ; 207
    db 57, 26,  0   ; 208
    db 57, 26, 15   ; 209
    db 57, 26, 26   ; 210
    db 57, 26, 36   ; 211
    db 57, 26, 46   ; 212
    db 57, 26, 57   ; 213
    db 57, 36,  0   ; 214
    db 57, 36, 15   ; 215
    db 57, 36, 26   ; 216
    db 57, 36, 36   ; 217
    db 57, 36, 46   ; 218
    db 57, 36, 57   ; 219
    db 57, 46,  0   ; 220
    db 57, 46, 15   ; 221
    db 57, 46, 26   ; 222
    db 57, 46, 36   ; 223
    db 57, 46, 46   ; 224
    db 57, 46, 57   ; 225
    db 57, 57,  0   ; 226
    db 57, 57, 15   ; 227
    db 57, 57, 26   ; 228
    db 57, 57, 36   ; 229
    db 57, 57, 46   ; 230
    db 57, 57, 57   ; 231

    ; --- 232-255: 24 grayscale shades (dark to light) ---
    db  2,  2,  2   ; 232
    db  4,  4,  4   ; 233
    db  7,  7,  7   ; 234
    db  9,  9,  9   ; 235
    db 12, 12, 12   ; 236
    db 14, 14, 14   ; 237
    db 17, 17, 17   ; 238
    db 19, 19, 19   ; 239
    db 22, 22, 22   ; 240
    db 24, 24, 24   ; 241
    db 27, 27, 27   ; 242
    db 29, 29, 29   ; 243
    db 32, 32, 32   ; 244
    db 34, 34, 34   ; 245
    db 37, 37, 37   ; 246
    db 39, 39, 39   ; 247
    db 42, 42, 42   ; 248
    db 44, 44, 44   ; 249
    db 47, 47, 47   ; 250
    db 49, 49, 49   ; 251
    db 52, 52, 52   ; 252
    db 54, 54, 54   ; 253
    db 57, 57, 57   ; 254
    db 59, 59, 59   ; 255
