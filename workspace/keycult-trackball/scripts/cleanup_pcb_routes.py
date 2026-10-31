# /// script
# requires-python = ">=3.11"
# dependencies = ["sexpdata==1.0.2"]
# ///
"""Remove KiCad-reported dead-end copper; never suppress DRC or unconnected nets.

Use maintained sexpdata for these small native-file edits. KiCad 10's installed
SWIG deletion bindings proved unstable; all geometry is reloaded and checked by
kicad-cli after every edit. Refill first so real ground-plane vias are retained.
"""
from pathlib import Path
import argparse,copy,json,re,subprocess,uuid
from collections import Counter
import sexpdata as sx
ROOT=Path(__file__).resolve().parents[1]
parser=argparse.ArgumentParser();parser.add_argument('--keys',type=int,choices=(65,62),default=65);args=parser.parse_args()
FP=ROOT/'design/pcb'/('62' if args.keys==62 else '')/f'keycult{args.keys}.kicad_pcb'
REPORT=ROOT/f'.cache/pcb-tools/cleanup-drc-{args.keys}.json'
S=sx.Symbol
def tag(x,name):return isinstance(x,list) and x and str(x[0])==name
def child(x,name):return next((a for a in x if tag(a,name)),None)
def save(a):FP.write_text(sx.dumps(a)+'\n')
def junction_points(start,end,points):
 dx,dy=end[0]-start[0],end[1]-start[1];length2=dx*dx+dy*dy
 cuts={tuple(start):0,tuple(end):1}
 if length2:
  for point in points:
   x,y=point[0]-start[0],point[1]-start[1];position=(x*dx+y*dy)/length2
   if 1e-7<position<1-1e-7 and abs(x*dy-y*dx)<=.001*length2**.5:cuts[tuple(point)]=position
 return sorted(cuts,key=cuts.get)
def junction_candidates(track,records):
 points=[]
 for other in records:
  if other is track or not isinstance(other,list) or child(other,'net')!=child(track,'net'):continue
  if tag(other,'segment') and child(other,'layer')==child(track,'layer'):points.extend([child(other,'start')[1:],child(other,'end')[1:]])
  elif tag(other,'via'):points.append(child(other,'at')[1:])
 return points
def unconnected_per_net(report):
 counts=Counter()
 for finding in report['unconnected_items']:
  names={match.group(1) for item in finding['items'] if (match:=re.search(r'\[([^\]]+)\]',item['description']))}
  counts.update(names or {'unknown'})
 return counts
removed=[];split_count=0;previous=None
for _ in range(32):
 subprocess.run(['kicad-cli','pcb','drc','--refill-zones','--save-board','--format','json','-o',str(REPORT),str(FP)],check=True,stdout=subprocess.DEVNULL)
 report=json.loads(REPORT.read_text());counts=unconnected_per_net(report)
 if previous and counts-previous[1]:
  FP.write_bytes(previous[0]);raise RuntimeError('Cleanup would disconnect copper; restored previous board: '+repr(counts-previous[1]))
 dead=[v for v in report['violations'] if v['type'] in ('track_dangling','via_dangling')]
 if not dead:break
 ids={i['uuid'] for v in dead for i in v['items']};content=FP.read_bytes();a=sx.load(FP.open());new=[];split=False
 # KiCad flags a whole segment when only its tail beyond a T junction is dead.
 for n in a:
  u=child(n,'uuid') if isinstance(n,list) else None
  if not (tag(n,'segment') and u and u[1] in ids):new.append(n);continue
  cuts=junction_points(child(n,'start')[1:],child(n,'end')[1:],junction_candidates(n,a))
  if len(cuts)<=2:new.append(n);continue
  split=True;split_count+=1
  for index,(start,end) in enumerate(zip(cuts,cuts[1:])):
   piece=copy.deepcopy(n);child(piece,'start')[1:]=start;child(piece,'end')[1:]=end
   child(piece,'uuid')[1]=str(uuid.uuid5(uuid.NAMESPACE_URL,f'keycult-cleanup/{u[1]}/{index}'));new.append(piece)
 previous=(content,counts)
 if split:save(new);continue
 new=[]
 for n in a:
  u=child(n,'uuid') if isinstance(n,list) else None
  if (tag(n,'segment') or tag(n,'via')) and u and u[1] in ids:removed.append(u[1])
  else:new.append(n)
 save(new)
else:
 if previous:FP.write_bytes(previous[0])
 raise RuntimeError('Dead-end cleanup did not converge; restored last checked board; inspect actual DRC')
print('Split',split_count,'T-junction segments; removed',len(removed),'KiCad-confirmed dead tracks/vias. No net gained unconnected items.')
