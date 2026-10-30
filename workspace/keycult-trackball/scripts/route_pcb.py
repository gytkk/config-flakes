# /// script
# requires-python = ">=3.11"
# dependencies = []
# ///
"""Reapply the checked-in initial route, or run the pinned local Freerouting.

The CAD remains the editable deliverable. A changed circuit requires --route,
not applying an old snapshot. Downloads are explicit and checksum checked.
"""
from pathlib import Path
import argparse, hashlib, json, os, shutil, subprocess, urllib.request
from kicad_runtime import configure
configure()
import pcbnew as pcb
ROOT=Path(__file__).resolve().parents[1];D=ROOT/'design/pcb';CACHE=ROOT/'.cache/pcb-tools';CACHE.mkdir(parents=True,exist_ok=True)
JAR_SHA='d7fd0f63f52e6d74b0fad6715f87ca9f0ffd7109d66b2a584638000270592ecf'
URL='https://github.com/freerouting/freerouting/releases/download/v2.0.1/freerouting-2.0.1.jar'
parser=argparse.ArgumentParser();g=parser.add_mutually_exclusive_group(required=True);g.add_argument('--import-saved',action='store_true');g.add_argument('--route',action='store_true');parser.add_argument('--java');parser.add_argument('--passes',type=int,default=40);parser.add_argument('--keys',type=int,choices=(65,62),default=65);parser.add_argument('--capture',type=Path,help='Capture an already-completed local SES instead of launching router (requires --route)');args=parser.parse_args()
if args.keys==62:D=D/'62'
CACHE=CACHE/str(args.keys);CACHE.mkdir(parents=True,exist_ok=True)
def sha(path):return hashlib.sha256(path.read_bytes()).hexdigest()
fp=D/f'keycult{args.keys}.kicad_pcb';saved=D/'initial-route.ses';manifest=D/'route-manifest.json'
identity={str(p.relative_to(ROOT)):sha(p) for p in [D/'circuit.json',ROOT/'design/preliminary/layout.json',ROOT/'scripts/build_pcb.py',ROOT/'scripts/usb_routing.py']}
if args.import_saved:
 m=json.loads(manifest.read_text());assert m['input_sha256']==identity,'Circuit/layout changed: saved route cannot be reused';assert sha(saved)==m['ses_sha256'],'SES checksum mismatch'
else:
 if args.capture:
  source=args.capture
 else:
  jar=CACHE/'freerouting-2.0.1.jar'
  if not jar.exists():urllib.request.urlretrieve(URL,jar)
  assert sha(jar)==JAR_SHA,'Freerouting JAR checksum mismatch'
  java=args.java or os.environ.get('KEYCULT_JAVA') or shutil.which('java')
  if not java:
   matches=sorted(Path('/nix/store').glob('*openjdk-21*/bin/java'));java=str(matches[0]) if matches else None
  if not java:raise RuntimeError('Java 21 required; set KEYCULT_JAVA or --java')
  board=pcb.LoadBoard(str(fp))
  nc=board.GetDesignSettings().m_NetSettings.GetDefaultNetclass();nc.SetClearance(pcb.FromMM(.15));nc.SetTrackWidth(pcb.FromMM(.15))
  board.SetLayerType(pcb.In1_Cu,pcb.LT_POWER)
  dsn=CACHE/'seed.dsn';pcb.ExportSpecctraDSN(board,str(dsn));source=CACHE/'routed.ses'
  cmd=[java,'-Djava.awt.headless=true','-jar',str(jar),'-de',str(dsn),'-do',str(source),'-mp',str(args.passes),'-mt','1',f'--router.stop_pass_no={args.passes}','--gui.enabled=false','--api_server.enabled=false','--usage_and_diagnostic_data.disable_analytics=true','--feature_flags.logging=false','--user_data_path='+str(CACHE/'freerouting-profile')]
  for attempt in range(3):
   subprocess.run(cmd,check=True,timeout=900)
   if source.exists() and source.stat().st_size:break
   print(f'Freerouting returned an empty SES (attempt {attempt+1}/3); retrying its CLI export race.',flush=True)
  else:raise RuntimeError('Freerouting produced no SES after three attempts; saved route was not replaced')
 # Preserve board data, not the local checkout's absolute path in the SES header.
 content=source.read_text()
 if not content.lstrip().startswith('(session'):raise RuntimeError(f'Invalid SES from {source}; saved route was not replaced')
 content='\n'.join(line.rstrip() for line in content.replace(str(ROOT),'PROJECT_ROOT').splitlines())+'\n';saved.write_text(content)
 m={'tool':'Freerouting','version':'2.0.1','jar_url':URL,'jar_sha256':JAR_SHA,'java':'21','stop_passes':args.passes,'note':'v2.0.1 -mp alone does not bound BatchAutorouter; router.stop_pass_no is explicitly supplied. No cloud routing; usage analytics disabled. SES preserves the base route; scripts/usb_routing.py replaces connector-side USB copper and adds the DRAFT E ground changes after import.','input_sha256':identity,'ses_sha256':sha(saved)}
 manifest.write_text(json.dumps(m,indent=2)+'\n')
b=pcb.LoadBoard(str(fp));assert pcb.ImportSpecctraSES(b,str(saved)),'KiCad SES import failed'
# Drawing-only label placement fix: reserve board-edge silkscreen clearance.
for x in b.GetDrawings():
 if isinstance(x,pcb.PCB_TEXT) and x.GetText().startswith('r') and 'c' in x.GetText():
  p=x.GetPosition();p.y=pcb.FromMM(pcb.ToMM(p.y)-.8) if pcb.ToMM(p.y)>146.5 else p.y;x.SetPosition(p)
pcb.SaveBoard(str(fp),b)
subprocess.run(['uv','run',str(ROOT/'scripts/usb_routing.py'),str(fp)],check=True)
subprocess.run(['uv','run',str(ROOT/'scripts/cleanup_pcb_routes.py'),'--keys',str(args.keys)],check=True)
print('Imported and cleaned native route. Run validate_pcb.py; unconnected nets remain explicitly reportable.')
