"""Dump a guest-memory surface of the running local RPCS3 test instance as a PNG (read-only,
through rpcs3_guest_read.py) and print the fraction of black pixels. Used to inspect the main-memory
G-buffer copies the SPUs read (TEST213: depth 0x445A3A80, normals 0x44813A80, 1024x624).

Usage: python scripts/rpcs3_dump_surface.py ADDR WIDTH HEIGHT OUT.png [argb|rgba]
"""
import sys,struct
sys.path.insert(0, str(__import__('pathlib').Path(__file__).resolve().parent)); _a=sys.argv; sys.argv=sys.argv[:1]
import rpcs3_guest_read as g
sys.argv=_a
from PIL import Image
B=g.find_base()
addr=int(sys.argv[1],0); w=int(sys.argv[2]); h=int(sys.argv[3]); out=sys.argv[4]; fmt=sys.argv[5] if len(sys.argv)>5 else 'argb'
data=b''
chunk=w*4*16
for y in range(0,h,16):
    n=min(16,h-y)*w*4
    d=g.rpm(B+addr+y*w*4,n) or bytes(n); data+=d
if fmt=='argb':  # big-endian A8R8G8B8
    rgb=bytes(x for i in range(0,len(data),4) for x in (data[i+1],data[i+2],data[i+3]))
else:  # rgba
    rgb=bytes(x for i in range(0,len(data),4) for x in (data[i],data[i+1],data[i+2]))
Image.frombytes('RGB',(w,h),rgb).save(out)
blk=sum(1 for i in range(0,len(data),4) if data[i+1]<8 and data[i+2]<8 and data[i+3]<8)/(w*h)
print(out,'black',round(blk,3))
