#!/usr/bin/env python3
"""Reproducible preliminary layout, NOT fabrication CAD. stdlib only."""
from pathlib import Path
import json, math, html, itertools, xml.etree.ElementTree as ET
P=Path(__file__).resolve().parent
U=19.05
R=17.0
CAP=18.0
SW=14.0
KEEP=50.0
F=lambda v: round(v,6)
def boxdist(a,b):
    return math.hypot(max(abs(a['x_mm']-b['x_mm'])-(cap_width(a)+cap_width(b))/2,0),max(abs(a['y_mm']-b['y_mm'])-CAP,0))
def cap_width(k):return CAP+(k['width_u']-1)*U
def circle_rect_clearance(k,r,side):
    width=side+(k['width_u']-1)*U if side==CAP else side
    return math.hypot(max(abs(k['x_mm'])-width/2,0),max(abs(k['y_mm'])-side/2,0))-r
def square_clearance(k,side):
    # signed separation on a separating axis: positive clear; negative overlap.
    width=side+(k['width_u']-1)*U if side in (CAP,U) else side
    return max(abs(k['x_mm'])-(KEEP+width)/2,abs(k['y_mm'])-(KEEP+side)/2)
left=[['Esc','1','2','3','4','5'],['Tab','Q','W','E','R','T'],['Caps','A','S','D','F','G'],['Shift','Z','X','C','V','B'],['Ctrl','GUI','Alt','Fn','Space','Del']]
right=[['6','7','8','9','0','Bksp'],['Y','U','I','O','P','\\'],['H','J','K','L',';','Enter'],['N','M',',','.','/','Shift'],['Bksp','Space','Fn','Left','Down','Right']]
centers={3:{0:['Vol-','Mute','Vol+'],4:['M1','M3','M2']},2:{0:['Vol-','Vol+'],4:['M1','M2']}}
def make_layout(gap):
    cols=12+gap; keys=[]
    for row in range(5):
        for col in range(cols):
            if gap==3 and row==4 and col==7:continue
            if 6<=col<6+gap:
                if row not in (0,4):continue
                label=centers[gap][row][col-6]; group='center'
            elif col<6:label=left[row][col];group='left'
            else:label=right[row][col-6-gap];group='right'
            keys.append({'id':f'r{row}c{col}','row':row,'column':col,'label':label,'group':group,'x_mm':F((col-(cols-1)/2)*U),'y_mm':F((row-2)*U),'rotation_deg':0,'width_u':1,'height_u':1})
            if gap==3 and row==4 and col in (6,8):
                keys[-1].update(x_mm=F((-0.75 if col==6 else 0.75)*U),width_u=1.5)
    return {'name':('preferred-15u-3u-65' if gap==3 else 'alternative-14u-2u-64-not-fit-approved'),'key_count':len(keys),'center_gap_u':gap,'total_width_u':cols,'nominal_envelope_mm':{'width':F(cols*U),'height':F(5*U),'xmin':F(-cols*U/2),'ymin':F(-2.5*U)},'central_empty_cells_mm':{'width':F(gap*U),'height':F(3*U)},'keys':keys}
a=make_layout(3);b=make_layout(2)
rear=make_layout(3)
rear['name']='rear-clearance-15u-3u-62'
rear['keys']=[k for k in rear['keys'] if not (k['row']==0 and k['group']=='center')]
rear['key_count']=len(rear['keys'])
rear['central_empty_cells_mm']={'width':F(3*U),'height':F(4*U),'xmin':F(-1.5*U),'ymin':F(-2.5*U),'xmax':F(1.5*U),'ymax':F(1.5*U)}
rear['status']='mount/support study only; no cutout, larger ball or stiffness improvement specified'
metadata={'status':'preliminary; not fabrication-ready; exact case opening and z stack unmeasured','units':'mm','coordinate_system':{'origin':'center of keyfield and ball, home row between G and H','x':'right','y':'toward user/front/bottom row','z':'up from plate upper surface; z coordinates not yet assigned','case_alignment':'assumed concentric with exterior only for illustration; must register to physical plate datum'},'pitch_mm':U,'keycap_plan_assumption_mm':[CAP,CAP],'keycap_plan_width_formula':'18 + (width_u - 1) * 19.05 mm; height 18 mm','conservative_key_cell_mm':[U,U],'switch_cutout_proxy_mm':[SW,SW],'case_exterior_reference_mm':[294.3,111],'case_opening_mm':None,'plate_to_floor_mm':None,'official_plate_envelope_mm':[285.75,108.25,1.5],'trackball':{'center_mm':[0,0],'ball_diameter_mm':34,'ball_diameter_status':'primary source verified nominal','ball_center_height_above_plate_mm':None,'assembly_keepout_mm':[KEEP,KEEP],'assembly_keepout_status':'provisional planning reservation, not measured holder and not a plate/PCB cutout','holder_verified_envelope_mm':None,'sensor_board_outline_mm':[22,25],'sensor_board_outline_scope':'Keyball46 published 2022 Gerber; vertical installation','sensor_connector_envelope_mm':None},'preferred':a,'alternative':b}
evidence=P/'sources/keyball-evidence.json'
metadata['rear_clearance']=rear
if evidence.exists():metadata['trackball']['primary_evidence']=json.loads(evidence.read_text())
report={'status':'numeric 2D checks only; z, opening, tolerances, sockets, wiring and final mounting remain unverified','layouts':{}}
for lay in [a,b,rear]:
    ks=lay['keys'];pairs=list(itertools.combinations(ks,2))
    collisions=[(i['id'],j['id']) for i,j in pairs if abs(i['x_mm']-j['x_mm'])<(cap_width(i)+cap_width(j))/2 and abs(i['y_mm']-j['y_mm'])<CAP]
    swcollisions=[(i['id'],j['id']) for i,j in pairs if abs(i['x_mm']-j['x_mm'])<SW and abs(i['y_mm']-j['y_mm'])<SW]
    d={'keys':len(ks),'unique_centers':len({(k['x_mm'],k['y_mm']) for k in ks}),'keycap_pair_collisions':collisions,'switch_proxy_pair_collisions':swcollisions,'minimum_keycap_pair_gap_mm':F(min(boxdist(i,j) for i,j in pairs)),'ball_to_keycap_min_mm':F(min(circle_rect_clearance(k,R,CAP) for k in ks)),'ball_to_switch_proxy_min_mm':F(min(circle_rect_clearance(k,R,SW) for k in ks)),'50mm_square_to_keycap_min_signed_mm':F(min(square_clearance(k,CAP) for k in ks)),'50mm_square_to_full_cell_min_signed_mm':F(min(square_clearance(k,U) for k in ks)),'50mm_square_to_switch_proxy_min_signed_mm':F(min(square_clearance(k,SW) for k in ks)),'case_exterior_x_margin_to_grid_mm':F((294.3-lay['nominal_envelope_mm']['width'])/2),'case_exterior_y_margin_to_grid_mm':F((111-5*U)/2),'keycap_envelope_mm':[F(lay['nominal_envelope_mm']['width']-U+CAP),F(4*U+CAP)],'G_mm':next([k['x_mm'],k['y_mm']] for k in ks if k['label']=='G'),'H_mm':next([k['x_mm'],k['y_mm']] for k in ks if k['label']=='H')}
    assert not collisions and not swcollisions and d['unique_centers']==len(ks)
    assert d['ball_to_keycap_min_mm']>0
    if lay is not b:assert d['50mm_square_to_full_cell_min_signed_mm']>0
    report['layouts'][lay['name']]=d
(P/'validation.json').write_text(json.dumps(report,indent=2)+'\n')
# Public illustrations use dimensions only, never copied upstream outline vertices.
metadata['plate_visualization']='generic rectangular bounding envelope; not the official contour or case opening'
(P/'layout.json').write_text(json.dumps(metadata,ensure_ascii=False,indent=2)+'\n')
def esc(s):return html.escape(str(s))
def text(x,y,s,size=3.2,anchor='start',color='#172033'):
    return f'<text x="{x}" y="{y}" font-size="{size}" text-anchor="{anchor}" fill="{color}">{esc(s)}</text>'
def line(x1,y1,x2,y2,stroke='#64748b',dash=''):
    return f'<path d="M{x1},{y1} L{x2},{y2}" fill="none" stroke="{stroke}" stroke-width=".22"'+(f' stroke-dasharray="{dash}"' if dash else '')+'/>'
def rect(x,y,w,h,fill,stroke='#64748b',dash='',radius=0):
    return f'<rect x="{x}" y="{y}" width="{w}" height="{h}" rx="{radius}" fill="{fill}" stroke="{stroke}" stroke-width=".25"'+(f' stroke-dasharray="{dash}"' if dash else '')+'/>'
def dim(x1,x2,y,label):
    return line(x1,y,x2,y)+line(x1,y-1,x1,y+1)+line(x2,y-1,x2,y+1)+text((x1+x2)/2,y-1.5,label,3,'middle')
def render(lay,filename):
    parts=['<svg xmlns="http://www.w3.org/2000/svg" width="330mm" height="190mm" viewBox="-165 -88 330 190" role="img">','<title>Preliminary Keycult No.1/60 + Keyball ortholinear layout</title>','<desc>Millimetre-scale engineering illustration. Unknown case opening and vertical assembly depth. Not fabrication CAD.</desc>','<g font-family="Arial, sans-serif">',rect(-165,-88,330,190,'#ffffff','#ffffff')]
    parts+=[text(-157,-79,f"KEYCULT No.1/60 | {lay['key_count']} keys | {lay['total_width_u']}u / center {lay['center_gap_u']}u",5),text(-157,-72,'PRELIMINARY — not a machining or PCB file. All dimensions mm.',3.6)]
    parts+=[rect(-147.15,-55.5,294.3,111,'#f1f5f9','#64748b','1.5 1.5',2)]
    parts+=[rect(-142.875,-54.125,285.75,108.25,'#ffffff','#0f766e','2 1').replace('<rect ', '<rect id="plate-envelope" ', 1)]
    env=lay['nominal_envelope_mm']; parts+=[rect(env['xmin'],env['ymin'],env['width'],env['height'],'none','#94a3b8','1 1')]
    if lay is rear:
        empty=lay['central_empty_cells_mm']
        parts+=[rect(empty['xmin'],empty['ymin'],empty['width'],U,'#ede9fe','#7c3aed','1 1').replace('<rect ', '<rect id="rear-extension" ', 1)]
        parts+=[rect(empty['xmin'],empty['ymin'],empty['width'],empty['height'],'none','#7c3aed','1 1').replace('<rect ', '<rect id="rear-clearance-grid" ', 1)]
        parts+=[text(0,-40,'Mount / support study',2.8,'middle','#6d28d9'),text(0,-34,'+19.05 mm rear space',2.6,'middle','#6d28d9')]
    parts+=[line(-150,0,150,0,'#cbd5e1','1 2'),line(0,-58,0,58,'#cbd5e1','1 2')]
    parts+=[rect(-25,-25,50,50,'#fff7ed','#c2410c','1.4 1.2'),f'<circle cx="0" cy="0" r="17" fill="#bfdbfe" stroke="#1d4ed8" stroke-width=".45"/>',line(-2,0,2,0,'#1d4ed8'),line(0,-2,0,2,'#1d4ed8')]
    parts+=[text(0,-7,'BALL Ø34',3.4,'middle','#1d4ed8'),text(0,8,'(0, 0)',3.1,'middle','#1d4ed8'),text(0,21,'50 × 50 provisional',2.8,'middle','#9a3412')]
    for k in lay['keys']:
        x,y=k['x_mm'],k['y_mm'];fill='#f8fafc' if k['group']!='center' else '#e2e8f0'
        if k['label'] in ['G','H']:fill='#dbeafe'
        parts+=[rect(x-cap_width(k)/2,y-9,f'{cap_width(k):g}',18,fill,'#475569',radius=.8),rect(x-7,y-7,14,14,'none','#cbd5e1'),text(x,y+1.2,k['label'],3.1 if len(k['label'])>3 else 4,'middle'),text(x,y+6.6,f'{x:g},{y:g}',1.75,'middle','#64748b')]
    parts+=[dim(-147.15,147.15,-64,'294.30 exterior reference — NOT the usable opening'),dim(env['xmin'],env['xmin']+env['width'],64,f"{env['width']:.2f} nominal keyfield ({lay['total_width_u']} × 19.05)")]
    parts+=[dim(-lay['center_gap_u']*U/2,lay['center_gap_u']*U/2,74,f"{lay['center_gap_u']*U:.2f} central grid gap")]
    note='Purple: 57.15 x 76.20 empty grid, NOT a cutout; stiffness and Z UNKNOWN.' if lay is rear else 'Caps: 18 x 18 (1u), 27.525 x 18 (1.5u); switches: 14 x 14. Opening and Z UNKNOWN.'
    if lay is b:note='Keys: assumed 18 × 18 caps / 14 × 14 switch proxies. Inner opening and Z stack UNKNOWN.'
    parts+=[text(-157,84,'Green: 285.75 × 108.25 bounding reference ONLY — not the official contour.',3),text(-157,90,'Orange: reserved assembly box, NOT a verified holder outline or cutout.',3,'start','#9a3412'),text(-157,96,note,3)]
    parts+=['</g></svg>'];s='\n'.join(parts);ET.fromstring(s);(P/filename).write_text(s+'\n');return s
svg=render(a,'layout.svg');altsvg=render(b,'alternative.svg')
rearsvg=render(rear,'rear-clearance.svg')
section='''<svg xmlns="http://www.w3.org/2000/svg" width="170mm" height="90mm" viewBox="-55 -40 170 90"><g font-family="Arial,sans-serif"><rect x="-55" y="-40" width="170" height="90" fill="white"/><text x="-50" y="-32" font-size="4">Y–Z SECTION — illustrative, exact mounting height unknown</text><path d="M-50,0 H-28 M28,0 H100" fill="none" stroke="#0f766e" stroke-width="1.5"/><circle cx="0" cy="-8" r="17" fill="#bfdbfe" stroke="#1d4ed8" stroke-width=".3"/><path d="M-23,-4 V15 H23 V-4" fill="none" stroke="#c2410c" stroke-dasharray="2 1" stroke-width=".5"/><rect x="-27" y="-12" width="2" height="25" fill="#fed7aa" stroke="#c2410c" stroke-width=".3"/><path d="M-26,13 V17 H-19" fill="none" stroke="#c2410c" stroke-width=".5"/><path d="M-50,31 H100" fill="none" stroke="#64748b" stroke-width=".3" stroke-dasharray="2 1"/><path d="M0,-8 H65 M0,0 H65 M0,9 H65 M0,-25 H65" fill="none" stroke="#cbd5e1" stroke-width=".2"/><text x="36" y="-23" font-size="3">ball top = c + 17</text><text x="36" y="-10" font-size="3">ball center = c (UNKNOWN)</text><text x="36" y="-2" font-size="3">plate top z = 0</text><text x="36" y="12" font-size="3">ball bottom = c - 17</text><text x="36" y="21" font-size="3">vertical sensor PCB at rear (left): Z unknown</text><text x="36" y="29" font-size="3">floor z = -D (UNKNOWN)</text><text x="-50" y="42" font-size="3">Circle Ø34; example c=8 only. Sensor is VERTICAL, not under-ball horizontal.</text><text x="-50" y="47" font-size="3">Holder / sensor position / floor are schematic. No assembly pose is verified.</text></g></svg>'''
ET.fromstring(section);(P/'section.svg').write_text(section+'\n')
body='''<!doctype html><html lang="ko"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>Keycult No.1/60 중앙 Keyball 예비 배치</title><style>:root{color-scheme:light}body{margin:0;background:#f1f5f9;color:#172033;font:16px/1.6 system-ui,sans-serif}main{max-width:1500px;margin:24px auto;padding:0 24px}h1{font-size:26px;margin:0}p{margin:8px 0}.panel{background:white;padding:20px;border:1px solid #cbd5e1;margin:20px 0}svg{display:block;width:100%;height:auto}.notice{color:#9a3412}details{margin:20px 0}summary{cursor:pointer;font-weight:650}table{border-collapse:collapse}td,th{padding:5px 12px;border:1px solid #cbd5e1;text-align:left}a{color:#1d4ed8}@media print{body{background:white}main{max-width:none;margin:0;padding:0}.noprint,details,.section{display:none}.panel{border:0;padding:0;margin:0}svg{width:330mm;height:190mm}h1,p{font-size:11pt}@page{size:A3 landscape;margin:10mm}}</style></head><body><main><h1>Keycult No.1/60 + 중앙 34mm Keyball · 65키 예비안</h1><p>1u = 19.05mm · 15열 × 5행 · 중앙 3 × 3칸 확보 · G(-38.10, 0) / H(+38.10, 0)</p><p class="notice">제작 도면 아님. 케이스 내부 개구부, 홀더 조립 치수, 플레이트–바닥 거리 미확정. 외장 높이를 내부 여유로 사용하지 않았습니다.</p><div class="panel">'''+svg+'''</div><div class="noprint"><p><a href="design.md">근거·설계 설명</a> · <a href="layout.json">키 중심 JSON</a> · <a href="validation.json">수치 검증</a> · <a href="print-1to1.svg">1:1 인쇄용</a></p><p>청색: 볼 최대 평면 투영 Ø34. 주황색 50 × 50: 임시 조립 예약 영역이며 실제 홀더 외곽·가공 구멍이 아닙니다. 키 좌표는 키캡 안쪽 하단에 mm로 표기했습니다.</p><p>인쇄: A3 가로, 배율 100% / 실제 크기, 페이지 맞춤 끄기. 100mm 검정 기준선 실측 후 사용. A4 한 장에는 294.3mm 외장 폭과 여백을 1:1로 담기 어렵습니다.</p></div><details><summary>대안: 14u / 중앙 2u / 64키 — 홀더 적합성 미승인</summary>'''+altsvg+'''<p>볼만 보면 가능하지만 50mm 조립 예약 영역과 키캡이 겹칩니다. 실물 홀더·PCB·커넥터의 높이별 외곽 검증 전에는 채택하지 않습니다.</p></details><div class="panel section">'''+section+'''</div></main></body></html>'''
rear_panel='<details open><summary>장착 공간 대안: 15u / 중앙 3열 x 4행 / 62키</summary>'+rearsvg+'<p>상단 중앙 Vol- / Mute / Vol+를 제거하고 뒤쪽 19.05mm를 장착 구조 검토에 사용합니다. 볼은 지름 34mm, 중심 (0, 0)을 유지합니다. 보라색 영역은 가공 구멍이 아니며 보강판 재료를 남긴 지지 구조도 검토합니다. 강성과 수직 공간은 미검증입니다.</p><p><a href="rear-clearance.svg">SVG</a> · <a href="rear-clearance.png">PNG</a> · <a href="rear-clearance-print.pdf">62키 1:1 A3 PDF</a></p></details>'
body=body.replace('<details><summary>대안:',rear_panel+'<details><summary>대안:',1)
(P/'layout.html').write_text(body)
for source, svg_name, html_name in [(svg,'print-1to1.svg','print.html'),(rearsvg,'rear-clearance-print.svg','rear-clearance-print.html')]:
    printsvg=source.replace('height="190mm" viewBox="-165 -88 330 190"','height="200mm" viewBox="-165 -88 330 200"').replace('</g></svg>',dim(-50,50,108,'100 mm calibration — print at 100%')+'</g></svg>')
    ET.fromstring(printsvg);(P/svg_name).write_text(printsvg+'\n')
    (P/html_name).write_text('<!doctype html><html><head><meta charset="utf-8"><title>1:1 validation sheet</title><style>@page{size:A3 landscape;margin:10mm}body{margin:0;font:10pt sans-serif}svg{width:330mm;height:200mm;display:block}</style></head><body><p>Actual size / 100%. Verify the 100mm line with a ruler. Preliminary layout only; not fabrication CAD.</p>'+printsvg+'</body></html>')
print(json.dumps(report,indent=2))
