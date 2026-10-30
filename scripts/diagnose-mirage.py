#!/usr/bin/env python3
"""Bounded QMP diagnostic; remove raw memory after extracting selected evidence."""
import argparse, json, os, re, socket, struct, tempfile
from pathlib import Path
p=argparse.ArgumentParser()
p.add_argument('--device',type=Path,required=True)
p.add_argument('--output',type=Path,required=True)
p.add_argument('--stop',action='store_true')
p.add_argument('--resume',action='store_true')
p.add_argument('--trace-offset',type=lambda x:int(x,0))
a=p.parse_args()
a.output.mkdir(parents=True,exist_ok=True)
r=json.loads((a.device/'runtime.json').read_text())
url=r.get('qmpAux') or r['commandLine'].split(' -qmp unix:',2)[1].split(',server',1)[0]
s=socket.socket(socket.AF_UNIX); s.connect(url); f=s.makefile('rwb',buffering=0)
f.readline()
def call(cmd,args=None):
    f.write((json.dumps({'execute':cmd,'arguments':args or {}})+'\n').encode())
    while True:
        reply=json.loads(f.readline())
        if 'return' in reply:return reply['return']
        if 'error' in reply:raise RuntimeError(reply)
call('qmp_capabilities')
if a.resume:call('cont')
else:
    call('stop')
    regs=call('human-monitor-command',{'command-line':'info registers'})
    (a.output/'registers.txt').write_text(regs)
    if a.trace_offset is not None:
        framework=int(call('human-monitor-command',{'command-line':'xp /1wx 0x200d972c'}).split(':')[-1],16)
        vptr=int(call('human-monitor-command',{'command-line':f'xp /1wx {framework}'}).split(':')[-1],16)
        address=vptr+a.trace_offset
        raw=Path(tempfile.mktemp(prefix='astrolink-trace-'))
        try:
            call('pmemsave',{'val':address,'size':256,'filename':str(raw)})
            trace=list(struct.unpack('<64I',raw.read_bytes()))
            (a.output/'native-trace.json').write_text(json.dumps({'framework':framework,'vtable':vptr,'traceAddress':address,'stages':trace},indent=2))
        finally:raw.unlink(missing_ok=True)
    raw=Path(tempfile.mktemp(prefix='astrolink-sram-'))
    try:
        call('pmemsave',{'val':0x20100000,'size':65536,'filename':str(raw)})
        data=raw.read_bytes()
        out=[]
        for m in re.finditer(rb'[\x20-\x7e]{8,}',data):
            text=m.group().decode()
            if re.search(r'/(?:  )?[0-9]+[ /]|panic|fault|assert|error|Astrolink',text,re.I):
                out.append(f'0x{0x20100000+m.start():08x} {text}')
        (a.output/'live-sram-64k.log').write_text('\n'.join(out))
    finally:raw.unlink(missing_ok=True)
s.close()
