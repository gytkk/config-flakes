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
import argparse,json,subprocess
import sexpdata as sx
ROOT=Path(__file__).resolve().parents[1]
parser=argparse.ArgumentParser();parser.add_argument('--keys',type=int,choices=(65,62),default=65);args=parser.parse_args()
FP=ROOT/'design/pcb'/('62' if args.keys==62 else '')/f'keycult{args.keys}.kicad_pcb'
REPORT=ROOT/f'.cache/pcb-tools/cleanup-drc-{args.keys}.json'
S=sx.Symbol
def tag(x,name):return isinstance(x,list) and x and str(x[0])==name
def child(x,name):return next((a for a in x if tag(a,name)),None)
def save(a):FP.write_text(sx.dumps(a)+'\n')
removed=[]
for _ in range(12):
 subprocess.run(['kicad-cli','pcb','drc','--refill-zones','--save-board','--format','json','-o',str(REPORT),str(FP)],check=True,stdout=subprocess.DEVNULL)
 report=json.loads(REPORT.read_text());dead=[v for v in report['violations'] if v['type'] in ('track_dangling','via_dangling')]
 if not dead:break
 ids={i['uuid'] for v in dead for i in v['items']};a=sx.load(FP.open());new=[]
 for n in a:
  u=child(n,'uuid') if isinstance(n,list) else None
  if (tag(n,'segment') or tag(n,'via')) and u and u[1] in ids:removed.append(u[1])
  else:new.append(n)
 save(new)
else:raise RuntimeError('Dead-end cleanup did not converge; inspect actual DRC')
print('Removed',len(removed),'KiCad-confirmed open-ended tracks/vias. Rules and unconnected reports unchanged.')
