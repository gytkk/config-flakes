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
import argparse, csv, hashlib, io, json, re, subprocess, tempfile, zipfile, xml.etree.ElementTree as ET
from kicad_runtime import configure
configure()
import pcbnew as pcb
from shapely.geometry import box, LineString, Point, Polygon
from shapely import make_valid
from shapely.ops import unary_union
import pymupdf
VARIANTS={65:('preferred',[155,75,205,125]),62:('rear_clearance',[155.75,75.75,204.25,124.25])}
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
def sensor_connector_matches(pad_nets):
 return pad_nets=={'1':'SENSOR_SCLK_3V3','2':'SENSOR_CS_3V3','3':'GND','4':'+3V3','5':'GND','6':'SENSOR_MOSI_3V3','7':'SENSOR_MISO_3V3','MP':'GND'}
def outline_matches(edges,keys):
 # Compare exact lines and arc midpoints: a notch, missing edge or wrong corner must fail.
 outer=[((37.5,52.7),(322.5,52.7)),((322.5,52.7),(322.5,147.3)),((322.5,147.3),(37.5,147.3)),((37.5,147.3),(37.5,52.7))]
 expected=[{'start':a,'end':b} for a,b in outer]
 if keys==62:
  expected.extend({'start':a,'end':b} for a,b in [((157,76),(203,76)),((204,77),(204,123)),((203,124),(157,124)),((156,123),(156,77))])
  d=2**-.5
  expected.extend({'start':a,'mid':m,'end':b} for a,m,b in [((203,76),(203+d,77-d),(204,77)),((204,123),(203+d,123+d),(203,124)),((157,124),(157-d,123+d),(156,123)),((156,77),(157-d,77-d),(157,76))])
 def normalized(edge):
  ends=sorted(tuple(round(v,5) for v in edge[k]) for k in ('start','end'))
  return tuple(ends)+(tuple(round(v,5) for v in edge['mid']) if 'mid' in edge else (),)
 return sorted(map(normalized,edges))==sorted(map(normalized,expected))
def usb_uncovered_area(trace_area,reference,transition_exclusions):
 return trace_area.difference(reference).difference(transition_exclusions).area
def stackup_layers(native):
 return [(name,kind,float(thickness),material or None,float(er) if er else None) for name,kind,thickness,material,er in re.findall(r'\(layer\s+"([^"]+)"\s+\(type\s+"([^"]+)"\)\s+\(thickness\s+([\d.]+)\)(?:\s+\(material\s+"([^"]+)"\)\s+\(epsilon_r\s+([\d.]+)\))?',native)]
def stackup_matches(layers):
 return layers==[('F.Cu','copper',.035,None,None),('dielectric 1','prepreg',.2104,'7628',4.4),('In1.Cu','copper',.0152,None,None),('dielectric 2','core',1.065,'FR4',4.6),('In2.Cu','copper',.0152,None,None),('dielectric 3','prepreg',.2104,'7628',4.4),('B.Cu','copper',.035,None,None)]
def placement_rows(content,expected):
 rows=list(csv.DictReader(io.StringIO(content)))
 if len(rows)!=len(expected) or {r['Ref'] for r in rows}!=set(expected):
  raise ValueError('CAM placement reference set differs from native PCB')
 for row in rows:
  x,y,angle,side=expected[row['Ref']]
  if max(abs(float(row['PosX'])-x),abs(float(row['PosY'])+y),abs(float(row['Rot'])-angle))>1e-5 or row['Side']!=side:
   raise ValueError('CAM placement coordinates/rotation/side differ: '+row['Ref'])
 return [r for r in rows if not r['Ref'].startswith('TP') and not (r['Ref'].startswith('SW') and int(r['Ref'][2:])<=66)]
def export_prototype_cam(directory,board,summary):
 """Stage and inspect prototype CAM before publishing a local review bundle."""
 name=board.GetFileName();stem=Path(name).stem;output=directory/'prototype-cam'
 with tempfile.TemporaryDirectory(prefix=stem+'-cam-') as temporary:
  stage=Path(temporary);commands=[]
  commands.append(run('pcb','export','gerbers','--layers','F.Cu,In1.Cu,In2.Cu,B.Cu,F.Mask,B.Mask,F.SilkS,B.SilkS,F.Paste,B.Paste,Edge.Cuts','--subtract-soldermask','--output',str(stage)+'/',name))
  commands.append(run('pcb','export','drill','--format','excellon','--excellon-units','mm','--excellon-separate-th','--generate-map','--map-format','pdf','--generate-report','--output',str(stage)+'/',name))
  commands.append(run('pcb','export','pos','--format','csv','--units','mm','--side','both','--exclude-dnp','--output',stage/(stem+'-all-positions.csv'),name))
  job=json.loads((stage/(stem+'-job.gbrjob')).read_text())
  functions=[item['FileFunction'] for item in job['FilesAttributes']]
  required={'Copper,L1,Top','Copper,L2,Inr','Copper,L3,Inr','Copper,L4,Bot','SolderPaste,Top','SolderPaste,Bot','Legend,Top','Legend,Bot','SolderMask,Top','SolderMask,Bot','Profile'}
  if set(functions)!=required or len(functions)!=len(required) or job['GeneralSpecs']['LayerNumber']!=4:
   raise ValueError('CAM Gerber job does not contain exactly the intended four-layer outputs')
  for item in job['FilesAttributes']:
   content=(stage/item['Path']).read_text()
   function=item['FileFunction'].replace('SolderPaste,','Paste,').replace('SolderMask,','Soldermask,')
   if '%MOMM*%' not in content or not content.rstrip().endswith('M02*') or '%TF.FileFunction,'+function not in content:
    raise ValueError('Invalid Gerber format/function: '+item['Path'])
  drill_hits={}
  for kind in ('PTH','NPTH'):
   content=(stage/(stem+'-'+kind+'.drl')).read_text()
   if 'METRIC' not in content or not content.startswith('M48') or not content.rstrip().endswith('M30'):
    raise ValueError('Invalid Excellon drill output: '+kind)
   drill_hits[kind]=len(re.findall(r'^X[-\d.]+Y[-\d.]+$',content,re.M))
  expected_hits={'PTH':sum(isinstance(t,pcb.PCB_VIA) for t in board.GetTracks()),'NPTH':0}
  for footprint in board.GetFootprints():
   for pad in footprint.Pads():
    if pad.GetDrillSize().x:
     expected_hits['NPTH' if pad.GetAttribute()==pcb.PAD_ATTRIB_NPTH else 'PTH']+=1
  if drill_hits!=expected_hits:raise ValueError(f'CAM drill hit count differs: {drill_hits} != {expected_hits}')
  expected={f.GetReference():(*xy(f.GetPosition()),f.GetOrientationDegrees(),'bottom' if f.GetLayer()==pcb.B_Cu else 'top') for f in board.GetFootprints() if not f.GetAttributes() & pcb.FP_EXCLUDE_FROM_POS_FILES and not f.IsDNP()}
  smt=placement_rows((stage/(stem+'-all-positions.csv')).read_text(),expected)
  with (stage/(stem+'-top-smt-positions.csv')).open('w') as file:
   writer=csv.DictWriter(file,fieldnames=['Ref','Val','Package','PosX','PosY','Rot','Side'],lineterminator='\n');writer.writeheader();writer.writerows(smt)
  with (directory/'bom.csv').open() as file:rows=list(csv.DictReader(file))
  for row in rows:
   if row['Reference'].startswith('TP'):row['Assembly']='PCB feature; do not populate'
  with (stage/(stem+'-bom.csv')).open('w') as file:
   writer=csv.DictWriter(file,fieldnames=list(rows[0]),lineterminator='\n');writer.writeheader();writer.writerows(rows)
  manifest={'status':'PROTOTYPE CAM FOR REVIEW; NOT APPROVED FOR ORDER OR POWERED ASSEMBLY','key_count':62,'kicad_version':summary['kicad_version'],'source_sha256':{str(p.relative_to(ROOT)):hashlib.sha256(p.read_bytes()).hexdigest() for p in [directory/(stem+ext) for ext in ('.kicad_pro','.kicad_sch','.kicad_pcb')]+[directory/'bom.csv',directory/'circuit.json',directory/'controller-seed.json',directory/'route-manifest.json',directory/'reports/validation.json',Path(__file__)]},'checks':{'erc':summary['erc_count'],'drc':summary['drc_count'],'unconnected':summary['unconnected_item_count'],'parity':summary['schematic_parity_count'],'drill_hits':drill_hits,'all_positions':len(expected),'top_smt_positions':len(smt)},'stackup':summary['stackup_review'],'coordinates':'Absolute KiCad origin; Gerber/drill/placement mm. Position CSV has +X right, +Y up (negative PCB Y), bottom X is NOT mirrored. Rotations are native KiCad; assembler must approve transforms. Switch positions use pin-1 anchors, not stems.','assembly':'BOM candidates remain provisional. TP rows are copper features. DB1 is an external assembly, absent from placement. All-position CSV includes THT switches; top-SMT CSV excludes THT switches and test pads. Every populated SMT component is top-side.','open_gates':['Actual module identity, sensor-end connector and conditional +3V3 harness verification','All solder joints and switch metal/plastic tips must be trimmed and measured <=0.82mm below the PCB; standard untrimmed MX leads exceed this. Insulate from conductive case. Case depth, plate relief and cable clearances remain to be measured','Fabricator-approved stackup, finish, solder mask and controlled-impedance geometry','QFN exposed-pad via/paste, via tenting/filling and assembler rotation review','Final passive procurement, capacitor derating and crystal startup','USB inrush/startup/suspend, regulator heat, SPI levels and USB waveform bench tests'],'commands':[[str(x).replace(str(stage),'CAM_OUTPUT').replace(str(ROOT),'PROJECT_ROOT') for x in command] for command in commands]}
  manifest['files_sha256']={p.name:hashlib.sha256(p.read_bytes()).hexdigest() for p in sorted(stage.iterdir())}
  manifest['procurement_specs']={'resistors':'0402, stated resistance, 1% tolerance, >=50V; manufacturer/series not selected','capacitors':'Stated capacitance and footprint; >=10V X7R for nF/uF, >=50V C0G for pF; verify effective capacitance under DC bias','switches':'62 PCB-mount Cherry MX compatible solder switches, not hot-swap; exact variant remains user choice','external_parts':'External C5-JSH assembly and contact-number 1:1 JST-SH cable; JST GH GHR-07V-S host sensor housing, sensor-end connector unidentified','status':'Generic minimum procurement candidates, not approved manufacturer part numbers or assembly substitutions'}
  (stage/'manifest.json').write_text(json.dumps(manifest,indent=2)+'\n')
  output.mkdir(exist_ok=True)
  prior=json.loads((output/'manifest.json').read_text()).get('files_sha256',{}) if (output/'manifest.json').exists() else {}
  for path in output.iterdir():
   if path.name in prior and not (stage/path.name).exists():path.unlink()
  for path in stage.iterdir():(output/path.name).write_bytes(path.read_bytes())
  archive=directory/(stem+'-prototype-cam.zip')
  with zipfile.ZipFile(archive,'w',compression=zipfile.ZIP_DEFLATED) as bundle:
   for path in sorted(stage.iterdir()):bundle.writestr(zipfile.ZipInfo('prototype-cam/'+path.name,date_time=(2026,10,31,0,0,0)),path.read_bytes())
 print('Prototype CAM checked and exported: '+str(archive))
ROOT=Path(__file__).resolve().parents[1]
parser=argparse.ArgumentParser();parser.add_argument('--draft',action='store_true');parser.add_argument('--keys',type=int,choices=VARIANTS,default=65);parser.add_argument('--prototype-cam',action='store_true',help='Export checked 62-key prototype Gerber/drill/BOM/placement bundle; not fabrication approval');args=parser.parse_args()
if args.prototype_cam and (args.keys!=62 or args.draft):parser.error('--prototype-cam requires --keys 62 and strict validation (no --draft)')
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
check(('J2' in fps)==(args.keys==62),'Sensor J2 must be present only on the 62-key draft')
if 'J2' in fps:
 j=fps['J2']
 check(sensor_connector_matches({p.GetNumber():p.GetNetname() for p in j.Pads()}),'J2 differs from the 3V3 Keyball contact map')
 check(j.GetFPID().GetLibItemName()=='JST_GH_SM07B-GHS-TB_1x07-1MP_P1.25mm_Horizontal' and j.GetLayer()==pcb.F_Cu,'Wrong J2 footprint or side')
if 'J1' in fps:
 j=fps['J1']
 check(usb_connector_matches({p.GetNumber():p.GetNetname() for p in j.Pads()}),'J1 differs from the UDB contact pinout')
 check(j.GetFPID().GetLibItemName()=='JST_SH_SM04B-SRSS-TB_1x04-1MP_P1.00mm_Horizontal','Wrong J1 connector footprint')
 check(j.GetLayer()==(pcb.F_Cu if args.keys==62 else pcb.B_Cu) and abs(j.GetOrientationDegrees()-90)<.00001,'J1 side or orientation differs from selected variant')
 check(all(abs(actual-expected)<1e-5 for actual,expected in zip(xy(j.GetPosition()),expectedparts['J1']['pos'])),'J1 position differs from the circuit placement')
if args.keys==62:
 check(all(f.GetLayer()==pcb.F_Cu for ref,f in fps.items() if not ref.startswith('TP')),'Shallow 62-key PCB must keep every populated component on top')
 check(abs(pcb.ToMM(b.GetDesignSettings().GetBoardThickness())-layout['rear_clearance']['prototype_stack_mm']['pcb_thickness'])<1e-5,'PCB nominal thickness differs from shallow assembly stack')
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
 check(fps[f'D{i}'].GetLayer()==(pcb.F_Cu if args.keys==62 else pcb.B_Cu),'Diode on wrong side '+k['id'])
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
# Read the actual contour, independently of the generator's dimension metadata.
edges=[x for x in b.GetDrawings() if x.GetLayer()==pcb.Edge_Cuts]
edge_geometry=[]
for e in edges:
 check(e.GetShape() in (pcb.SHAPE_T_SEGMENT,pcb.SHAPE_T_ARC),'Unexpected Edge.Cuts primitive')
 record={'start':xy(e.GetStart()),'end':xy(e.GetEnd())}
 if e.GetShape()==pcb.SHAPE_T_ARC:record['mid']=xy(e.GetArcMid())
 edge_geometry.append(record)
check(outline_matches(edge_geometry,args.keys),'Board outline or 48x48 R1 inner opening differs from draft')
stackup=stackup_layers((D/(NAME+'.kicad_pcb')).read_text())
check(b.GetCopperLayerCount()==4 and stackup_matches(stackup),'Native copper/dielectric stackup differs from four-layer 7628 candidate')
opening=layout[variant].get('pcb_opening')
check((opening is not None)==(args.keys==62),'Opening metadata missing or on wrong variant')
if opening:
 check(opening['bounds_mm']==[-24,-24,24,24] and opening['corner_radius_mm']==1,'Opening metadata differs from native contour requirement')
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
check(all(any(abs(pcb.ToMM(t.GetWidth())-width)<1e-5 for width in ((.15,.29) if args.keys==62 else (.29,))) for t in usb_tracks),'Connector-side USB width differs from selected routing study')
check(all(sum(pcb.ToMM(t.GetLength()) for t in usb_tracks if t.GetNetname()==net and t.GetLayer()==pcb.F_Cu)>(15 if args.keys==62 else 130) for net in usb_names),'Missing F.Cu USB route')
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
check(len(usb_vias)==(2 if args.keys==62 else 8) and all(d<=3 for d in via_ground_distances),'USB transitions require nearby ground vias within 3mm study rule')
summary={'status':('validation failed' if errs or viol or (not args.draft and unconn) else 'reviewable draft; NOT manufacturing approval'),'key_count':len(keys),'layout_variant':variant,'keepout_mm':circuit['keepout_mm'],'kicad_version':pcb.GetBuildVersion(),'erc_count':len(eitems),'erc_by_type':dict(Counter(v['type'] for v in eitems)),'drc_count':len(viol),'drc_by_type':dict(Counter(v['type'] for v in viol)),'drc_by_severity':dict(Counter(v['severity'] for v in viol)),'unconnected_item_count':len(unconn),'unconnected_nets':sorted({i['description'].split('[')[1].split(']')[0] for v in unconn for i in v['items'] if '[' in i['description']}),'schematic_parity_count':len(parity),'independent_netlist_mismatch_count':len(netdiff),'envelope_bounds_mm':bounds,'switch_count':len(set(fps)&switch_refs),'diode_count':len(set(fps)&diode_refs),'footprint_count':len(fps),'net_count':len(list(b.GetNetInfo().NetsByNetcode()))-1,'track_segment_count':sum(not isinstance(t,pcb.PCB_VIA) for t in b.GetTracks()),'via_count':sum(isinstance(t,pcb.PCB_VIA) for t in b.GetTracks()),'signal_tracks_on_ground_layer':sum(t.GetLayer()==pcb.In1_Cu and t.GetNetname() not in ('GND','+3V3','+1V1') for t in b.GetTracks() if not isinstance(t,pcb.PCB_VIA)),'keepout_intersections':sorted(set(bad)),'structural_errors':errs,'native_export_readback':readback,'commands':commands,'artifact_sha256':{p.name:hashlib.sha256(p.read_bytes()).hexdigest() for p in [D/(NAME+ext) for ext in ['.kicad_pro','.kicad_sch','.kicad_pcb']]}}
summary['usb_reference_review']={'f_cu_widths_mm':sorted({round(pcb.ToMM(t.GetWidth()),6) for t in usb_tracks if t.GetLayer()==pcb.F_Cu}),'minimum_f_cu_pair_edge_gap_mm':round(pair_gap,6),'uncovered_projection_mm2':uncovered,'transition_exclusion_radius_mm':.51,'maximum_signal_via_to_ground_via_mm':round(max(via_ground_distances,default=0),6),'note':'Saved refilled In1 polygons, make_valid for KiCad touching contours. Projection only; .51mm disks exclude signal-via antipads. 3mm stitching rule is a project study condition, not USB compliance. Finite plane edges, transitions and actual impedance still require qualification. DRAFT H has a local .15mm D+ resistor escape and USB test-pad branches; the uniform .29mm impedance estimate does not apply to these discontinuities.'}
if args.keys==62:
 summary['shallow_assembly']={'assumed_stack_mm':layout[variant]['prototype_stack_mm'],'bottom_populated_components':[ref for ref,f in fps.items() if f.GetLayer()==pcb.B_Cu and not ref.startswith('TP')],'status':'All populated components top-side. Not a whole-assembly floor pass: stock switch leads/plastic posts, solder and insulation must be trimmed/measured to <=0.82mm below PCB. J2 needs the prototype plate relief; actual mating and cable path remain physical checks.'}
summary['ground_layer_tracks_by_net']=dict(Counter(t.GetNetname() for t in b.GetTracks() if not isinstance(t,pcb.PCB_VIA) and t.GetLayer()==pcb.In1_Cu))
summary['pcb_opening']=opening
summary['sensor_connector_pinout']={p.GetNumber():p.GetNetname() for p in fps['J2'].Pads()} if 'J2' in fps else None
summary['stackup_review']={'status':'Unqualified JLC04161H-7628 candidate; fabricator approval required','top_to_bottom':[{'name':name,'type':kind,'thickness_mm':thickness,'material':material,'epsilon_r':er} for name,kind,thickness,material,er in stackup],'nominal_board_thickness_mm':pcb.ToMM(b.GetDesignSettings().GetBoardThickness()),'copper_and_dielectric_sum_mm':round(sum(layer[2] for layer in stackup),6),'mask_and_finish':'Solder mask construction and finish unspecified. Copper/dielectric sum is 1.5862mm; nominal board thickness is 1.6mm. Do not infer the 0.0138mm difference as an approved mask allowance.'}
(R/'validation.json').write_text(json.dumps(summary,indent=2,ensure_ascii=False)+'\n');(R/'switch-coordinates.json').write_text(json.dumps(coord,indent=2)+'\n')
print(json.dumps({k:v for k,v in summary.items() if k not in ['commands','native_export_readback','artifact_sha256']},indent=2))
if args.prototype_cam and not (errs or viol or unconn):export_prototype_cam(D,b,summary)
if errs or viol or (not args.draft and unconn):raise SystemExit(1)
