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

/// Detection and firing numbers for anything that acquires targets. Fixed map
/// turrets and future player-placed turrets look up the same table.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TurretProfile {
    /// Detection range without a powered same-team sensor.
    pub range: f32,
    /// Detection range while a same-team sensor is powered.
    pub sensed_range: f32,
    pub speed: f32,
    pub life: f32,
    pub cooldown: f32,
    /// Aim at the constant-velocity intercept point instead of the target.
    pub leads: bool,
    pub fires: bool,
    /// Projectile kind stored in `Disc::kind`.
    pub projectile: u8,
}

pub const SENSOR_PROFILE: TurretProfile = TurretProfile {
    range:260., sensed_range:260., speed:0., life:0., cooldown:0., leads:false, fires:false, projectile:0};
pub const BULLET_TURRET_PROFILE: TurretProfile = TurretProfile {
    range:80., sensed_range:150., speed:crate::sim::BOLT_SPEED, life:1., cooldown:0.22, leads:false, fires:true, projectile:1};
pub const PLASMA_TURRET_PROFILE: TurretProfile = TurretProfile {
    range:80., sensed_range:150., speed:80., life:3., cooldown:1.2, leads:true, fires:true, projectile:3};

pub fn profile(kind:Kind, weapon:TurretWeapon)->Option<TurretProfile> {
    match (kind,weapon) {
        (Kind::Sensor,_)=>Some(SENSOR_PROFILE),
        (Kind::Turret,TurretWeapon::Bullet)=>Some(BULLET_TURRET_PROFILE),
        (Kind::Turret,TurretWeapon::Plasma)=>Some(PLASMA_TURRET_PROFILE),
        _=>None,
    }
}

/// Hull and generator-powered shield numbers per kind. Shields only exist on
/// sensors and fixed turrets: generators are the thing attackers go for, and
/// stations stay as they were. A shield soaks damage before the hull, refills
/// after `shield_regen_delay` seconds without damage while its circuit is
/// powered, and drops to zero the moment the circuit loses power.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Durability {
    pub hull: f32,
    pub shield_max: f32,
    pub shield_regen_delay: f32,
    /// Shield points per second once regeneration starts.
    pub shield_regen_rate: f32,
    /// Fraction of bullet (chaingun and bullet-turret) damage a shield takes;
    /// shields shrug off small arms, explosives hit them in full.
    pub shield_bullet_factor: f32,
}

pub fn durability(kind:Kind)->Durability {
    let unshielded=|hull|Durability {hull,shield_max:0.,shield_regen_delay:0.,shield_regen_rate:0.,shield_bullet_factor:1.};
    match kind {
        Kind::Turret=>Durability {hull:250.,shield_max:450.,shield_regen_delay:5.,shield_regen_rate:60.,shield_bullet_factor:0.5},
        Kind::Sensor=>Durability {hull:150.,shield_max:300.,shield_regen_delay:5.,shield_regen_rate:45.,shield_bullet_factor:0.5},
        Kind::Generator=>unshielded(500.),
        Kind::Inventory|Kind::Repair=>unshielded(300.),
    }
}

/// Barrel clearance beyond the turret's body radius; LOS and shots start here.
pub const MUZZLE_GAP: f32 = 0.6;
/// Targets are sighted at the chest, not the feet.
pub const CHEST_HEIGHT: f32 = 0.8;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Candidate { pub index: usize, pub team: u8, pub pos: Vec3, pub vel: Vec3 }

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Acquired {
    pub index: usize,
    /// Where the turret points (the intercept point for leading weapons).
    pub aim_point: Vec3,
    pub aim: Vec3,
    pub muzzle: Vec3,
}

/// The one targeting rule: the nearest living enemy within range whose chest
/// is visible from the barrel start. `clear(from,to)` is the caller's line of
/// sight query, so callers can add blockers such as placed objects. Before
/// firing, callers must also check `clear(acquired.muzzle, acquired.aim_point)`.
pub fn acquire_target(origin:Vec3, radius:f32, team:u8, profile:&TurretProfile, sensed:bool,
    candidates:impl IntoIterator<Item=Candidate>, mut clear:impl FnMut(Vec3,Vec3)->bool)->Option<Acquired> {
    let range=if sensed {profile.sensed_range} else {profile.range};
    let (_,index,target,velocity)=candidates.into_iter().filter(|c|c.team!=team)
        .filter_map(|c| {
            let target=c.pos+Vec3::Y*CHEST_HEIGHT;let delta=target-origin;
            if delta.length()>range {return None;}
            let start=origin+delta.normalize_or_zero()*(radius+MUZZLE_GAP);
            clear(start,target).then_some((delta.length(),c.index,target,c.vel))
        }).min_by(|a,b|a.0.total_cmp(&b.0))?;
    let aim_point=if profile.leads {
        intercept_time(target-origin,velocity,profile.speed,radius+MUZZLE_GAP,profile.life)
            .map_or(target,|t|target+velocity*t)
    } else {target};
    let aim=(aim_point-origin).normalize_or_zero();
    Some(Acquired {index,aim_point,aim,muzzle:origin+aim*(radius+MUZZLE_GAP)})
}

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
    pub fn max_health(&self)->f32 {durability(self.kind).hull}
    pub fn max_shield(&self)->f32 {durability(self.kind).shield_max}
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct State {
    pub health: f32,
    pub powered: bool,
    pub cooldown: f32,
    pub aim: Vec3,
    pub contacts: u8,
    pub shield: f32,
    /// Seconds since this object last took damage. Server-side regen timer;
    /// not sent to clients, which never simulate equipment.
    #[serde(skip, default)]
    pub since_hit: f32,
}

impl State {
    pub fn new(d:&Definition)->Self {
        State {health:d.max_health(),powered:true,cooldown:0.,aim:Vec3::NEG_Z,contacts:0,shield:d.max_shield(),since_hit:f32::INFINITY}
    }
    /// Damage soaks into the shield first, the overflow reaches the hull.
    pub fn damage(&mut self, d:&Definition, amount:f32, bullet:bool) {
        if !amount.is_finite() || amount<=0. || self.health<=0. {return;}
        self.since_hit=0.;
        let factor=if bullet {durability(d.kind).shield_bullet_factor} else {1.};
        let mut rest=amount;
        if self.shield>0. && factor>0. {
            let absorbed=(rest*factor).min(self.shield);
            self.shield-=absorbed;rest-=absorbed/factor;
        }
        self.health=(self.health-rest).max(0.);
        if self.health<=0. {self.shield=0.;}
    }
    /// Advance shield regeneration. Call after `power` for the tick.
    pub fn regen(&mut self, d:&Definition, dt:f32) {
        let spec=durability(d.kind);
        self.since_hit+=dt;
        if !self.powered || self.health<=0. {self.shield=0.;return;}
        if self.since_hit>=spec.shield_regen_delay {
            self.shield=(self.shield+spec.shield_regen_rate*dt).min(spec.shield_max);
        }
    }
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
        if d.circuit!="always-on" && !defs.iter().any(|g|g.kind==Kind::Generator && g.team==d.team && g.circuit==d.circuit) {
            return Err(format!("Equipment {} has no same-team generator",d.id));
        }
    }
    Ok(())
}

pub fn definitions(map:MapId)->&'static [Definition] {
    crate::map_pack::on(map).map_or(&[],|p|p.manifest.entities.as_slice())
}
pub fn fresh(map:MapId)->Vec<State> {
    definitions(map).iter().map(State::new).collect()
}
pub fn power(defs:&[Definition],states:&mut [State]) {
    let live: Vec<_>=defs.iter().zip(states.iter()).filter(|(d,s)|d.kind==Kind::Generator && s.health>0.)
        .map(|(d,_)|(d.team,d.circuit.as_str())).collect();
    for (d,s) in defs.iter().zip(states) {
        s.powered=s.health>0. && (d.circuit=="always-on" || live.contains(&(d.team,d.circuit.as_str())));
        // Shields are projected by the generator: no power, no shield.
        if !s.powered {s.shield=0.;}
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
    fn enemy(index:usize,pos:Vec3)->Candidate {Candidate {index,team:1,pos,vel:Vec3::ZERO}}
    fn open(_:Vec3,_:Vec3)->bool {true}

    #[test]
    fn profile_table_keeps_the_established_numbers() {
        assert_eq!(profile(Kind::Sensor,TurretWeapon::Bullet).map(|p|(p.range,p.sensed_range,p.fires)),Some((260.,260.,false)));
        let bullet=profile(Kind::Turret,TurretWeapon::Bullet).unwrap();
        assert_eq!((bullet.range,bullet.sensed_range,bullet.speed,bullet.life,bullet.cooldown,bullet.leads,bullet.projectile),
            (80.,150.,crate::sim::BOLT_SPEED,1.,0.22,false,1));
        let plasma=profile(Kind::Turret,TurretWeapon::Plasma).unwrap();
        assert_eq!((plasma.range,plasma.sensed_range,plasma.speed,plasma.life,plasma.cooldown,plasma.leads,plasma.projectile),
            (80.,150.,80.,3.,1.2,true,3));
        for kind in [Kind::Inventory,Kind::Generator,Kind::Repair] {assert!(profile(kind,TurretWeapon::Bullet).is_none());}
    }

    #[test]
    fn acquire_target_respects_range_sensors_and_teams() {
        let p=BULLET_TURRET_PROFILE;
        let far=enemy(0,Vec3::new(100.,-0.8,0.));
        assert!(acquire_target(Vec3::ZERO,1.,0,&p,false,[far],open).is_none(),"100 m is outside unsensed range");
        assert_eq!(acquire_target(Vec3::ZERO,1.,0,&p,true,[far],open).map(|a|a.index),Some(0),"a powered sensor extends range");
        let friend=Candidate {index:1,team:0,pos:Vec3::new(5.,-0.8,0.),vel:Vec3::ZERO};
        assert!(acquire_target(Vec3::ZERO,1.,0,&p,false,[friend],open).is_none(),"teammates are never targets");
    }

    #[test]
    fn acquire_target_picks_the_nearest_visible_enemy() {
        let p=BULLET_TURRET_PROFILE;
        let near=enemy(0,Vec3::new(10.,-0.8,0.));let farther=enemy(1,Vec3::new(30.,-0.8,0.));
        assert_eq!(acquire_target(Vec3::ZERO,1.,0,&p,false,[farther,near],open).map(|a|a.index),Some(0));
        let blocked_near=|_:Vec3,to:Vec3|to.x>20.;
        assert_eq!(acquire_target(Vec3::ZERO,1.,0,&p,false,[farther,near],blocked_near).map(|a|a.index),Some(1),
            "a blocked nearer enemy is skipped");
        assert!(acquire_target(Vec3::ZERO,1.,0,&p,false,[near],|_,_|false).is_none());
    }

    #[test]
    fn acquire_target_sights_from_the_barrel_to_the_chest() {
        let mut seen=Vec::new();
        let hit=acquire_target(Vec3::ZERO,1.5,0,&BULLET_TURRET_PROFILE,false,[enemy(0,Vec3::new(20.,-0.8,0.))],
            |a,b|{seen.push((a,b));true}).unwrap();
        assert_eq!(seen,vec![(Vec3::new(1.5+MUZZLE_GAP,0.,0.),Vec3::new(20.,0.,0.))]);
        assert_eq!((hit.aim_point,hit.aim,hit.muzzle),(Vec3::new(20.,0.,0.),Vec3::X,Vec3::new(2.1,0.,0.)));
    }

    #[test]
    fn leading_profiles_aim_at_the_intercept_point() {
        let mover=Candidate {index:0,team:1,pos:Vec3::new(40.,-0.8,0.),vel:Vec3::new(0.,0.,20.)};
        let plain=acquire_target(Vec3::ZERO,1.,0,&BULLET_TURRET_PROFILE,false,[mover],open).unwrap();
        let led=acquire_target(Vec3::ZERO,1.,0,&PLASMA_TURRET_PROFILE,false,[mover],open).unwrap();
        assert_eq!(plain.aim_point,Vec3::new(40.,0.,0.));
        let t=intercept_time(Vec3::new(40.,0.,0.),mover.vel,80.,1.+MUZZLE_GAP,3.).unwrap();
        assert_eq!(led.aim_point,Vec3::new(40.,0.,0.)+mover.vel*t);
        assert!(led.aim.z>0.,"plasma leads a target moving across its line");
    }

    #[test]
    fn always_on_circuits_need_no_fabricated_generator() {
        let d=Definition{id:"station".into(),kind:Kind::Inventory,position:[0.;3],team:0,
            circuit:"always-on".into(),radius:1.5,weapon:TurretWeapon::Bullet};
        assert!(validate(&[d.clone()]).is_ok());
        let mut states=vec![State{health:100.,powered:false,cooldown:0.,aim:Vec3::ZERO,contacts:0,shield:0.,since_hit:0.}];
        power(&[d.clone()],&mut states);assert!(states[0].powered);
        states[0].health=0.;power(&[d.clone()],&mut states);assert!(!states[0].powered);
        let mut invalid=d;invalid.circuit="missing-generator".into();assert!(validate(&[invalid]).is_err());
    }
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
        let mut states:Vec<_>=defs.iter().map(|d|State::new(d)).collect();
        states[0].health=0.;power(&defs,&mut states);
        assert!(!states[0].powered && !states[1].powered && states[2].powered);
        states[0].health=1.;power(&defs,&mut states);assert!(states[1].powered);
    }
    fn def(id:&str,kind:Kind,circuit:&str)->Definition {
        Definition {id:id.into(),kind,team:0,circuit:circuit.into(),position:[0.;3],radius:2.,weapon:TurretWeapon::Bullet}
    }
    #[test]
    fn only_sensors_and_turrets_are_shielded() {
        assert!(durability(Kind::Turret).shield_max>0. && durability(Kind::Sensor).shield_max>0.);
        for kind in [Kind::Generator,Kind::Inventory,Kind::Repair] {assert_eq!(durability(kind).shield_max,0.);}
        assert_eq!(def("t",Kind::Turret,"a").max_health(),250.,"hull numbers are unchanged");
        assert_eq!(def("g",Kind::Generator,"a").max_health(),500.);
        assert_eq!(def("s",Kind::Sensor,"a").max_health(),150.);
    }
    #[test]
    fn shield_absorbs_before_hull_and_bullets_are_halved() {
        let d=def("t",Kind::Turret,"a");let mut s=State::new(&d);
        s.damage(&d,100.,false);assert_eq!((s.shield,s.health),(350.,250.));
        s.damage(&d,8.,true);assert_eq!((s.shield,s.health),(346.,250.),"shields take half of bullet damage");
        s.damage(&d,400.,false);assert_eq!((s.shield,s.health),(0.,196.),"overflow reaches the hull");
        s.shield=2.;s.damage(&d,8.,true);assert_eq!((s.shield,s.health),(0.,192.),"bullet overflow is counted at full value");
        s.damage(&d,1000.,false);assert_eq!((s.shield,s.health),(0.,0.));
        let hp=s.health;s.damage(&d,5.,false);assert_eq!(s.health,hp,"destroyed objects take no more damage");
    }
    #[test]
    fn shields_regen_only_after_the_delay_while_powered() {
        let d=def("t",Kind::Turret,"a");let spec=durability(d.kind);let mut s=State::new(&d);
        s.damage(&d,300.,false);let hit=s.shield;
        for _ in 0..((spec.shield_regen_delay-0.5)*10.) as usize {s.regen(&d,0.1);}
        assert_eq!(s.shield,hit,"no regen during the delay");
        for _ in 0..200 {s.regen(&d,0.1);}
        assert_eq!(s.shield,spec.shield_max,"regenerates to full while powered");
        s.damage(&d,300.,false);s.powered=false;
        for _ in 0..200 {s.regen(&d,0.1);}
        assert_eq!(s.shield,0.,"an unpowered object has no shield and cannot regen");
    }
    #[test]
    fn generator_loss_drops_shields_and_leaves_the_hull_exposed() {
        let defs=vec![def("g",Kind::Generator,"a"),def("t",Kind::Turret,"a"),def("s",Kind::Sensor,"a")];
        let mut states:Vec<_>=defs.iter().map(State::new).collect();
        states[0].damage(&defs[0],1000.,false);assert_eq!(states[0].health,0.);
        power(&defs,&mut states);
        assert!(!states[1].powered && !states[2].powered);
        assert_eq!((states[1].shield,states[2].shield),(0.,0.));
        states[1].damage(&defs[1],100.,false);assert_eq!(states[1].health,150.,"damage goes straight to the hull");
        // Repairing the generator brings power back; the shield regrows after the delay.
        states[0].health=1.;power(&defs,&mut states);
        for _ in 0..200 {for (d,s) in defs.iter().zip(&mut states) {s.regen(d,0.1);}}
        assert!(states[1].powered && states[1].shield==durability(Kind::Turret).shield_max);
    }
    #[test]
    fn always_on_equipment_keeps_a_regenerating_shield() {
        let d=def("t",Kind::Turret,"always-on");assert!(validate(&[d.clone()]).is_ok());
        let mut states=vec![State::new(&d)];
        states[0].damage(&d,300.,false);power(std::slice::from_ref(&d),&mut states);
        for _ in 0..200 {states[0].regen(&d,0.1);}
        assert_eq!(states[0].shield,durability(Kind::Turret).shield_max);
    }
    #[test]
    fn rejects_missing_generator_and_duplicate_ids() {
        let d=Definition {id:"station".into(),kind:Kind::Inventory,team:0,circuit:"a".into(),position:[0.;3],radius:2.,weapon:TurretWeapon::Bullet};
        assert!(validate(&[d.clone()]).is_err());
        let mut g=d.clone();g.kind=Kind::Generator;
        assert!(validate(&[g,d]).is_err());
    }
}
