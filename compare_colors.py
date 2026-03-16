#!/usr/bin/env python3
"""Compare color bars between expected and actual composite renders.
Analyzes whether differences are hue, saturation, or brightness (HSV).
"""
from PIL import Image
import colorsys

exp = Image.open('core/src/test_data/video/ct755r/ntsc_out_m6.png').convert('RGB')
act = Image.open('core/src/test_data/video/ct755r/ntsc_out_m6_actual.png').convert('RGB')

def avg_row(img, y):
    w = img.width
    pixels = [img.getpixel((x, y)) for x in range(w)]
    r = sum(p[0] for p in pixels) / len(pixels)
    g = sum(p[1] for p in pixels) / len(pixels)
    b = sum(p[2] for p in pixels) / len(pixels)
    return r, g, b

def rgb_hsv(r, g, b):
    h, s, v = colorsys.rgb_to_hsv(r/255, g/255, b/255)
    return h*360, s*100, v*100

def hue_diff(h1, h2):
    d = h1 - h2
    if d > 180: d -= 360
    if d < -180: d += 360
    return d

# Find horizontal bar boundaries by detecting row-to-row color changes
bar_rows = []
prev = None
for y in range(25, exp.height-20):
    c = avg_row(exp, y)
    rc = (round(c[0]), round(c[1]), round(c[2]))
    if prev != rc:
        bar_rows.append((y, c))
        prev = rc

print(f'Found {len(bar_rows)} distinct bar regions\n')
print(f'{"y":>4}  {"Exp RGB":>10}  {"Exp HSV":>20}  {"Act RGB":>10}  {"Act HSV":>20}  {"ΔHue°":>8}  {"ΔSat%":>7}  {"ΔVal%":>7}  Diagnosis')
print('-'*115)

for i, (y, ec) in enumerate(bar_rows):
    er, eg, eb = ec
    ac = avg_row(act, y)
    ar, ag, ab = ac

    eh, es, ev = rgb_hsv(er, eg, eb)
    ah, as_, av = rgb_hsv(ar, ag, ab)

    dh = hue_diff(ah, eh)
    ds = as_ - es
    dv = av - ev

    if abs(dh) > 5 and abs(dh) > abs(ds) and abs(dh) > abs(dv):
        diag = 'HUE'
    elif abs(ds) > 5 and abs(ds) > abs(dh):
        diag = 'SAT'
    elif abs(dv) > 5:
        diag = 'BRIGHT'
    else:
        diag = 'ok'

    print(f'{y:>4}  #{int(er):02X}{int(eg):02X}{int(eb):02X}      ({eh:5.0f}°,{es:4.0f}%,{ev:4.0f}%)  #{int(ar):02X}{int(ag):02X}{int(ab):02X}      ({ah:5.0f}°,{as_:4.0f}%,{av:4.0f}%)  {dh:>+8.1f}  {ds:>+7.1f}  {dv:>+7.1f}  {diag}')
