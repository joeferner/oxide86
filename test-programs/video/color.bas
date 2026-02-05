CLS
PRINT "QBasic Text Mode Color Map"
PRINT "---------------------------"
PRINT

' Loop through all 16 foreground colors
FOR i = 0 TO 15
    COLOR i, 0  ' Set foreground to i, background to black
    PRINT "Color"; i; " - This is foreground color"; i
NEXT i

' Reset to standard white on black
COLOR 7, 0
PRINT
PRINT "Press any key to see background variations..."
SLEEP

CLS
PRINT "Standard Backgrounds (0-7):"
FOR b = 0 TO 7
    COLOR 15, b
    PRINT " BG "; b; " ";
NEXT b

' Final Reset
COLOR 7, 0
PRINT
PRINT
PRINT "Note: Colors 8-15 for background usually trigger blinking."
END
