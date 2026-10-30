# /// script
# requires-python = ">=3.11"
# dependencies = ["shapely==2.1.2", "pymupdf==1.26.7"]
# ///
"""Run genuine KiCad checks, compare nets and coordinates, and export/read back CAD.

Default succeeds only when checks are clean. --draft allows specifically reported
unconnected items only; geometric DRC findings always fail in either mode.
The JSON remains an honest count and is NOT a fabrication approval.
"""
from pathlib import Path
from collections import Counter
import argparse, hashlib, json, subprocess, xml.etree.ElementTree as ET
from kicad_runtime import configure
configure()
import pcbnew as pcb
from shapely.geometry import box, LineString, Point, Polygon
from shapely import make_valid
from shapely.ops import unary_union
import pymupdf
VARIANTS={65:('preferred',[155,75,205,125]),62:('rear_clearance',[151.425,52.7,208.575,128.575])}
def indexed_keys(layout,variant):
 refs={key_id:i for i,key_id in enumerate([f'r{r}c{c}' for r in range(5) for c in range(15) if r in (0,4) or c not in (6,7,8)],1)}
 selected={k['id'] for k in layout[variant]['keys']}
 return [(refs[k['id']],k) for k in layout['preferred']['keys'] if k['id'] in selected]
def circuit_net_matches(expected,actual,ref,pin,net_pad_count):
 if expected is None:
  return actual.startswith(f'unconnected-({ref}-') and actual.endswith(f'-Pad{pin})') and net_pad_count==1
 return actual==expected.replace('/','{slash}')
def reservation_matches(shape,reservation):
 return shape.is_valid and shape.hausdorff_distance(reservation)<=1e-5 and shape.symmetric_difference(reservation).area<1e-3
def usb_connector_matches(pad_nets):
 return pad_nets=={'1':'VBUS_USB','2':'USB_DM','3':'USB_DP','4':'GND','MP':'GND'}
def usb_uncovered_area(trace_area,reference,transition_exclusions):
 return trace_area.difference(reference).difference(transition_exclusions).area
ROOT=Path(__file__).resolve().parents[1]
parser=argparse.ArgumentParser();parser.add_argument('--draft',action='store_true');parser.add_argument('--keys',type=int,choices=VARIANTS,default=65);args=parser.parse_args()
D=ROOT/'design/pcb' if args.keys==65 else ROOT/'design/pcb/62'; R=D/'reports'; E=D/'exports'; NAME=f'keycult{args.keys}'
R.mkdir(parents=True,exist_ok=True); E.mkdir(parents=True,exist_ok=True)
def run(*args):
 command=['kicad-cli',*map(str,args)];r=subprocess.run(command,check=False,capture_output=True,text=True)
 print(r.stdout.strip())
 if r.returncode:raise RuntimeError(f'{command}: {r.returncode}\n{r.stderr}')
 return command
commands=[]
commands.append(run('sch','erc','--format','json','--severity-all','-o',R/'erc.json',D/(NAME+'.kicad_sch')))
commands.append(run('sch','export','netlist','--format','kicadxml','-o',R/'netlist.xml',D/(NAME+'.kicad_sch')))
commands.append(run('pcb','drc','--schematic-parity','--refill-zones','--save-board','--format','json','--severity-all','-o',R/'drc.json',D/(NAME+'.kicad_pcb')))
b=pcb.LoadBoard(str(D/(NAME+'.kicad_pcb')));fps={f.GetReference():f for f in b.GetFootprints()}; errs=[]
def check(ok,message):
 if not ok:errs.append(message)
def xy(p):return (pcb.ToMM(p.x),pcb.ToMM(p.y))
layout=json.loads((ROOT/'design/preliminary/layout.json').read_text());variant,expected_keepout=VARIANTS[args.keys];keys=indexed_keys(layout,variant)
circuit=json.loads((D/'circuit.json').read_text());expectedparts={p['ref']:p for p in circuit['parts'] if p['fp']}
external=[p for p in circuit['parts'] if p['ref']=='DB1']
check(len(external)==1 and external[0]['value']=='UDB C5-JSH / EXTERNAL' and not external[0]['fp'],'Expected external C5-JSH assembly DB1 without main-board footprint')
check('DB1' not in fps,'External daughterboard must not be placed on the main PCB')
check(set(fps)==set(expectedparts),'Schematic/PCB footprint reference set differs')
check('J1' in fps,'Missing UDB cable connector')
if 'J1' in fps:
 j=fps['J1']
 check(usb_connector_matches({p.GetNumber():p.GetNetname() for p in j.Pads()}),'J1 differs from the UDB contact pinout')
 check(j.GetFPID().GetLibItemName()=='JST_SH_SM04B-SRSS-TB_1x04-1MP_P1.00mm_Horizontal','Wrong J1 connector footprint')
 check(j.GetLayer()==pcb.B_Cu and abs(j.GetOrientationDegrees()-90)<.00001,'J1 must face the left edge from the bottom side')
check(circuit['key_count']==args.keys and circuit['layout_variant']==variant,'Circuit layout variant differs from selected key count')
check(len(keys)==args.keys and layout[variant]['key_count']==args.keys,'Unexpected layout key count')
check([k for _,k in keys]==layout[variant]['keys'],'Variant must preserve baseline key coordinates and order')
switch_refs={f'SW{i}' for i,_ in keys};diode_refs={f'D{i}' for i,_ in keys}
check({ref for ref in fps if ref.startswith('SW') and ref not in {'SW67','SW68'}}==switch_refs,'Wrong matrix switch references')
check({ref for ref in fps if ref.startswith('D')}==diode_refs,'Wrong matrix diode references')
coord=[]
for i,k in keys:
 if f'SW{i}' not in fps or f'D{i}' not in fps:continue
 f=fps[f'SW{i}'];expected=(180+k['x_mm'],100+k['y_mm'])
 # Inspect actual 4mm stem NPTH, not the footprint anchor.
 stems=[p for p in f.Pads() if not p.GetNumber() and pcb.ToMM(p.GetDrillSize().x)>3.9]
 check(len(stems)==1,'Expected one stem NPTH '+k['id'])
 if len(stems)!=1:continue
 stem=stems[0]
 actual=xy(stem.GetPosition());delta=max(abs(actual[j]-expected[j]) for j in [0,1]);check(delta<.00001,'Stem position '+k['id'])
 check(f.GetValue().startswith(k['id']+' '),'Wrong key ID '+k['id'])
 check(f.GetFPID().GetLibItemName()==f'SW_Cherry_MX_{k["width_u"]:.2f}u_PCB','Wrong switch width '+k['id'])
 check(fps[f'D{i}'].GetLayer()==pcb.B_Cu,'Diode not bottom-mounted '+k['id'])
 for ref,nets in [(f'SW{i}',{'1':f'COL{k["column"]}','2':'K_'+k['id']}),(f'D{i}',{'1':f'ROW{k["row"]}','2':'K_'+k['id']})]:
  check({p.GetNumber():p.GetNetname() for p in fps[ref].Pads() if p.GetNumber()}==nets,'Wrong matrix pad nets '+ref)
 coord.append({'id':k['id'],'switch':f'SW{i}','expected_stem_mm':expected,'actual_stem_mm':actual,'error_mm':delta})
netroot=ET.parse(R/'netlist.xml').getroot()
schnet={(n.attrib['ref'],n.attrib['pin']):net.attrib['name'].replace('/','{slash}') for net in netroot.find('nets') for n in net.findall('node')}
check({pin:net for (ref,pin),net in schnet.items() if ref=='DB1'}=={'1':'VBUS_USB','2':'USB_DM','3':'USB_DP','4':'GND'},'External C5-JSH netlist pinout differs from J1')
netdiff=[]
net_pad_counts=Counter(p.GetNetname() for f in fps.values() for p in f.Pads() if p.GetNumber())
for ref,f in fps.items():
 if ref in expectedparts:
  check({p.GetNumber() for p in f.Pads() if p.GetNumber()}==set(expectedparts[ref]['nets']),'PCB pad set differs from circuit: '+ref)
 for p in f.Pads():
  if not p.GetNumber():continue
  key=(ref,p.GetNumber());a=schnet.get(key,'');actual=p.GetNetname()
  if a!=actual:netdiff.append({'ref':ref,'pin':p.GetNumber(),'schematic':a,'pcb':actual})
  if ref in expectedparts:
   check(circuit_net_matches(expectedparts[ref]['nets'].get(p.GetNumber()),actual,ref,p.GetNumber(),net_pad_counts[actual]),'PCB pad net differs from circuit: '+ref+':'+p.GetNumber())
check(not netdiff,'Actual KiCad netlist and PCB pad net mismatch')
check(circuit['keepout_mm']==expected_keepout,'Circuit central reservation bounds differ from selected variant')
keepout=box(*circuit['keepout_mm']); fill_keepout=keepout; bad=[]
def chain(c):return [xy(c.CPoint(i)) for i in range(c.PointCount())]
for f in fps.values():
 # Courtyard covers body, not potentially offboard reference fields.
 for shape in f.GraphicalItems():
  if shape.GetLayer() in (pcb.F_CrtYd,pcb.B_CrtYd):
   a=shape.GetBoundingBox();g=box(*xy(a.GetOrigin()),*xy(a.GetEnd()))
   if g.intersection(keepout).area>1e-8:bad.append('courtyard:'+f.GetReference())
 for p in f.Pads():
  a=p.GetBoundingBox();g=box(*xy(a.GetOrigin()),*xy(a.GetEnd()))
  if g.intersection(keepout).area>1e-8:bad.append('pad:'+f.GetReference()+':'+p.GetNumber())
for t in b.GetTracks():
 if isinstance(t,pcb.PCB_VIA):g=Point(*xy(t.GetPosition())).buffer(pcb.ToMM(t.GetWidth(pcb.F_Cu))/2)
 else:g=LineString([xy(t.GetStart()),xy(t.GetEnd())]).buffer(pcb.ToMM(t.GetWidth())/2)
 if g.intersects(keepout):bad.append('copper:'+t.GetNetname())
# Verify the actual board rule area has every restriction on all enabled copper layers.
zones=list(b.Zones());rules=[z for z in zones if z.GetIsRuleArea()]
check(len(rules)==1,'Expected one central rule area')
if rules:
 z=rules[0];check(all([z.GetDoNotAllowTracks(),z.GetDoNotAllowVias(),z.GetDoNotAllowPads(),z.GetDoNotAllowZoneFills(),z.GetDoNotAllowFootprints()]),'Central rule area restriction missing')
 outline=z.Outline()
 check(outline.OutlineCount()==1 and outline.HoleCount(0)==0,'Central rule area must have one outline and no holes')
 if outline.OutlineCount()==1:
  rule_shape=Polygon(chain(outline.COutline(0)),[chain(outline.CHole(0,h)) for h in range(outline.HoleCount(0))])
  check(reservation_matches(rule_shape,keepout),'Central rule area polygon differs from circuit reservation')
  # Filled copper follows the native boundary, whose mm conversion can round by 1 nm.
  if reservation_matches(rule_shape,keepout):fill_keepout=rule_shape
 for layer in [pcb.F_Cu,pcb.In1_Cu,pcb.In2_Cu,pcb.B_Cu]:check(z.GetLayerSet().Contains(layer),'Keepout missing copper layer')
# Inspect filled polygons too; outline-with-hole geometry must not occupy the reserve.
for z in zones:
 if z.GetIsRuleArea():continue
 for layer in [pcb.F_Cu,pcb.In1_Cu,pcb.In2_Cu,pcb.B_Cu]:
  if not z.GetLayerSet().Contains(layer):continue
  filled=z.GetFilledPolysList(layer)
  for j in range(filled.OutlineCount()):
   g=Polygon(chain(filled.COutline(j)),[chain(filled.CHole(j,h)) for h in range(filled.HoleCount(j))])
   if g.intersection(fill_keepout).area>1e-8:bad.append('filled-copper:'+z.GetNetname())
check(not bad,'Component/copper intersects central reservation: '+str(sorted(set(bad))))
# A rectangular envelope only: no inner Edge.Cuts and no inherited tray mounting holes.
edges=[x for x in b.GetDrawings() if x.GetLayer()==pcb.Edge_Cuts]
check(len(edges)==4,'Working envelope must have four edges, no invented central cutout')
epts=[xy(p) for e in edges for p in [e.GetStart(),e.GetEnd()]]
bounds=[min(p[0] for p in epts),min(p[1] for p in epts),max(p[0] for p in epts),max(p[1] for p in epts)]
check(all(abs(a-z)<.00001 for a,z in zip(bounds,[37.5,52.7,322.5,147.3])),'GH60 scalar envelope bounds differ from 285.0 x 94.6 mm')
commands.append(run('sch','export','pdf','--exclude-pdf-property-popups','-o',E/'schematic.pdf',D/(NAME+'.kicad_sch')))
commands.append(run('sch','export','svg','-o',str(E/'schematic')+'/',D/(NAME+'.kicad_sch')))
for side,layers in [('front','F.Cu,F.SilkS,Edge.Cuts,Dwgs.User'),('back','B.Cu,B.SilkS,Edge.Cuts,Dwgs.User'),('ground','In1.Cu,Edge.Cuts,Dwgs.User')]:
 commands.append(run('pcb','export','svg','--layers',layers,'--mode-single','--fit-page-to-board','--exclude-drawing-sheet','-o',E/(side+'.svg'),D/(NAME+'.kicad_pcb')))
commands.append(run('pcb','export','pdf','--layers','F.Cu,B.Cu,In1.Cu,In2.Cu,Edge.Cuts,Dwgs.User','--mode-multipage','--scale','0','-o',E/'board-layers.pdf',D/(NAME+'.kicad_pcb')))
readback={}
for p in sorted(E.rglob('*.svg')):
 # Normalize KiCad trailing spaces without changing vector geometry.
 p.write_text('\n'.join(line.rstrip() for line in p.read_text().splitlines())+'\n')
 root=ET.parse(p).getroot();check(root.tag.endswith('svg'),'SVG parse failure '+str(p));readback[str(p.relative_to(D))]={'bytes':p.stat().st_size,'viewBox':root.attrib.get('viewBox')}
for p in [E/'schematic.pdf',E/'board-layers.pdf']:
 doc=pymupdf.open(p);check(len(doc)>0,'Empty PDF '+str(p));readback[str(p.relative_to(D))]={'pages':len(doc),'page_sizes_pt':[[p.rect.width,p.rect.height] for p in doc]}
 if p.name=='schematic.pdf':
  doc[0].get_pixmap(matrix=pymupdf.Matrix(.9,.9)).save(str(E/'schematic-preview.png'))
  # Native PDF crop for checking fine MCU, power, flash and crystal connections.
  doc[0].get_pixmap(matrix=pymupdf.Matrix(2,2),clip=pymupdf.Rect(15*72/25.4,48*72/25.4,290*72/25.4,190*72/25.4)).save(str(E/'mcu-schematic-detail.png'))
erc=json.loads((R/'erc.json').read_text());drc=json.loads((R/'drc.json').read_text())
eitems=[v for s in erc.get('sheets',[]) for v in s.get('violations',[])];viol=drc['violations'];unconn=drc['unconnected_items'];parity=drc['schematic_parity']
check(not eitems,'ERC violations');check(not parity,'KiCad native schematic parity violations')
usb_names=('USB_DM','USB_DP')
usb_tracks=[t for t in b.GetTracks() if not isinstance(t,pcb.PCB_VIA) and t.GetNetname() in usb_names]
usb_vias=[t for t in b.GetTracks() if isinstance(t,pcb.PCB_VIA) and t.GetNetname() in usb_names]
gnd_vias=[xy(t.GetPosition()) for t in b.GetTracks() if isinstance(t,pcb.PCB_VIA) and t.GetNetname()=='GND']
check(all(t.GetLayer() in (pcb.F_Cu,pcb.B_Cu) for t in usb_tracks),'Connector-side USB must use F.Cu with B.Cu escapes')
check(all(abs(pcb.ToMM(t.GetWidth())-.29)<1e-5 for t in usb_tracks),'Connector-side USB width differs from .29mm study')
check(all(sum(pcb.ToMM(t.GetLength()) for t in usb_tracks if t.GetNetname()==net and t.GetLayer()==pcb.F_Cu)>130 for net in usb_names),'Missing F.Cu USB margin route')
reference=[]
for z in b.Zones():
 if z.GetNetname()=='GND' and z.GetLayer()==pcb.In1_Cu:
  poly=z.GetFilledPolysList(pcb.In1_Cu)
  reference.extend(make_valid(Polygon(chain(poly.COutline(i)),[chain(poly.CHole(i,j)) for j in range(poly.HoleCount(i))])) for i in range(poly.OutlineCount()))
reference=unary_union(reference)
transitions=unary_union([Point(xy(v.GetPosition())).buffer(.51) for v in usb_vias])
usb_areas={net:unary_union([LineString([xy(t.GetStart()),xy(t.GetEnd())]).buffer(pcb.ToMM(t.GetWidth())/2,cap_style=2) for t in usb_tracks if t.GetNetname()==net and t.GetLayer()==pcb.F_Cu]) for net in usb_names}
uncovered={net:usb_uncovered_area(area,reference,transitions) for net,area in usb_areas.items()}
check(all(area<1e-6 for area in uncovered.values()),'F.Cu USB copper crosses an In1.Cu reference void outside transition antipads')
pair_gap=usb_areas['USB_DM'].distance(usb_areas['USB_DP'])
check(pair_gap>=.2-1e-5,'F.Cu USB pair edge gap below .2mm study')
via_ground_distances=[min((Point(xy(v.GetPosition())).distance(Point(p)) for p in gnd_vias),default=float('inf')) for v in usb_vias]
check(len(usb_vias)==8 and all(d<=3 for d in via_ground_distances),'USB transitions require nearby ground vias within 3mm study rule')
summary={'status':('validation failed' if errs or viol or (not args.draft and unconn) else 'reviewable draft; NOT manufacturing approval'),'key_count':len(keys),'layout_variant':variant,'keepout_mm':circuit['keepout_mm'],'kicad_version':pcb.GetBuildVersion(),'erc_count':len(eitems),'erc_by_type':dict(Counter(v['type'] for v in eitems)),'drc_count':len(viol),'drc_by_type':dict(Counter(v['type'] for v in viol)),'drc_by_severity':dict(Counter(v['severity'] for v in viol)),'unconnected_item_count':len(unconn),'unconnected_nets':sorted({i['description'].split('[')[1].split(']')[0] for v in unconn for i in v['items'] if '[' in i['description']}),'schematic_parity_count':len(parity),'independent_netlist_mismatch_count':len(netdiff),'envelope_bounds_mm':bounds,'switch_count':len(set(fps)&switch_refs),'diode_count':len(set(fps)&diode_refs),'footprint_count':len(fps),'net_count':len(list(b.GetNetInfo().NetsByNetcode()))-1,'track_segment_count':sum(not isinstance(t,pcb.PCB_VIA) for t in b.GetTracks()),'via_count':sum(isinstance(t,pcb.PCB_VIA) for t in b.GetTracks()),'signal_tracks_on_ground_layer':sum(t.GetLayer()==pcb.In1_Cu and t.GetNetname() not in ('GND','+3V3','+1V1') for t in b.GetTracks() if not isinstance(t,pcb.PCB_VIA)),'keepout_intersections':sorted(set(bad)),'structural_errors':errs,'native_export_readback':readback,'commands':commands,'artifact_sha256':{p.name:hashlib.sha256(p.read_bytes()).hexdigest() for p in [D/(NAME+ext) for ext in ['.kicad_pro','.kicad_sch','.kicad_pcb']]}}
summary['usb_reference_review']={'f_cu_width_mm':.29,'minimum_f_cu_pair_edge_gap_mm':round(pair_gap,6),'uncovered_projection_mm2':uncovered,'transition_exclusion_radius_mm':.51,'maximum_signal_via_to_ground_via_mm':round(max(via_ground_distances,default=0),6),'note':'Saved refilled In1 polygons, make_valid for KiCad touching contours. Projection only; .51mm disks exclude signal-via antipads. 3mm stitching rule is a project study condition, not USB compliance. Finite plane edges, transitions and actual impedance still require qualification.'}
summary['ground_layer_tracks_by_net']=dict(Counter(t.GetNetname() for t in b.GetTracks() if not isinstance(t,pcb.PCB_VIA) and t.GetLayer()==pcb.In1_Cu))
(R/'validation.json').write_text(json.dumps(summary,indent=2,ensure_ascii=False)+'\n');(R/'switch-coordinates.json').write_text(json.dumps(coord,indent=2)+'\n')
print(json.dumps({k:v for k,v in summary.items() if k not in ['commands','native_export_readback','artifact_sha256']},indent=2))
if errs or viol or (not args.draft and unconn):raise SystemExit(1)
