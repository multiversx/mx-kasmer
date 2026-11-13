"""Remove the DataCount section (id 12) from a wasm binary, in place.
Only valid when the module uses neither memory.init nor data.drop."""
import sys

def leb(b, i):
    r = s = 0
    while True:
        x = b[i]; i += 1
        r |= (x & 0x7F) << s; s += 7
        if not x & 0x80:
            return r, i

for path in sys.argv[1:]:
    b = open(path, 'rb').read()
    out = bytearray(b[:8]); i = 8
    while i < len(b):
        sid = b[i]; size, j = leb(b, i + 1)
        if sid != 12:
            out += b[i:j + size]
        i = j + size
    open(path, 'wb').write(out)
    print(path, len(b), '->', len(out))
