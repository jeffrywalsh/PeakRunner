"""Original enclosed circulation primitives: floor, two walls, ceiling.

Only the two end portals are intentionally open. The same geometry supplies
rendering and collision, so a stairwell cannot silently lack its visible roof.
"""
def stairwell(mesh, x, width, z0, z1, y0, y1, clearance=6):
    if width <= 0 or clearance < 3 or z0 == z1:
        raise ValueError('invalid stairwell dimensions')
    mesh.ramp(x,width,z0,z1,y0,y1)
    # Mesh.ramp's underside is 0.6 m below its top face.
    mesh.ramp(x,width,z0,z1,y0+clearance+.6,y1+clearance+.6,'panel')
    for side in [-1,1]:
        xx=x+side*width/2
        mesh.quad((xx,y0,z0),(xx,y1,z1),
                  (xx,y1+clearance,z1),(xx,y0+clearance,z0),'concrete')
