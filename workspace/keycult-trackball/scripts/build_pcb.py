# /// script
# requires-python = ">=3.11"
# dependencies = ["sexpdata==1.0.2"]
# ///
"""Create the review seed using KiCad's installed symbols, footprints and pcbnew.

This deliberately small project generator is not a general CAD/routing engine.
Native CAD is editable; regeneration overwrites the seed. See design/pcb/README.md.
"""
from pathlib import Path
import argparse, copy, csv, json, math, os, sys, uuid, subprocess
import xml.etree.ElementTree as ET
import sexpdata as sx
from kicad_runtime import configure
configure()
import pcbnew as pcb

ROOT = Path(__file__).resolve().parents[1]
parser=argparse.ArgumentParser()
parser.add_argument('--keys',type=int,choices=(65,62),default=65)
args=parser.parse_args()
OUT = ROOT / 'design/pcb' / ('62' if args.keys==62 else '')
OUT.mkdir(parents=True,exist_ok=True)
(OUT/'reports').mkdir(exist_ok=True)
(ROOT/'.cache/pcb-tools').mkdir(parents=True,exist_ok=True)
NAME = f'keycult{args.keys}'
S = sx.Symbol
NAMESPACE = uuid.UUID('407a7896-387e-451d-87c9-48af58dcf00c')
def uid(s): return str(uuid.uuid5(NAMESPACE, s))
def tag(v, name): return isinstance(v, list) and v and str(v[0]) == name
def item(a, name): return next((x for x in a if tag(x, name)), None)
def txt(v): return sx.dumps(v)
def node(name, *args): return [S(name), *args]
def mm(v): return pcb.FromMM(v)
def pt(x,y): return pcb.VECTOR2I(mm(x),mm(y))

SYMDIR = Path(os.environ['KICAD10_SYMBOL_DIR'])
FPDIR = Path(os.environ['KICAD10_FOOTPRINT_DIR'])
SYMS = {}
def symbol(lib_id):
    if lib_id in SYMS: return SYMS[lib_id]
    lib,name = lib_id.split(':')
    data = sx.load(open(SYMDIR / (lib+'.kicad_sym')))
    def resolve(name):
        a = copy.deepcopy(next(x for x in data if tag(x,'symbol') and x[1]==name))
        ext = item(a,'extends')
        if ext:
            base=resolve(ext[1]); old=base[1]
            for sub in base:
                if tag(sub,'symbol'): sub[1]=sub[1].replace(old+'_',name+'_',1)
            overrides={x[1]:x for x in a if tag(x,'property')}
            base=[x for x in base if not (tag(x,'property') and x[1] in overrides)]
            base.extend(overrides.values()); base[1]=name; a=base
        return a
    a=resolve(name); a[1]=lib_id
    SYMS[lib_id]=a
    return a

def pins(lib):
    return [p for sub in symbol(lib) if tag(sub,'symbol') for p in sub if tag(p,'pin')]

parts=[]
def part(ref,lib,value,fp,nets,sch,pos=None,angle=0,side='B',mpn='',status='provisional sourcing',note=''):
    p=dict(ref=ref,lib=lib,value=value,fp=fp,nets={str(k):v for k,v in nets.items()},sch=sch,pos=pos,angle=angle,side=side,mpn=mpn,status=status,note=note,uuid=uid(ref))
    parts.append(p); symbol(lib); return p
RFP='Resistor_SMD:R_0402_1005Metric'; CFP='Capacitor_SMD:C_0402_1005Metric'
def res(ref,value,nets,sch,pos,angle=0): return part(ref,'Device:R',value,RFP,dict(enumerate(nets,1)),sch,pos,angle,mpn='TBD 0402 1% 50V',note='value fixed; manufacturer not selected')
def cap(ref,value,net,sch,pos,angle=0,fp=CFP): return part(ref,'Device:C',value,fp,{1:net,2:'GND'},sch,pos,angle,mpn='TBD X7R 10V' if 'p' not in value else 'TBD C0G 50V',note='voltage rating >=10V; capacitance under DC bias to verify')
layout=json.loads((ROOT/'design/preliminary/layout.json').read_text())
variant='preferred' if args.keys==65 else 'rear_clearance'
keys=layout[variant]['keys']
# Keep original switch/diode references when a matrix cell is removed.
refs={key_id:i for i,key_id in enumerate([f'r{r}c{c}' for r in range(5) for c in range(15) if r in (0,4) or c not in (6,7,8)],1)}
indexed_keys=[(refs[k['id']],k) for k in keys]
keepout=[155,75,205,125] if args.keys==65 else [151.425,52.7,208.575,128.575]
ROW_GPIO=[4,5,6,7,8]; COL_GPIO=[9,10,11,12,13,14,15,16,17,18,19,20,21,22,26]
# Reserve hardware SPI0 GP0/1/2/3 before assigning the twenty matrix pins.
gpio={0:'SENSOR_MISO_3V3',1:'SENSOR_CS_3V3',2:'SENSOR_SCLK_3V3',3:'SENSOR_MOSI_3V3'}
gpio.update({n:f'ROW{i}' for i,n in enumerate(ROW_GPIO)})
gpio.update({n:f'COL{i}' for i,n in enumerate(COL_GPIO)})
if args.keys==62:gpio[COL_GPIO[7]]=None  # No switches remain in COL7 on this variant.
gpio.update({23:'SENSOR_MOTION_3V3',24:'SENSOR_RESET_3V3'})
u1={}
for p in pins('MCU_RaspberryPi:RP2040'):
    num=item(p,'number')[1]; name=item(p,'name')[1]
    if name.startswith('GPIO'): u1[num]=gpio.get(int(name.split('/')[0][4:]))
    elif name in ['IOVDD','ADC_AVDD','VREG_VIN','USB_VDD']: u1[num]='+3V3'
    elif name in ['DVDD','VREG_VOUT']: u1[num]='+1V1'
    elif name in ['GND','TESTEN']: u1[num]='GND'
    else: u1[num]={'XIN':'XIN','XOUT':'XOUT','RUN':'RUN','SWCLK':'SWCLK','SWDIO':'SWDIO','USB_DM':'USB_DM_MCU','USB_DP':'USB_DP_MCU','~{QSPI_SS}':'QSPI_SS'}.get(name,name)
part('U1','MCU_RaspberryPi:RP2040','RP2040','Package_DFN_QFN:QFN-56-1EP_7x7mm_P0.4mm_EP3.2x3.2mm',u1,(100.33,106.68),(180,68.5),mpn='RP2040',note='Direct mounted QFN; exposed pad to GND; unused GP25/27/28/29 NC')
part('U2','Memory_Flash:W25Q16JVSS','W25Q16JVSSIQ','Package_SO:SOIC-8_5.3x5.3mm_P1.27mm',{1:'QSPI_SS',2:'QSPI_SD1',3:'QSPI_SD2',4:'GND',5:'QSPI_SD0',6:'QSPI_SCLK_FLASH',7:'QSPI_SD3',8:'+3V3'},(215.9,83.82),(194.5,68.5),mpn='W25Q16JVSSIQ',note='208 mil SOIC, NOT 150mil package; QMK RP2040 flash boot stage must match')
part('U3','Regulator_Linear:AP2112K-3.3','AP2112K-3.3TRG1','Package_TO_SOT_SMD:SOT-23-5',{1:'VBUS_FUSED',2:'GND',3:'VBUS_FUSED',4:None,5:'+3V3'},(330.2,73.66),(158,68.5),mpn='AP2112K-3.3TRG1',note='600mA part rating is NOT USB budget; 5V->3V3 thermal/load budget unresolved')
# UDB pin numbers are connector contacts, independent of the legacy test-pad numbering.
part('J1','Connector_Generic_MountingPin:Conn_01x04_MountingPin','USB / C5-JSH JST-SH','Connector_JST:JST_SH_SM04B-SRSS-TB_1x04-1MP_P1.00mm_Horizontal',{1:'VBUS_USB',2:'USB_DM',3:'USB_DP',4:'GND','MP':'GND'},(391.16,91.44),(116.8,112),angle=90,mpn='JST SM04B-SRSS-TB(LF)(SN)',status='C5-JSH interface selected; mechanical fit provisional',note='1 protected USB power, 2 D-, 3 D+, 4 GND; 1:1 JST-SH cable of suitable length. External DB1 supplies CC/ESD/PTC/TVS; VBUS_USB is after its PTC and before main-board F1')
part('DB1','Connector_Generic:Conn_01x04','UDB C5-JSH / EXTERNAL','',{1:'VBUS_USB',2:'USB_DM',3:'USB_DP',4:'GND'},(375.92,63.5),mpn='Unified Daughterboard C5-JSH',status='user-approved design assumption; external assembly',note='Pinned 2b6c52aa2de6f9f60ca99c6250e4bdf01e8f4eea; 5.1k CC, TPD4E05U06DQAR ESD, ASMD1206-150 PTC, SMF6.0A TVS, direct shield/GND connection; no active sustained-overvoltage disconnect; not fitted on main PCB')
for i,(net,x) in enumerate([('VBUS_USB',145),('GND',148),('USB_DM',151),('USB_DP',154)],1):
    part(f'TP{i}','Connector:TestPoint',net+' / USB TEST','TestPoint:TestPoint_Pad_D1.5mm',{1:net},(300.99+17.78*i,111.76),(x,56),mpn='PCB pad',status='test access; J1 is the cable connector',note='Test-pad numbering differs from J1 contact numbering')
part('F1','Device:Polyfuse','500mA hold / provisional','Fuse:Fuse_0805_2012Metric',{1:'VBUS_USB',2:'VBUS_FUSED'},(300.99,139.7),(148,68),mpn='Bourns MF-PSMF050X-2',note='Retained local main-board PTC after C5-JSH PTC; total drop, load/inrush and trip coordination need verification; not a USB current entitlement')
cap('C1','1uF','VBUS_FUSED',(325.12,139.7),(153,68.5),fp='Capacitor_SMD:C_0603_1608Metric')
cap('C2','1uF','+3V3',(350.52,139.7),(162,68.5),fp='Capacitor_SMD:C_0603_1608Metric')
# One 100nF for each IOVDD group, ADC_AVDD, USB_VDD and each DVDD pin.
for i,(pos,net,pinnote) in enumerate([
 ((184.7,66.1),'+3V3','IOVDD1'),((184.7,70.1),'+3V3','IOVDD10'),
 ((179.7,73.5),'+3V3','IOVDD22'),((175.1,69.4),'+3V3','IOVDD33'),
 ((177.7,63.1),'+3V3','IOVDD42'),((181.8,63.1),'+3V3','IOVDD49'),
 ((175.7,63.1),'+3V3','ADC_AVDD43'),((179.7,63.1),'+3V3','USB_VDD48'),
 ((181.8,73.5),'+1V1','DVDD23'),((183.8,63.1),'+1V1','DVDD50'),
 ((171.5,66.5),'+3V3','VREG_VIN44'),((171.5,70.5),'+1V1','VREG_VOUT45'),
 ((200.5,68.5),'+3V3','QSPI flash VCC')],3):
    p=cap(f'C{i}','1uF' if i in (13,14) else '100nF',net,(30.48+25.4*(i-3),213.36),pos,angle=90)
    p['note']+='; '+pinnote
res('R1','27R',('USB_DM_MCU','USB_DM'),(419.1,73.66),(173.8,61.5),90)
res('R2','27R',('USB_DP_MCU','USB_DP'),(444.5,73.66),(171.5,61.5),90)
res('R3','10k',('+3V3','QSPI_SS'),(241.3,116.84),(194.5,73.6))
res('R4','1k',('QSPI_SS','BOOT_SW'),(215.9,139.7),(202.5,72.5))
res('R5','10k',('+3V3','RUN'),(139.7,139.7),(169,69.5))
res('R6','1k',('XOUT','XOUT_CRYSTAL'),(469.9,73.66),(186.5,71.8),90)
res('R7','27R',('QSPI_SCLK','QSPI_SCLK_FLASH'),(241.3,139.7),(188.2,66.1))
# 12MHz, CL=10pF; 15pF caps assume ~2.5pF parasitics and require oscillator bring-up.
part('Y1','Device:Crystal_GND24','ABM8-272-T3 12MHz CL10pF','Crystal:Crystal_SMD_3225-4Pin_3.2x2.5mm',{1:'XIN',2:'GND',3:'XOUT_CRYSTAL',4:'GND'},(520.7,73.66),(187.2,69),mpn='ABM8-272-T3',note='Confirm ESR/drive/startup on assembled board')
cap('C16','15pF','XIN',(500.38,116.84),(186.5,73.8))
cap('C17','15pF','XOUT_CRYSTAL',(541.02,116.84),(189,73.8))
for ref,net,x,schx in [('SW67','BOOT_SW',205.5,215.9),('SW68','RUN',165.5,139.7)]:
    part(ref,'Switch:SW_Push','BOOTSEL' if ref=='SW67' else 'RESET','Button_Switch_SMD:SW_SPST_TL3342',{1:net,2:'GND'},(schx,170.18),(x,68.5),mpn='E-Switch TL3342F160QG',note='Backside service access and switch height not mechanically verified')
for i,net in enumerate(['SWCLK','SWDIO','RUN','GND','+3V3'],5):
    part(f'TP{i}','Connector:TestPoint',net+' / SWD','TestPoint:TestPoint_Pad_D1.0mm',{1:net},(25.4+(i-5)*25.4,251.46),(133+(i-5)*3,69),mpn='PCB pad',status='electrically defined; access provisional')
for i,net in enumerate(['SENSOR_MISO_3V3','SENSOR_CS_3V3','SENSOR_SCLK_3V3','SENSOR_MOSI_3V3','SENSOR_MOTION_3V3','SENSOR_RESET_3V3','GND','+3V3'],10):
    part(f'TP{i}','Connector:TestPoint',net,'TestPoint:TestPoint_Pad_D1.0mm',{1:net},(584.2+(i-10)*27.94,160.02),(211.5,76+(i-10)*2.5),mpn='PCB pad',status='3V3 MCU-side logical pads ONLY',note='NO sensor connector, NO 5V sensor supply, NO approved module connection; level translation pending #2')
res('R8','10k',('+3V3','SENSOR_CS_3V3'),(609.6,116.84),(215,68.5))
for n,net in enumerate(['VBUS_FUSED','GND'],1):
    part(f'#FLG0{n}','power:PWR_FLAG','PWR_FLAG','',{1:net},(365.76+25.4*n,139.7),status='external USB supply assertion; not a fitted component')
for i,k in indexed_keys:
    x,y=180+k['x_mm'],100+k['y_mm']; r,c=k['row'],k['column']; nodekey='K_'+k['id']
    sx0,sy0=27.94+50.8*c,309.88+45.72*r
    part(f'SW{i}','Switch:SW_Push',k['id']+' '+k['label'],f'Button_Switch_Keyboard:SW_Cherry_MX_{k["width_u"]:.2f}u_PCB',{1:f'COL{c}',2:nodekey},(sx0,sy0),(x+2.54,y-5.08),side='F',mpn='Cherry MX PCB-mount solder; exact variant TBD',note='Stock footprint origin is pin1, NOT stem; stem offset (-2.54,+5.08)mm')
    part(f'D{i}','Device:D','1N4148W','Diode_SMD:D_SOD-123',{1:f'ROW{r}',2:nodekey},(sx0,sy0+15.24),(x-4,y+6.6),mpn='1N4148W',note='Cathode pad1 to row; QMK COL2ROW; '+k['id'])


# Placement tuning against real stock drill and courtyard geometry.
placements={
 'TP1':(145,53.8),'R2':(170.5,64.8),'D10':(216,73.4),'U1':(180,69.8),'U2':(198,68.5),'Y1':(171,68.5),
 'R1':(185.6,68.4),'R2':(185.6,69.6),'C3':(185.3,66.7),'C4':(185.3,71.5),'C5':(178,74.4),
 'C6':(174.7,67.6),'C7':(177.1,64.4),'C8':(182.8,64.4),
 'C9':(174,64.4),'C10':(180,64.6),'C11':(181,74.4),'C12':(185.3,64.2),
 'C13':(167,64.8),'C14':(166.5,72),'C15':(204.5,73.2),
 'R1':(185.6,68.4),'R2':(185.6,69.6),'R3':(198,73.6),'R4':(207,73.3),
 'R5':(162,65.8),'R6':(174.2,70.8),'R7':(191.5,73.7),
 'C16':(168.9,72),'C17':(171,72),'SW67':(208.5,68.5),
 'SW68':(128,68.5),'C2':(161.5,72.5),'U3':(157,68),
 'R8':(221,71.5),'TP5':(132,73),'TP6':(135,73),'TP7':(138,73),'TP8':(141,73),'TP9':(144,73),
 'D3':(153-76.2,73),'D4':(172-76.2,58),'D5':(211-76.2,73),'D6':(216-76.2,73.4),
}
placements.pop('D10')
for p in parts:
 if p['ref'] in placements:p['pos']=placements[p['ref']]
 if p['ref']=='U1':p['angle']=90
 if p['ref'] in ('R1','R2'):p['angle']=0
 if p['ref'] in ('C5','C7','C8','C9','C10','C11','C12'):p['angle']=0
 if p['ref'].startswith('TP') and int(p['ref'][2:])>=10:
  p['pos']=(209,78+2.5*(int(p['ref'][2:])-10))
 # A four-pitch translation preserves switch-hole registration and frees the 62-key rear band.
 if p['fp'] and not (p['ref'].startswith('D') or (p['ref'].startswith('SW') and int(p['ref'][2:])<=66)):
  p['pos']=(p['pos'][0]-76.2,p['pos'][1])

# Place decouplers beside the actual rotated power pads, not the schematic pin order.
local_caps={'C3':(108.6,73.5),'C4':(102.8,74.6),'C5':(98.1,67.7),
 'C6':(102.8,64.8),'C7':(106.4,64.8),'C8':(111.5,69.8),
 'C9':(108.7,65.6),'C10':(109.4,69.8),'C11':(98.1,69.0),'C12':(109.4,71.0),
 'C13':(109.4,67.4),'C14':(110.0,68.6)}
for p in parts:
 if p['ref'] in local_caps:
  p['pos']=local_caps[p['ref']];p['angle']=0
 if p['ref'] in ('R1','R2'):p['pos']=(114,67.5 if p['ref']=='R1' else 68.7)
 if p['ref'] in ('TP3','TP4'):p['pos']=(113.9 if p['ref']=='TP3' else 117.1,54.5)

# Embed the stock symbols so the native schematic is self-contained.
rootuuid=uid('schematic')
sch=[S('kicad_sch'),node('version',20250114),node('generator','eeschema'),node('uuid',rootuuid),node('paper','A1'),node('title_block',node('title',f'Keycult{args.keys} / electrical review draft'),node('date','2026-10-30'),node('rev','DRAFT-E'),node('comment',1,'C5-JSH / 7628 STACKUP ASSUMED - NOT FOR FABRICATION')),node('lib_symbols',*SYMS.values())]
def effect(size=1.27,just=None,hide=False):
    a=node('effects',node('font',node('size',size,size)))
    if just:a.append(node('justify',S(just)))
    if hide:a.append(S('hide'))
    return a
for p in parts:
    x,y=p['sch']; lib=symbol(p['lib'])
    sym=node('symbol',node('lib_id',p['lib']),node('at',x,y,0),node('unit',1),node('in_bom',S('no' if (not p['fp'] and p['ref']!='DB1') or p['ref'].startswith('TP') else 'yes')),node('on_board',S('no' if not p['fp'] else 'yes')),node('dnp',S('no')),node('uuid',p['uuid']))
    for n,v,dy,hide in [('Reference',p['ref'],-4.1,False),('Value',p['value'],-1.6,False),('Footprint',p['fp'],0,True),('Datasheet','',0,True)]:
        prop=next((q for q in lib if tag(q,'property') and q[1]==n),None)
        pos=item(prop,'at') if prop else None
        tx,ty=(x+pos[1],y-pos[2]) if pos and not hide else (x,y+dy)
        if p['lib'] in ('Device:R','Device:C','Device:Polyfuse') and n in ('Reference','Value'):
            tx,ty=x+5.08,y+(-1.27 if n=='Reference' else 1.27)
        if p['ref']=='J1' and n=='Value':tx,ty=x+2.54,y-1.27
        if p['ref']=='F1' and n in ('Reference','Value'):tx=x+2.54
        left=(p['ref']=='J1' and n=='Value') or (p['ref']=='F1' and n in ('Reference','Value'))
        size=0.9 if p['ref']=='F1' and n=='Value' else 1.0
        sym.append(node('property',n,v,node('at',round(tx,4),round(ty,4),0),effect(size,'left' if left else None,hide=hide)))
    sym.append(node('instances',node('project',NAME,node('path','/'+rootuuid,node('reference',p['ref']),node('unit',1)))))
    sch.append(sym)
    for pin in pins(p['lib']):
        num=item(pin,'number')[1]; a=item(pin,'at'); px,py=x+a[1],y-a[2]; ang=a[3]
        net=p['nets'].get(num)
        if net and net.startswith('K_'):continue
        if net is None:
            sch.append(node('no_connect',node('at',round(px,4),round(py,4)),node('uuid',uid(p['ref']+'NC'+num))))
            continue
        # Extend away from symbol body, on the 1.27mm schematic grid.
        dx,dy={0:(-3.81,0),180:(3.81,0),90:(0,3.81),270:(0,-3.81)}[ang]
        qx,qy=round(px+dx,4),round(py+dy,4)
        sch.append(node('wire',node('pts',node('xy',round(px,4),round(py,4)),node('xy',qx,qy)),node('stroke',node('width',0),node('type',S('default'))),node('uuid',uid(p['ref']+'wire'+num))))
        # Labels on vertical power pins rotate to avoid overlap between adjacent supply pins.
        la=90 if dx==0 else 0
        sch.append(node('global_label',net,node('shape',S('input')),node('at',qx,qy,la),effect(0.9,'right' if (dx<0 or dy>0) else 'left'),node('uuid',uid(p['ref']+'label'+num))))
# Visible switch-to-diode wires make each key's series current path reviewable.
for i,k in indexed_keys:
 x,y=27.94+50.8*k['column'],309.88+45.72*k['row']
 ps=[(x+5.08,y),(x+12.7,y),(x+12.7,y+15.24),(x+3.81,y+15.24)]
 for j,(a,b) in enumerate(zip(ps,ps[1:])):
  sch.append(node('wire',node('pts',node('xy',round(a[0],4),round(a[1],4)),node('xy',round(b[0],4),round(b[1],4))),node('stroke',node('width',0),node('type',S('default'))),node('uuid',uid('series'+str(i)+'_'+str(j)))))
 sch.append(node('global_label','K_'+k['id'],node('shape',S('input')),node('at',round(x+12.7,4),round(y,4),0),effect(.9,'left'),node('uuid',uid('series-label'+str(i)))))
for text,x,y,size in [
 ('1 / RP2040 + FLASH + BOOT + RESET',25.4,25.4,3),('2 / USB DAUGHTERBOARD + 3V3 POWER',292.1,25.4,3),
 ('3 / CLOCK + SENSOR MCU-SIDE ONLY',492.76,25.4,3),
 ('No module connector or 5V sensor connection. Verify revision and level translation before connection.',551.18,190.5,1.5),
 ('DECOUPLING: individual IOVDD / USB / ADC / DVDD pins; VREG 1uF input + output',25.4,195.58,1.5),
 ('4 / 5 ROW x 15 COLUMN MATRIX: COL -> switch -> diode ANODE -> CATHODE -> ROW (COL2ROW)',25.4,284.48,2.2),
 (f'{len(keys)} keys; Keycult{args.keys}. Bottom M1/M2: 1.5u each. Central reservation is NOT A HOLE.',25.4,294.64,1.5),
 ('DB1: EXTERNAL C5-JSH assumed; CC 5.1k x2 / TPD4E05U06 ESD / PTC / SMF6.0A TVS.',292.1,35.56,1.2),
 ('J1: 1=protected USB power 2=D- 3=D+ 4=GND; 1:1 cable. Main-board F1 retained.',292.1,43.18,1.2),
 ('Shield joins GND on DB1. No active sustained-overvoltage disconnect. Mechanical fit provisional.',292.1,50.8,1.2)]:
 sch.append(node('text',text,node('at',x,y,0),effect(size,'left'),node('uuid',uid('text'+text))))
(OUT/(NAME+'.kicad_sch')).write_text(txt(sch)+'\n')

subprocess.run(['kicad-cli','sch','export','netlist','--format','kicadxml','-o',str(OUT/'reports/netlist.xml'),str(OUT/(NAME+'.kicad_sch'))],check=True)
netroot=ET.parse(OUT/'reports/netlist.xml').getroot()
actualnets={(n.attrib['ref'],n.attrib['pin']): net.attrib['name'].replace('/', '{slash}') for net in netroot.find('nets') for n in net.findall('node')}
board=pcb.BOARD(); board.SetCopperLayerCount(4)
board.GetDesignSettings().SetCopperLayerCount(4)
board.SetLayerType(pcb.In1_Cu,pcb.LT_POWER)
netnames=sorted(set(actualnets.values()))
nets={}
for i,name in enumerate(netnames,1):
 n=pcb.NETINFO_ITEM(board,name,i); board.Add(n); nets[name]=n
fps={}
for p in parts:
 if not p['fp']: continue
 lib,name=p['fp'].split(':'); f=pcb.FootprintLoad(str(FPDIR/(lib+'.pretty')),name)
 if f is None: raise ValueError(p['fp'])
 f.SetReference(p['ref']); f.SetValue(p['value']); f.SetPath(pcb.KIID_PATH('/'+rootuuid+'/'+p['uuid']))
 board.Add(f)
 f.SetFPID(pcb.LIB_ID(lib,name)); f.SetPosition(pt(*p['pos']))
 if p['side']=='B':f.Flip(f.GetPosition(),False)
 f.SetOrientationDegrees(p['angle'])
 f.Reference().SetVisible(False); f.Value().SetVisible(False)
 # Explicit net for every electrically numbered pad; duplicate switch pads are common.
 for pad in f.Pads():
  net=actualnets.get((p['ref'],pad.GetNumber()))
  if net:pad.SetNet(nets[net])
 fps[p['ref']]=f

# Working envelope is a scalar GH60 reference, not any Keycult CAD contour.
# Updated after source inspection; a centred rectangle, no mounting-hole inheritance.
W,H=285.0,94.6
for a,b in [((180-W/2,100-H/2),(180+W/2,100-H/2)),((180+W/2,100-H/2),(180+W/2,100+H/2)),((180+W/2,100+H/2),(180-W/2,100+H/2)),((180-W/2,100+H/2),(180-W/2,100-H/2))]:
 s=pcb.PCB_SHAPE();s.SetShape(pcb.SHAPE_T_SEGMENT);s.SetStart(pt(*a));s.SetEnd(pt(*b));s.SetLayer(pcb.Edge_Cuts);s.SetWidth(mm(.05));board.Add(s)
zone=pcb.ZONE(board);zone.SetIsRuleArea(True);zone.SetLayerSet(pcb.LSET.AllCuMask());zone.SetDoNotAllowTracks(True);zone.SetDoNotAllowVias(True);zone.SetDoNotAllowPads(True);zone.SetDoNotAllowZoneFills(True);zone.SetDoNotAllowFootprints(True)
zone.Outline().NewOutline()
left,top,right,bottom=keepout
for x,y in [(left,top),(right,top),(right,bottom),(left,bottom)]:zone.Outline().Append(mm(x),mm(y))
board.Add(zone)

def xy(p):return (pcb.ToMM(p.x),pcb.ToMM(p.y))
def pad(ref,n):return next(p for p in fps[ref].Pads() if p.GetNumber()==str(n))
def track(net,points,layer=pcb.F_Cu,width=.25):
 for a,b in zip(points,points[1:]):
  if math.dist(a,b)<.00001:continue
  t=pcb.PCB_TRACK(board);t.SetStart(pt(*a));t.SetEnd(pt(*b));t.SetWidth(mm(width));t.SetLayer(layer);t.SetNet(nets[net]);board.Add(t)
def via(net,xy0):
 v=pcb.PCB_VIA(board);v.SetPosition(pt(*xy0));v.SetWidth(mm(.6));v.SetDrill(mm(.3));v.SetViaType(pcb.VIATYPE_THROUGH);v.SetLayerPair(pcb.F_Cu,pcb.B_Cu);v.SetNet(nets[net]);board.Add(v)
# Explicit ground fan-out connects bottom SMT pads to the inner reference plane.
ground_offsets={'C10':(.42,.5),'C12':(.82,.1),'C11':(.42,.6),'C16':(0,.8),'U1':(-.9625,0),
 'U2':(-.9,0),'U3':(-.8,0),'TP2':(1.1,0),'Y1':(0,.9)}
for ref,f in fps.items():
 for p in f.Pads():
  if p.GetNetname()!='GND':continue
  if ref=='J1' and p.GetNumber()=='MP':continue  # USB overlay ground patch connects both mounting pads.
  a=xy(p.GetPosition())
  if ref=='U1' and p.GetNumber()=='57':via('GND',a);continue
  dx,dy=ground_offsets.get(ref,(.72,0))
  if ref=='C15':dx,dy=0,-.72
  if ref=='Y1':dx,dy=(.85 if p.GetNumber()=='2' else -1),0
  if ref.startswith('SW'):dx,dy=0,-1.1
  b=(a[0]+dx,a[1]+dy);track('GND',[a,b],pcb.B_Cu,.15 if ref=='U1' else .25);via('GND',b)
# Connector-side USB is applied by usb_routing.py after native serialization.
track('USB_DM_MCU',[xy(pad('U1',46).GetPosition()),(107.8,68.4),(108.2,68),(111.8,68),(112.3,67.5),xy(pad('R1',1).GetPosition())],pcb.B_Cu,.15)
track('USB_DP_MCU',[xy(pad('U1',47).GetPosition()),(107.8,68.8),(108.2,69.2),(112.99,69.2),xy(pad('R2',1).GetPosition())],pcb.B_Cu,.15)
track('SWCLK',[xy(pad('U1',24).GetPosition()),(99.65,68.8),(99.4,68.55)],pcb.B_Cu,.15)
via('SWCLK',(99.4,68.55))
# Inner-side escape avoids the crowded USB/decoupler corridor outside these QFN pads.
track('SWDIO',[xy(pad('U1',25).GetPosition()),(101.4,68.4)],pcb.B_Cu,.15)
via('SWDIO',(101.4,68.4))
track('+1V1',[xy(pad('U1',45).GetPosition()),(106.2,68)],pcb.B_Cu,.15)
via('+1V1',(106.2,68))
track('+1V1',[xy(pad('C14',1).GetPosition()),(108.8,68.6)],pcb.B_Cu,.15)
via('+1V1',(108.8,68.6))
track('+1V1',[(106.2,68),(108.2,68),(108.8,68.6)],pcb.F_Cu,.25)
track('+3V3',[xy(pad('U1',44).GetPosition()),(107.95,67.6),(108.15,67.4),xy(pad('C13',1).GetPosition())],pcb.B_Cu,.15)
# All switch-diode series links, row buses and per-column trunks are real copper.
rowends={}; colends={}
for i,k in indexed_keys:
 x,y=180+k['x_mm'],100+k['y_mm'];r,c=k['row'],k['column'];kn='K_'+k['id']
 if r==0 and c in range(1,6):continue
 a=xy(pad(f'SW{i}',2).GetPosition());b=xy(pad(f'D{i}',2).GetPosition()); cath=xy(pad(f'D{i}',1).GetPosition())
 track(kn,[a,(a[0],y+3.5),(b[0],y+3.5),b],pcb.B_Cu)
 via(f'ROW{r}',(cath[0],y+7.8));track(f'ROW{r}',[cath,(cath[0],y+7.8)],pcb.B_Cu)
 rowends.setdefault(r,[]).append((cath[0],y+7.8))
 a=xy(pad(f'SW{i}',1).GetPosition());branch=(x+7,y-5.08)
 track(f'COL{c}',[a,branch],pcb.B_Cu);colends.setdefault(c,[]).append(branch)
for r,pts in rowends.items():
 pts.sort()
 for a,b in zip(pts,pts[1:]):
  if (r==0 and a[0]<145) or (top<a[1]<bottom and a[0]<left and b[0]>right):
   pass # Detailed router bridges the protected centre on available layers.
  else:track(f'ROW{r}',[a,b])
for c,pts in colends.items():
 pts.sort(key=lambda a:a[1])
 if c in (6,7,8):
  pass # Central columns need detours around the keepout; detailed router owns them.
 else:track(f'COL{c}',pts,pcb.B_Cu)
# Signal ground reference: continuous inner plane outside the centre reservation.
g=pcb.ZONE(board);g.SetLayer(pcb.In1_Cu);g.SetNet(nets['GND']);g.SetLocalClearance(mm(.2));g.SetPadConnection(pcb.ZONE_CONNECTION_FULL);g.Outline().NewOutline()
for x,y in [(38,53.2),(322,53.2),(322,146.8),(38,146.8)]:g.Outline().Append(mm(x),mm(y))
board.Add(g)
# Local ground and support routing are performed by the detailed router.
# Board notes are on fabrication/documentation layers, not over copper/silkscreen.
for text0,xy0,size in [(f'DRAFT E / {len(keys)} KEYS - NOT FOR FABRICATION',(180,150),1.5),('RESERVED / NO CUTOUT',(180,100),1),('USB F.Cu 0.29/0.20mm; 7628 STACKUP STUDY / IMPEDANCE NOT QUALIFIED',(85,50),.8),('SENSOR 3V3 LOGIC ONLY / NO MODULE CONNECTOR',(98,98),.9)]:
 t=pcb.PCB_TEXT(board);t.SetText(text0);t.SetPosition(pt(*xy0));t.SetTextSize(pt(size,size));t.SetTextThickness(mm(.15));t.SetLayer(pcb.Dwgs_User);board.Add(t)
for i,k in indexed_keys:
 t=pcb.PCB_TEXT(board);t.SetText(k['id']);t.SetPosition(pt(180+k['x_mm'],100+k['y_mm']+8.2));t.SetTextSize(pt(.8,.8));t.SetTextThickness(mm(.12));t.SetLayer(pcb.F_SilkS);board.Add(t)
board.SetFileName(str(OUT/(NAME+'.kicad_pcb')))
pcb.SaveBoard(str(OUT/(NAME+'.kicad_pcb')),board)
from usb_routing import apply_usb_route
apply_usb_route(OUT/(NAME+'.kicad_pcb'))
# Explicit review rules: not a claim about a selected fabricator's process or stackup.
pro={'meta':{'filename':NAME+'.kicad_pro','version':1},'board':{'design_settings':{'rules':{'min_clearance':.15,'min_track_width':.15,'min_via_diameter':.55,'min_through_hole_diameter':.3,'min_hole_clearance':.25,'min_copper_edge_clearance':.25},'rule_severities':{}}},'net_settings':{'classes':[{'name':'Default','clearance':.15,'track_width':.25,'via_diameter':.6,'via_drill':.3,'diff_pair_width':.2,'diff_pair_gap':.15,'diff_pair_via_gap':.25}], 'meta':{'version':4}},'erc':{'erc_exclusions':[],'rule_severities':{}},'schematic':{'drawing':{}},'text_variables':{'STATUS':'DRAFT_NOT_FOR_FABRICATION'}}
pro['net_settings']['classes'].append({'name':'USB','clearance':.15,'track_width':.29,'via_diameter':.6,'via_drill':.3,'diff_pair_width':.29,'diff_pair_gap':.2,'diff_pair_via_gap':.2})
pro['net_settings']['netclass_patterns']=[{'netclass':'USB','pattern':net} for net in ('USB_DM','USB_DP')]
(OUT/(NAME+'.kicad_pro')).write_text(json.dumps(pro,indent=2)+'\n')
(OUT/'sym-lib-table').write_text('(sym_lib_table\n'+''.join(f'  (lib (name "{lib}")(type "KiCad")(uri "${{KICAD10_SYMBOL_DIR}}/{lib}.kicad_sym")(options "")(descr "KiCad stock"))\n' for lib in sorted({p['lib'].split(':')[0] for p in parts}))+')\n')
(OUT/'fp-lib-table').write_text('(fp_lib_table\n'+''.join(f'  (lib (name "{lib}")(type "KiCad")(uri "${{KICAD10_FOOTPRINT_DIR}}/{lib}.pretty")(options "")(descr "KiCad stock"))\n' for lib in sorted({p['fp'].split(':')[0] for p in parts if p['fp']}))+')\n')
(OUT/'circuit.json').write_text(json.dumps({'parts':parts,'rows':ROW_GPIO,'columns':COL_GPIO,'gpio':gpio,'origin_mm':[180,100],'envelope_mm':[W,H],'keepout_mm':keepout,'key_count':len(keys),'layout_variant':variant},ensure_ascii=False,indent=2)+'\n')
with (OUT/'bom.csv').open('w') as f:
 w=csv.writer(f,lineterminator='\n');w.writerow(['Reference','Value','Manufacturer part / candidate','Footprint','Assembly','Status','Notes'])
 for p in parts:
  if p['fp'] or p['ref']=='DB1':w.writerow([p['ref'],p['value'],p['mpn'],p['fp'],'External assembly' if p['ref']=='DB1' else ('THT solder' if p['side']=='F' else 'Bottom SMT'),p['status'],p['note']])
with (OUT/'gpio-map.csv').open('w') as f:
 w=csv.writer(f,lineterminator='\n');w.writerow(['key_id','label','switch','diode','matrix_row','matrix_col','row_gpio','column_gpio','x_mm','y_mm','diode_direction'])
 for i,k in indexed_keys:w.writerow([k['id'],k['label'],f'SW{i}',f'D{i}',k['row'],k['column'],f'GP{ROW_GPIO[k["row"]]}',f'GP{COL_GPIO[k["column"]]}',k['x_mm'],k['y_mm'],'COL2ROW'])
print(f'Generated {len(parts)} schematic symbols, {len(fps)} footprints, {len(nets)} nets, {len(list(board.GetTracks()))} tracks/vias with KiCad {pcb.GetBuildVersion()}')
