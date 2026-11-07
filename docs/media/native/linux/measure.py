import subprocess,os,time,json,re,pathlib,argparse
parser=argparse.ArgumentParser()
parser.add_argument('--example',default='el_hello')
parser.add_argument('--settle',type=float,default=10)
parser.add_argument('--out',default='/tmp/stillkit-el-live')
args=parser.parse_args()
out=pathlib.Path(args.out);out.mkdir(exist_ok=True)
old=subprocess.check_output(['gsettings','get','org.gnome.desktop.wm.preferences','focus-new-windows'],text=True).strip()
p=None
try:
 subprocess.run(['gsettings','set','org.gnome.desktop.wm.preferences','focus-new-windows','strict'],check=True)
 env=os.environ.copy();env.update(XDG_RUNTIME_DIR='/run/user/1000',WAYLAND_DISPLAY='wayland-0',WAYLAND_DEBUG='client')
 log=(out/'protocol.log').open('w');p=subprocess.Popen([f'target/release/examples/{args.example}', '--inactive'] if args.example != 'linux_hello' else ['target/release/examples/linux_hello'],env=env,stderr=log,stdout=(out/'stdout.log').open('w'))
 (out/'pid').write_text(str(p.pid))
 def sample():
  root=pathlib.Path(f'/proc/{p.pid}');raw=(root/'smaps_rollup').read_text();mem={k:int(v) for k,v in re.findall(r'^(Rss|Pss|Private_Clean|Private_Dirty):\s+(\d+)',raw,re.M)};mem['Private']=mem['Private_Clean']+mem['Private_Dirty'];stat=(root/'stat').read_text().rsplit(')',1)[1].split();status=(root/'status').read_text();maps=(root/'maps').read_text();(out/'maps.txt').write_text(maps);return dict(memory_kib=mem,utime=int(stat[11]),stime=int(stat[12]),voluntary=int(re.search(r'^voluntary_ctxt_switches:\s+(\d+)',status,re.M)[1]),involuntary=int(re.search(r'^nonvoluntary_ctxt_switches:\s+(\d+)',status,re.M)[1]),gpu=[l for l in maps.splitlines() if re.search(r'libEGL|libGL|libvulkan|libgbm|radeonsi|libdrm_amdgpu',l,re.I)])
 time.sleep(args.settle);a=sample();(out/'start.json').write_text(json.dumps(a,indent=2));print('READY',p.pid,flush=True)
 time.sleep(30);b=sample();result={'start':a,'end':b,'delta':{k:b[k]-a[k] for k in ['utime','stime','voluntary','involuntary']},'seconds':30,'settle_seconds':args.settle,'example':args.example};(out/'measurement.json').write_text(json.dumps(result,indent=2));print(json.dumps(result),flush=True)
 # No timer or sampler runs inside the client.
 time.sleep(1)
finally:
 if p is not None:p.terminate();p.wait()
 subprocess.run(['gsettings','set','org.gnome.desktop.wm.preferences','focus-new-windows',old],check=True)
 print('restored',old,flush=True)
