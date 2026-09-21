"""Original optional docking approach; metres, Y-up, entrance on negative Z."""
def build(mesh, team):
    accent = ('ember', 'glacier')[team]
    # Open-front receiving bay. The rear opens only into the sheltered ramp.
    mesh.box((0,-12.5,-86),(20,1,16),'panel')
    mesh.box((0,-5.5,-86),(20,1,16),'panel')
    for side in (-1,1):
        mesh.box((side*10,-9,-86),(1,6,16),'concrete')
        mesh.box((side*7,-9,-78),(6,6,1),'concrete')
        for z in (-93,-86,-79):
            mesh.box((side*9.4,-9,z),(.5,6,.6),'trim')
            mesh.box((side*9,-8,z),(.12,2,.25),accent,solid=False)
    # 8m wide, 1:2 slope, 6m vertical clearance. No collision wedge beneath it.
    mesh.ramp(0,8,-78,-54,-12,0)
    mesh.ramp(0,8,-78,-54,-5.4,6.6,'panel')
    for x in (-4,4):
        mesh.quad((x,-12,-78),(x,0,-54),(x,6,-54),(x,-6,-78),'concrete')
    mesh.box((0,-.5,-53),(8,1,2),'panel')
    # Recessed approach lights and visible structural support, not a new mechanic.
    for x in (-7,7):
        mesh.box((x,-11.95,-86),(.3,.08,12),accent,solid=False)
        mesh.column((x,-16,-86),1.4,3,'trim',top=2)
        mesh.column((x,-17,-86),.9,1,'glacier',top=1.4,solid=False)
    return (0,-10.8,-90)
