"""Run the release linux_gpu_measure child without activating windows or moving the pointer.
Build with `cargo build --release -p stillkit --features gpu,golden --example linux_gpu_measure`.
Optional --cpu-binary measures a separately built feature-off CPU executable.
"""
import argparse,json,os,pathlib,re,select,subprocess,time
parser=argparse.ArgumentParser();parser.add_argument('--binary',default='target/release/examples/linux_gpu_measure');parser.add_argument('--cpu-binary');parser.add_argument('--out',default='docs/media/native/linux/gpu');args=parser.parse_args()
out=pathlib.Path(args.out);out.mkdir(parents=True,exist_ok=True)
def sample(pid):
 root=pathlib.Path(f'/proc/{pid}');mem={k:int(v) for k,v in re.findall(r'^(Rss|Pss|Private_Clean|Private_Dirty):\s+(\d+)',(root/'smaps_rollup').read_text(),re.M)}
 mem['Private']=mem['Private_Clean']+mem['Private_Dirty'];stat=(root/'stat').read_text().rsplit(')',1)[1].split();status=(root/'status').read_text();maps=(root/'maps').read_text()
 contexts={'voluntary_all':0,'involuntary_all':0}
 for task in (root/'task').iterdir():
  try: task_status=(task/'status').read_text()
  except FileNotFoundError: continue
  contexts['voluntary_all']+=int(re.search(r'^voluntary_ctxt_switches:\s+(\d+)',task_status,re.M)[1]);contexts['involuntary_all']+=int(re.search(r'^nonvoluntary_ctxt_switches:\s+(\d+)',task_status,re.M)[1])
 gpu={};clients=set()
 for fd in (root/'fdinfo').iterdir():
  text=fd.read_text();client=re.search(r'drm-client-id:\s+(\d+)',text)
  if not client or client[1] in clients:continue
  clients.add(client[1])
  for k,v in re.findall(r'^(drm-memory-\S+):\s+(\d+) KiB',text,re.M):gpu[k]=gpu.get(k,0)+int(v)
 return dict(**contexts,memory_kib=mem,utime=int(stat[11]),stime=int(stat[12]),voluntary=int(re.search(r'^voluntary_ctxt_switches:\s+(\d+)',status,re.M)[1]),involuntary=int(re.search(r'^nonvoluntary_ctxt_switches:\s+(\d+)',status,re.M)[1]),gpu_memory_kib=gpu,libraries=sorted(set(l.split()[-1] for l in maps.splitlines() if re.search(r'libEGL|libGL|libvulkan|libgbm|radeonsi|libdrm|libgallium',l,re.I))))
def line(p):
 if not select.select([p.stdout],[],[],60)[0]:raise RuntimeError('child response timed out')
 text=p.stdout.readline().strip()
 if not text:raise RuntimeError(f'child exited {p.poll()}')
 return text
def command(p,text):p.stdin.write(text+'\n');p.stdin.flush()
def delta(a,b):return {k:b[k]-a[k] for k in ['utime','stime','voluntary','involuntary','voluntary_all','involuntary_all']}
old=subprocess.check_output(['gsettings','get','org.gnome.desktop.wm.preferences','focus-new-windows'],text=True).strip();results={}
try:
 subprocess.run(['gsettings','set','org.gnome.desktop.wm.preferences','focus-new-windows','strict'],check=True)
 for mode in ['cpu','gpu','glass','mixed']:
  binary=(args.cpu_binary or args.binary) if mode=='cpu' else args.binary
  env=os.environ.copy();env['WAYLAND_DEBUG']='client'
  with (out/f'{mode}-protocol.log').open('w') as log:
   p=subprocess.Popen([binary,mode,str(out/f'{mode}.png')],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=log,text=True,env=env)
   try:
    print(mode,line(p),flush=True);time.sleep(10);a=sample(p.pid);time.sleep(30);b=sample(p.pid)
    command(p,'snap');print(line(p),flush=True)
    c=sample(p.pid);command(p,'drag');drag=line(p);d=sample(p.pid)
    result=dict(settle_seconds=10,idle_seconds=30,drag_seconds=10,start=a,idle_end=b,idle_delta=delta(a,b),drag_start=c,drag_end=d,drag_delta=delta(c,d),drag=drag,clk_tck=os.sysconf('SC_CLK_TCK'))
    (out/f'{mode}.json').write_text(json.dumps(result,indent=2));results[mode]=result
    command(p,'quit');p.wait(timeout=10);assert p.returncode==0
   finally:
    if p.poll() is None:p.kill();p.wait()
  print(mode,drag,result['idle_delta'],flush=True)
finally:
 subprocess.run(['gsettings','set','org.gnome.desktop.wm.preferences','focus-new-windows',old],check=True)
(out/'measurements.json').write_text(json.dumps(results,indent=2))
