//! Map-authored equipment definitions and authoritative runtime state.
use glam::Vec3;
use serde::{Deserialize, Serialize};
use crate::{sim::{Player, ENERGY_MAX}, terrain::MapId};

/// Constant-velocity interception, including the barrel's forward spawn offset.
/// Plasma flies straight; gravity/steering changes after firing can evade it.
pub fn intercept_time(relative:Vec3, velocity:Vec3, speed:f32, muzzle_offset:f32, lifetime:f32)->Option<f32> {
    if !relative.is_finite() || !velocity.is_finite() || !speed.is_finite()
        || !muzzle_offset.is_finite() || !lifetime.is_finite() || speed<=0. || muzzle_offset<0. || lifetime<=0. {return None;}
    let r=relative.as_dvec3();let v=velocity.as_dvec3();let s=speed as f64;let o=muzzle_offset as f64;
    let a=v.length_squared()-s*s;
    let b=2.*(r.dot(v)-o*s);
    let c=r.length_squared()-o*o;
    let valid=|t:f64|t.is_finite() && t>0. && t<=lifetime as f64;
    if a.abs()<1e-8 {
        let t=-c/b;return valid(t).then_some(t as f32);
    }
    let discriminant=b*b-4.*a*c;
    if discriminant<0. {return None;}
    let root=discriminant.sqrt();
    [(-b-root)/(2.*a),(-b+root)/(2.*a)].into_iter().filter(|t|valid(*t))
        .min_by(f64::total_cmp).map(|t|t as f32)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all="snake_case")]
pub enum Kind { Inventory, Generator, Sensor, Turret, Repair }

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all="snake_case")]
pub enum TurretWeapon { #[default] Bullet, Plasma }

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Definition {
    pub id: String,
    pub kind: Kind,
    pub position: [f32;3],
    pub team: u8,
    pub circuit: String,
    pub radius: f32,
    #[serde(default)]
    pub weapon: TurretWeapon,
}
impl Definition {
    pub fn pos(&self)->Vec3 {Vec3::from_array(self.position)}
    pub fn max_health(&self)->f32 {match self.kind {Kind::Generator=>500.,Kind::Turret=>250.,Kind::Sensor=>150.,_=>300.}}
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct State {
    pub health: f32,
    pub powered: bool,
    pub cooldown: f32,
    pub aim: Vec3,
    pub contacts: u8,
}

pub fn validate(defs:&[Definition])->Result<(),String> {
    if defs.len()>128 {return Err("Too many equipment objects".into());}
    let mut ids=std::collections::HashSet::new();
    for d in defs {
        if d.id.is_empty() || d.id.len()>64 || !ids.insert(&d.id) || d.team>1
            || d.circuit.is_empty() || d.circuit.len()>64 || !d.radius.is_finite()
            || !(0.5..=8.).contains(&d.radius) || d.position.iter().any(|v|!v.is_finite() || v.abs()>10000.) {
            return Err("Invalid equipment definition".into());
        }
        if !defs.iter().any(|g|g.kind==Kind::Generator && g.team==d.team && g.circuit==d.circuit) {
            return Err(format!("Equipment {} has no same-team generator",d.id));
        }
    }
    Ok(())
}

pub fn definitions(map:MapId)->&'static [Definition] {
    crate::map_pack::on(map).map_or(&[],|p|p.manifest.entities.as_slice())
}
pub fn fresh(map:MapId)->Vec<State> {
    definitions(map).iter().map(|d|State {health:d.max_health(),powered:true,cooldown:0.,aim:Vec3::NEG_Z,contacts:0}).collect()
}
pub fn power(defs:&[Definition],states:&mut [State]) {
    let live: Vec<_>=defs.iter().zip(states.iter()).filter(|(d,s)|d.kind==Kind::Generator && s.health>0.)
        .map(|(d,_)|(d.team,d.circuit.as_str())).collect();
    for (d,s) in defs.iter().zip(states) {
        s.powered=s.health>0. && live.contains(&(d.team,d.circuit.as_str()));
    }
}
pub fn service(d:&Definition,s:&mut State,p:&mut Player,using:bool,dt:f32) {
    if !p.alive || p.team.idx()!=d.team as usize || p.pos.distance(d.pos())>d.radius+1.5 {return;}
    // Repairs require a held server input, proximity and energy. A ruined
    // generator can be brought back online; no automatic hidden respawn.
    if using && s.health<d.max_health() && p.energy>0. {
        let spent=p.energy.min(12.*dt);
        p.energy-=spent;s.health=(s.health+spent*4.).min(d.max_health());
    } else if s.powered && s.health>0. && (d.kind==Kind::Repair || (d.kind==Kind::Inventory && using)) {
        p.health=(p.health+35.*dt).min(100.);
        p.energy=(p.energy+40.*dt).min(ENERGY_MAX);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn plasma_lead_meets_constant_velocity_targets() {
        let r=Vec3::new(90.,5.,0.);
        for v in [Vec3::ZERO,Vec3::new(0.,0.,45.),Vec3::new(-40.,0.,0.),Vec3::new(35.,0.,0.),Vec3::new(0.,30.,10.),Vec3::new(-80.,0.,0.)] {
            let t=intercept_time(r,v,80.,3.,3.).expect("reachable target");
            let target=r+v*t;let aim=target.normalize();
            assert!((aim*(3.+80.*t)-target).length()<0.001,"missed {v:?}");
        }
        assert!(intercept_time(r,Vec3::X*100.,80.,3.,3.).is_none());
        assert!(intercept_time(r,Vec3::ZERO,80.,3.,0.2).is_none());
        assert!(intercept_time(Vec3::splat(f32::NAN),Vec3::ZERO,80.,3.,3.).is_none());
    }
    #[test]
    fn generator_loss_only_disables_its_own_circuit() {
        let make=|id:&str,kind,team|Definition {id:id.into(),kind,team,circuit:format!("base{team}"),position:[0.;3],radius:2.,weapon:TurretWeapon::Bullet};
        let defs=vec![make("a",Kind::Generator,0),make("b",Kind::Inventory,0),make("c",Kind::Generator,1)];
        assert!(validate(&defs).is_ok());
        let mut states:Vec<_>=defs.iter().map(|d|State{health:d.max_health(),powered:true,cooldown:0.,aim:Vec3::ZERO,contacts:0}).collect();
        states[0].health=0.;power(&defs,&mut states);
        assert!(!states[0].powered && !states[1].powered && states[2].powered);
        states[0].health=1.;power(&defs,&mut states);assert!(states[1].powered);
    }
    #[test]
    fn rejects_missing_generator_and_duplicate_ids() {
        let d=Definition {id:"station".into(),kind:Kind::Inventory,team:0,circuit:"a".into(),position:[0.;3],radius:2.,weapon:TurretWeapon::Bullet};
        assert!(validate(&[d.clone()]).is_err());
        let mut g=d.clone();g.kind=Kind::Generator;
        assert!(validate(&[g,d]).is_err());
    }
}
