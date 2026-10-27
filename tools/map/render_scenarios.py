#!/usr/bin/env python3
"""Render the scenario selector art (apple/Resources/scenarios/*.png).

Usage: render_scenarios.py <dir> where <dir> holds adm1.json, adm0.json, cities.json and lakes.json
exported from Natural Earth 10m with mapshaper (see tools/map/build_map.sh). Run it in the output
directory; it writes one 1200x640 PNG per scenario.
"""
import json, math, sys
from PIL import Image, ImageDraw, ImageFilter, ImageFont
W,H=1200,640; SS=2
adm1=json.load(open(sys.argv[1]+'/adm1.json'))['features']
adm0=json.load(open(sys.argv[1]+'/adm0.json'))['features']
cities=json.load(open(sys.argv[1]+'/cities.json'))['features']
lakes=json.load(open(sys.argv[1]+'/lakes.json'))['features']
font=ImageFont.truetype('/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf',16*SS)
def rings(geom):
    if not geom: return []
    if geom['type']=='Polygon': return [geom['coordinates'][0]]
    if geom['type']=='MultiPolygon': return [p[0] for p in geom['coordinates']]
    return []
def make(name, lon0, lat0, zoom, pick, tint, city_names, out):
    w,h=W*SS,H*SS
    R=min(w,h)*zoom
    L0,P0=math.radians(lon0),math.radians(lat0)
    def pr(lon,lat):
        l,p=math.radians(lon),math.radians(lat); dl=l-L0
        cosc=math.sin(P0)*math.sin(p)+math.cos(P0)*math.cos(p)*math.cos(dl)
        x=math.cos(p)*math.sin(dl); y=math.cos(P0)*math.sin(p)-math.sin(P0)*math.cos(p)*math.cos(dl)
        return (w/2+R*x, h*0.55-R*y), cosc>0
    img=Image.new('RGB',(w,h))
    d=ImageDraw.Draw(img)
    for yy in range(h):  # sky gradient
        t=yy/h; d.line([(0,yy),(w,yy)],fill=(int(8+10*t),int(16+22*t),int(28+40*t)))
    # globe disc
    cx,cy=w/2,h*0.55
    d.rectangle([0,0,w,h],fill=(14,40,64)) if R>w else d.ellipse([cx-R,cy-R,cx+R,cy+R],fill=(14,40,64))
    # graticule
    for lon in range(-180,180,10):
        pts=[pr(lon,la) for la in range(-80,81,2)]
        seg=[p for p,v in pts if v]
        if len(seg)>1: d.line(seg,fill=(26,58,86),width=SS)
    for lat in range(-80,81,10):
        pts=[pr(lo,lat) for lo in range(-180,181,2)]
        seg=[p for p,v in pts if v]
        if len(seg)>1: d.line(seg,fill=(26,58,86),width=SS)
    def poly(ring):
        pts=[pr(x,y) for x,y in ring]
        if not all(v for _,v in pts): return None
        return [p for p,_ in pts]
    for f in adm0:
        for r in rings(f['geometry']):
            p=poly(r)
            if p and len(p)>2: d.polygon(p,fill=(37,40,43),outline=(70,74,78))
    for f in adm1:
        for r in rings(f['geometry']):
            p=poly(r)
            if p and len(p)>2: d.line(p+[p[0]],fill=(58,62,66),width=SS)
    hl=[f for f in adm1 if pick(f['properties'])]
    glow=Image.new('RGBA',(w,h),(0,0,0,0)); g=ImageDraw.Draw(glow)
    for f in hl:
        for r in rings(f['geometry']):
            p=poly(r)
            if p and len(p)>2: g.polygon(p,fill=tint+(255,))
    blur=glow.filter(ImageFilter.GaussianBlur(26*SS))
    r_,g_,b_,a_=blur.split(); a_=a_.point(lambda v: min(255,v*9//10))
    blur=Image.merge('RGBA',(r_,g_,b_,a_))
    img=Image.alpha_composite(img.convert('RGBA'),blur)
    d=ImageDraw.Draw(img)
    for f in hl:
        for r in rings(f['geometry']):
            p=poly(r)
            if p and len(p)>2:
                d.polygon(p,fill=tuple(int(c*0.62+40*0.38) for c in tint)+(255,),outline=tuple(min(255,c+60) for c in tint)+(255,))
    for f in hl:
        for r in rings(f['geometry']):
            p=poly(r)
            if p and len(p)>2: d.line(p+[p[0]],fill=(232,238,245,220),width=2*SS)
    for f in lakes:
        for r in rings(f['geometry']):
            p=poly(r)
            if p and len(p)>2: d.polygon(p,fill=(22,53,85,255),outline=(48,96,140,255))
    placed=[]
    order={n:i for i,n in enumerate(city_names)}
    for c in sorted([c for c in cities if c['properties']['NAME'] in order], key=lambda c: order[c['properties']['NAME']]):
        nm=c['properties']['NAME']
        lo,la=c['geometry']['coordinates']; (x,y),v=pr(lo,la)
        if not v: continue
        tw=d.textlength(nm,font=font)
        box=(x-6*SS,y-12*SS,x+12*SS+tw,y+10*SS)
        if any(not (box[2]<b[0] or box[0]>b[2] or box[3]<b[1] or box[1]>b[3]) for b in placed): continue
        placed.append(box)
        d.ellipse([x-5*SS,y-5*SS,x+5*SS,y+5*SS],fill=(11,22,34),outline=(232,238,245),width=2*SS)
        for dx in (-2,2):
            for dy in (-2,2): d.text((x+9*SS+dx,y-8*SS+dy),nm,font=font,fill=(11,22,34))
        d.text((x+9*SS,y-8*SS),nm,font=font,fill=(232,238,245))
    # rim and vignette
    d.ellipse([cx-R,cy-R,cx+R,cy+R],outline=(40,90,130),width=3*SS)
    vig=Image.new('L',(w,h),0); vd=ImageDraw.Draw(vig)
    for i in range(40):
        a=int(255*(i/40)**2); vd.rectangle([i*SS*6,i*SS*4,w-i*SS*6,h-i*SS*4],outline=255-a)
    dark=Image.new('RGBA',(w,h),(5,9,14,255))
    img=Image.composite(dark,img,vig.filter(ImageFilter.GaussianBlur(40*SS)).point(lambda v:v*7//10))
    img=img.convert('RGB').resize((W,H),Image.LANCZOS)
    img.save(out,optimize=True)
    print(out)
GL=['IL','IN','KY','MI','NY','OH','PA','WV']
make('great-lakes',-83.0,42.6,3.5,lambda p:p['iso_a2']=='US' and p['postal'] in GL,(226,176,74),['Chicago','Detroit','Cleveland','Pittsburgh','New York','Columbus','Indianapolis','Buffalo','Cincinnati','Louisville'],'great-lakes-2000.png')
make('texas',-96.0,32.0,4.2,lambda p:p['iso_a2']=='US' and p['postal'] in ['TX','OK','LA'],(229,96,94),['Houston','Dallas','San Antonio','Austin','Oklahoma City','New Orleans','Fort Worth'],'texas-1981.png')
make('california',-119.5,37.2,5.2,lambda p:p['iso_a2']=='US' and p['postal'] in ['CA'],(66,165,245),['San Francisco','Los Angeles','San Jose','San Diego','Sacramento'],'silicon-valley-1995.png')
make('germany',10.4,51.0,5.9,lambda p:p['iso_a2']=='DE',(63,191,138),['Berlin','Hamburg','Munich','Cologne','Frankfurt','Düsseldorf','Essen','Stuttgart','Leipzig','Dresden'],'ruhr-rhine-1991.png')
