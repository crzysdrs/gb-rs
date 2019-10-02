#!/usr/bin/env python
from collections import defaultdict

manual = map(lambda x: x.split("\t")[2:-1], open("listButtonCombos.csv").read().split("\r\n")[1:-1])

colors = {}
for m in manual:
    for c in m[1:]:
        colors[c] = True


custom = map(lambda x: x.split("\t")[2:-1], open("listUsedWNames.csv").read().split("\r\n")[1:-1])

for m in custom:
    for c in m[3:]:
        colors[c] = True

all_c = list(colors.keys())
all_c.sort()
keys = dict([(v,k) for (k,v) in enumerate(all_c)])

for c in all_c:
    print "[0x{}, 0x{}, 0x{}],".format(c[1:3], c[3:5], c[5:7])
    
print(keys)

for m in manual:
    for c in range(1,len(m)):
        m[c] = keys[m[c]]
    print("KeyPalette {{keys: \"{}\", palette: Palette {{ bg:{}, obj0:{}, obj1:{} }}}},".format(m[0], m[1:5], m[5:9], m[9:13]))
    

matches = defaultdict(list)
for m in custom:
    for c in range(3, len(m)):
        m[c] = keys[m[c]]

    matches["CustomPalette {{bg:{}, obj0: {}, obj1: {} }}".format(m[3:7], m[7:11], m[11:15])].append("({}, {})".format(m[0], m[1] if m[1] else "_"))


for m,v  in matches.iteritems():
    print "{} => {},".format(("|".join(v)), m)
