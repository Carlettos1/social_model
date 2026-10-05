#![feature(f16)]

use csta::{MonteCarlo, Randomizable, Vec2f64, csta_derive::Randomizable};
use rand::Rng;
use serde_derive::{Deserialize, Serialize};

#[derive(Debug)]
pub struct Config {
    #[allow(dead_code)]
    version: usize,
    pub influece_zones_n: usize,
    pub n_people: usize,
    pub n_steps: usize,
    pub n_burn: usize,
    pub runs_per_thread_n: usize,
    pub n_print: usize,
    pub n_log: usize,
    pub preparation_n: usize,
    pub n_before_flip: usize,
    pub n_flip_accomodation: usize,
    pub hysteresis_constant_1: f64,
    pub flipped_multiplier: f64,
    pub sin_k: f64,
    pub hysteresis_constant_4: f64,
    pub hysteresis_constant_5: f64,

    /// Cantidad de opiniones
    pub q: usize,
    /// j del ising
    pub j: f64,
    /// Consistencia
    pub c: f64,
    /// Consistencia con el IZ
    pub g: f64,
    pub l: f64,
    pub r: f64,

    pub ps: f64,
    pub delta1: f64,
    pub delta2: f64,

    pub e_total: f64,
    pub e_prep: f64,
    pub e_flip: f64,

    ///nvt data
    pub beta1: f64,
    pub beta2: f64,
}

impl Default for Config {
    fn default() -> Self {
        let l = 1.0;
        let r = 0.12 * l;
        let people_n = 200;
        let n_steps = 20_000_000;

        Config {
            version: 3,
            influece_zones_n: 1,
            n_people: people_n,
            n_steps,
            n_burn: n_steps, // 1_000_000
            runs_per_thread_n: 1,
            n_print: 1000,
            n_log: 1000,
            preparation_n: 1_000_000,
            n_before_flip: 0 * n_steps / 100,
            n_flip_accomodation: 100 * n_steps / 100,
            q: 2,
            // j: 1.5,
            j: 0.3,
            c: 1.0,
            g: 6.0,
            l,
            r,
            ps: 0.5,
            delta1: r,
            delta2: 0.1 * r,
            e_total: -100.0,
            e_prep: -((people_n * 1) as f64),
            e_flip: 10000.0,

            beta1: 2.0,
            beta2: 2.0,

            hysteresis_constant_1: 2.0,
            flipped_multiplier: 1.0,
            sin_k: 1_000_000.0,
            hysteresis_constant_4: 0.0,
            hysteresis_constant_5: 0.0,
        }
    }
}

#[derive(Debug, Default, Serialize, Deserialize, Randomizable)]
pub struct Person {
    /// b
    #[csta(default = 0)]
    pub io: usize,
    /// s
    #[csta(default = 0)]
    pub po: usize,
    pub pos: Vec2f64,
}

impl Person {
    pub fn new(io: usize, po: usize, pos: Vec2f64) -> Self {
        Self { io, po, pos }
    }

    pub fn change_opinion(&mut self, config: &Config) {
        self.po += 1;
        self.po %= config.q;
    }

    fn set_opinion(&mut self, opinion: usize) {
        self.io = opinion;
        self.po = opinion;
    }

    fn distance(&self, other: &Self) -> f64 {
        (self.pos - other.pos).len()
    }

    /// Serializa la persona a binario usando 32 bits en vez de 256
    /// primer f16: signo -> io, el módulo es x
    /// segundo f16: signo -> po, el módulo es y
    pub fn serialize_to_bin(&self) -> impl Iterator<Item = u8> {
        let x = self.pos.x() as f16;
        let y = self.pos.y() as f16;
        let x = (x.to_bits() & 0x7FFF) | ((self.io as u16) << 15);
        let y = (y.to_bits() & 0x7FFF) | ((self.po as u16) << 15);
        let x_bytes = x.to_le_bytes();
        let y_bytes = y.to_le_bytes();
        vec![x_bytes, y_bytes].into_iter().flatten()
    }
}

#[derive(Debug, Default, Serialize, Deserialize, Randomizable)]
pub enum Shape {
    Square {
        bottom_left: Vec2f64,
        top_right: Vec2f64,
    },
    Circle {
        center: Vec2f64,
        r: f64,
    },
    #[default]
    None,
}

impl Shape {
    pub fn circle(center: Vec2f64, r: f64) -> Self {
        Self::Circle { center, r }
    }

    pub fn square(bottom_left: Vec2f64, top_right: Vec2f64) -> Self {
        Self::Square {
            bottom_left,
            top_right,
        }
    }

    pub fn inside(&self, point: &Vec2f64) -> bool {
        match self {
            Self::Circle { center, r } => (point - center).len() < *r,
            Self::Square {
                bottom_left,
                top_right,
            } => {
                point.0 > bottom_left.0
                    && point.0 < top_right.0
                    && point.1 > bottom_left.1
                    && point.1 < top_right.1
            }
            Self::None => false,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Randomizable)]
pub struct InfluenceZone {
    pub shape: Shape,
    #[csta(default)]
    pub opinion: usize,
    pub strength: f64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct System {
    pub ppl: Vec<Person>,
    pub izones: Vec<InfluenceZone>,
}

impl InfluenceZone {
    pub fn squared(opinion: usize, strength: f64, x0: f64, y0: f64, x1: f64, y1: f64) -> Self {
        Self {
            shape: Shape::Square {
                bottom_left: (x0, y0).into(),
                top_right: (x1, y1).into(),
            },
            opinion,
            strength,
        }
    }

    pub fn inside(&self, point: &Vec2f64) -> bool {
        self.shape.inside(point)
    }

    pub fn change_opinion(&mut self, config: &Config) {
        self.opinion += 1;
        self.opinion %= config.q;
    }

    /// Serializa la persona a binario usando 80 bits en vez de 416
    /// primeros cuatro f16: bottom_left y top_right
    /// quinto f16: el signo es la opinion, y el módulo es la fuerza
    pub fn serialize_to_bin(&self) -> impl Iterator<Item = u8> {
        match self.shape {
            Shape::Square {
                bottom_left,
                top_right,
            } => {
                let blx = bottom_left.x() as f16;
                let bly = bottom_left.y() as f16;
                let trx = top_right.x() as f16;
                let try_ = top_right.y() as f16;
                let strength = self.strength as f16;
                let strength = (strength.to_bits() & 0x7FFF) | ((self.opinion as u16) << 15);
                return vec![
                    blx.to_le_bytes().to_vec(),
                    bly.to_le_bytes().to_vec(),
                    trx.to_le_bytes().to_vec(),
                    try_.to_le_bytes().to_vec(),
                    strength.to_le_bytes().to_vec(),
                ]
                .into_iter()
                .flatten();
            }
            _ => unimplemented!("ONLY SQUARED SERIALIZATION IMPLEMENTED"),
        }
    }
}

impl Randomizable for System {
    type Conf = Config;
    fn sample<R: rand::Rng + ?Sized>(_: &mut R, config: &Self::Conf) -> Self {
        let mut ppl: Vec<Person> = MonteCarlo::default().take(config.n_people).collect();
        for i in 0..ppl.len() {
            ppl[i].set_opinion(i % config.q);
        }
        Self {
            ppl,
            izones: MonteCarlo::default()
                .take(config.influece_zones_n)
                .collect(),
        }
    }
}

impl System {
    pub fn temperature(&self, config: &Config) -> f64 {
        (config.e_total - self.energy(&config)) / (config.n_people - 1) as f64
    }

    pub fn energy(&self, config: &Config) -> f64 {
        let mut energy = 0.0;
        for i in 0..config.n_people {
            let iperson = self.get(i);
            for j in 0..config.n_people {
                let jperson = self.get(j);
                if j == i {
                    continue;
                }
                if iperson.distance(jperson) > config.r {
                    continue;
                }
                if iperson.po == jperson.po {
                    energy -= config.j * 0.5;
                } else {
                    energy += config.j * 0.5;
                }
            }
            if iperson.po == iperson.io {
                energy -= config.c;
            } else {
                energy += config.c;
            }
        }

        for zone in self.izones.iter() {
            for person in self.ppl.iter() {
                if zone.inside(&person.pos) {
                    if zone.opinion == person.po {
                        energy -= config.g * zone.strength;
                    } else {
                        energy += config.g * zone.strength;
                    }
                }
            }
        }
        energy
    }

    pub fn step<R: Rng>(
        &mut self,
        rng: &mut MonteCarlo<f64, R>,
        index: usize,
        delta: f64,
        config: &Config,
    ) -> (f64, f64, usize) {
        let person = self.get_mut(index);
        let Person { po, pos, .. } = *person;
        let x = pos.x();
        let y = pos.y();

        if rng.next().unwrap() < config.ps {
            person.change_opinion(&config);
        } else {
            loop {
                let dx = (2.0 * rng.next().unwrap() - 1.0) * delta;
                if 0.0 <= person.pos.x() + dx && person.pos.y() + dx <= config.l {
                    person.pos.0 += dx;
                    break;
                }
            }
            loop {
                let dy = (2.0 * rng.next().unwrap() - 1.0) * delta;
                if 0.0 <= person.pos.y() + dy && person.pos.y() + dy <= config.l {
                    person.pos.1 += dy;
                    break;
                }
            }
        }
        (x, y, po)
    }

    /// NVT (canonical) with
    /// N: config.n_ppl
    /// V: LxL
    /// beta: config.beta
    pub fn try_prepare_system(&mut self, config: &Config) -> bool {
        assert_eq!(self.izones.len(), config.influece_zones_n);
        assert_eq!(self.ppl.len(), config.n_people);

        let mut rng = MonteCarlo::default().into_iter();
        let mut energy0 = self.energy(&config);

        for _ in 0..config.preparation_n {
            let k: f64 = rng.next().unwrap();
            let k = (k * config.n_people as f64).floor() as usize;
            let (x, y, s) = self.step(&mut rng, k, config.delta1, &config);

            let energy = self.energy(&config);
            if energy < energy0 || rng.next().unwrap().ln() < -config.beta1 * (energy - energy0) {
                energy0 = energy;
            } else {
                self.get_mut(k).pos.0 = x;
                self.get_mut(k).pos.1 = y;
                self.get_mut(k).po = s;
            }
            if energy0 <= config.e_prep {
                return true;
            }
        }

        false
    }

    pub fn get(&self, idx: usize) -> &Person {
        &self.ppl[idx]
    }

    pub fn get_mut(&mut self, idx: usize) -> &mut Person {
        &mut self.ppl[idx]
    }

    /// Serializa a binario usando un algoritmo distinto
    /// El formato es:
    /// 8 bits: len del vector
    /// [32 bits]: personas
    /// 8 bits: len del vector
    /// [80 bits]: iz (cuadradas)
    pub fn serialize_to_bin(&self) -> Vec<u8> {
        let len_ppl = self.ppl.len() as u8;
        let personas = self.ppl.iter().map(Person::serialize_to_bin).flatten();
        let len_iz = self.izones.len() as u8;
        let izones = self
            .izones
            .iter()
            .map(InfluenceZone::serialize_to_bin)
            .flatten();
        [len_ppl]
            .into_iter()
            .chain(personas)
            .chain([len_iz].into_iter())
            .chain(izones)
            .collect()
    }
}

pub fn probability_log(energy: f64, etot: f64, n_people: usize) -> f64 {
    (n_people - 1) as f64 * (etot - energy).ln()
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Ready {
    Yes,
    No,
}

pub fn full_config(opinion: usize) -> InfluenceZone {
    InfluenceZone::squared(opinion, 1.0, 0.0, 0.0, 1.0, 1.0)
}

pub fn half_config() -> [InfluenceZone; 2] {
    [
        InfluenceZone::squared(0, 1.0, 0.0, 0.0, 1.0 / 2.0, 1.0),
        InfluenceZone::squared(1, 1.0, 1.0 / 2.0, 0.0, 1.0, 1.0),
    ]
}
pub fn chess_config() -> [InfluenceZone; 4] {
    [
        InfluenceZone::squared(0, 1.0, 0.0, 0.0, 1.0 / 2.0, 1.0 / 2.0),
        InfluenceZone::squared(1, 1.0, 1.0 / 2.0, 0.0, 1.0, 1.0 / 2.0),
        InfluenceZone::squared(1, 1.0, 0.0, 1.0 / 2.0, 1.0 / 2.0, 1.0),
        InfluenceZone::squared(0, 1.0, 1.0 / 2.0, 1.0 / 2.0, 1.0, 1.0),
    ]
}
pub fn third_config() -> [InfluenceZone; 2] {
    [
        InfluenceZone::squared(0, 1.0, 0.0, 0.0, 1.0 / 3.0, 1.0),
        InfluenceZone::squared(1, 1.0, 2.0 * 1.0 / 3.0, 0.0, 1.0, 1.0),
    ]
}
pub fn super_chess_config() -> [InfluenceZone; 16] {
    [
        InfluenceZone::squared(0, 1.0, 0.0, 0.0, 1.0 / 4.0, 1.0 / 4.0),
        InfluenceZone::squared(1, 1.0, 1.0 / 4.0, 0.0, 1.0 / 2.0, 1.0 / 4.0),
        InfluenceZone::squared(0, 1.0, 1.0 / 2.0, 0.0, 3.0 * 1.0 / 4.0, 1.0 / 4.0),
        InfluenceZone::squared(1, 1.0, 3.0 * 1.0 / 4.0, 0.0, 1.0, 1.0 / 4.0),
        InfluenceZone::squared(1, 1.0, 0.0, 1.0 / 4.0, 1.0 / 4.0, 1.0 / 2.0),
        InfluenceZone::squared(0, 1.0, 1.0 / 4.0, 1.0 / 4.0, 1.0 / 2.0, 1.0 / 2.0),
        InfluenceZone::squared(1, 1.0, 1.0 / 2.0, 1.0 / 4.0, 3.0 * 1.0 / 4.0, 1.0 / 2.0),
        InfluenceZone::squared(0, 1.0, 3.0 * 1.0 / 4.0, 1.0 / 4.0, 1.0, 1.0 / 2.0),
        InfluenceZone::squared(0, 1.0, 0.0, 1.0 / 2.0, 1.0 / 4.0, 3.0 * 1.0 / 4.0),
        InfluenceZone::squared(1, 1.0, 1.0 / 4.0, 1.0 / 2.0, 1.0 / 2.0, 3.0 * 1.0 / 4.0),
        InfluenceZone::squared(
            0,
            1.0,
            1.0 / 2.0,
            1.0 / 2.0,
            3.0 * 1.0 / 4.0,
            3.0 * 1.0 / 4.0,
        ),
        InfluenceZone::squared(1, 1.0, 3.0 * 1.0 / 4.0, 1.0 / 2.0, 1.0, 3.0 * 1.0 / 4.0),
        InfluenceZone::squared(1, 1.0, 0.0, 3.0 * 1.0 / 2.0, 1.0 / 4.0, 1.0),
        InfluenceZone::squared(0, 1.0, 1.0 / 4.0, 3.0 * 1.0 / 2.0, 1.0 / 2.0, 1.0),
        InfluenceZone::squared(1, 1.0, 1.0 / 2.0, 3.0 * 1.0 / 2.0, 3.0 * 1.0 / 4.0, 1.0),
        InfluenceZone::squared(0, 1.0, 3.0 * 1.0 / 4.0, 3.0 * 1.0 / 2.0, 1.0, 1.0),
    ]
}
