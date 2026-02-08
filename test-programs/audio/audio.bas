CLS
PRINT "Starting..."
start! = TIMER

PRINT "Playing music..."
PLAY "T180 L8 O3 G C E G4 E6 G6"

finish! = TIMER
duration! = finish! - start!

PRINT "Current Time: "; TIME$
PRINT "The sequence took:"; duration!; "seconds. Should take ~1.83 seconds"
