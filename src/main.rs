use std::f64;

use csta::MonteCarlo;
use file_log::log;
use modelo_social::*;

fn main() {
    // let mut args = env::args();
    // args.next();
    // let next = args.next().unwrap();

    n_sim();
}

fn n_sim() {
    let config = Config::default();
    assert!(config.e_prep < config.e_total);
    log!("config/config", "{config:#?}");
    println!("{config:#?}");
    for _ in 0..config.runs_per_thread_n {
        let mut sys = get_prepared_system(&config);
        nvt_simulation(&mut sys, &config);
    }
}

fn get_prepared_system(config: &Config) -> System {
    for mut sys in MonteCarlo::<System, _>::default() {
        sys.izones = vec![full_config(1)];
        if sys.try_prepare_system(config) {
            return sys;
        }
    }
    unreachable!("MonteCarlo Iterator has ended (not expected)");
}

#[allow(clippy::modulo_one)]
/// NVT (canonical) with
/// N: config.n_ppl
/// V: LxL
/// beta: config.beta
fn nvt_simulation(system: &mut System, config: &Config) {
    let mut rechazados: usize = 0;
    let mut b0_in_0: Vec<u32> = Vec::new();
    let mut b1_in_1: Vec<u32> = Vec::new();
    let mut flipped: bool = false;
    let mut rng = MonteCarlo::<f64, _>::default();
    let mut energy0 = system.energy(config);
    let mut temperatures = Vec::with_capacity(config.n_steps - config.n_burn);

    for n in 1..config.n_steps {
        let k = (rng.next().unwrap() * config.n_people as f64).floor() as usize;
        let (x, y, s) = system.step(&mut rng, k, config.delta2, config);
        let energy = system.energy(config);

        if n > config.n_before_flip && n <= config.n_flip_accomodation + config.n_before_flip {
            let step_after_flip = n - config.n_before_flip;
            let proportion = step_after_flip as f64 / config.n_flip_accomodation as f64;
            assert!(
                proportion >= 0.0 && proportion <= 1.0,
                "Proportion should go from 0.0 to 1.0"
            );

            for zone in &mut system.izones {
                let strength = config.flipped_multiplier
                    * (step_after_flip as f64 / config.sin_k * f64::consts::PI).cos();
                if flipped && strength > 0.0 {
                    zone.change_opinion(config);
                    flipped = false;
                }
                if !flipped && strength < 0.0 {
                    zone.change_opinion(config);
                    flipped = true;
                }

                zone.strength = strength.abs();
            }

            /* Using linear
            for zone in &mut system.izones {
                let mut strength = (0.5 - proportion) * config.hysteresis_constant_1;
                if !flipped && strength < 0.0 {
                    zone.change_opinion(config);
                    flipped = true;
                }
                if flipped {
                    strength *= config.flipped_multiplier;
                }
                zone.strength = strength.abs();
            } */
        }

        if energy < energy0 || rng.next().unwrap().ln() < -config.beta2 * (energy - energy0) {
            energy0 = energy;
        } else {
            system.get_mut(k).pos.0 = x;
            system.get_mut(k).pos.1 = y;
            system.get_mut(k).po = s;
            rechazados += 1;
        }
        if n % config.n_print == 0 {
            log!(
                "log/log",
                "Currently on: {n: >10}, Energy: {energy0: >6.1}, rej/n: {:.4}",
                rechazados as f64 / n as f64
            );
        }

        if n % config.n_log == 0 {
            let mut b0_count = 0;
            let mut b1_count = 0;
            for p in system.ppl.iter() {
                for z in system.izones.iter() {
                    if z.inside(&p.pos) && z.opinion == p.po {
                        if z.opinion == 0 {
                            b0_count += 1;
                        } else {
                            b1_count += 1;
                        }
                    }
                }
            }
            b0_in_0.push(b0_count);
            b1_in_1.push(b1_count);
            file_log::LOGGER
                .write_log("system/system", "log", system.serialize_to_bin())
                .unwrap();
            log!(
                "histeresis/g_vs_m",
                "{},{}",
                system.izones[0].strength * (system.izones[0].opinion as f64 - 0.5) * 2.0,
                system
                    .ppl
                    .iter()
                    .fold(0.0, |acc, p| acc + (p.po as f64 - 0.5) * 2.0)
            );
        }

        if n % 10 == 0 {
            log!(
                "histeresis/g_vs_m",
                "{},{}",
                system.izones[0].strength * (system.izones[0].opinion as f64 - 0.5) * 2.0,
                system
                    .ppl
                    .iter()
                    .fold(0.0, |acc, p| acc + (p.po as f64 - 0.5) * 2.0)
            );
        }

        if n > config.n_burn {
            temperatures.push(system.temperature(config));
        }
    }

    let average = temperatures.iter().sum::<f64>() / temperatures.len() as f64;
    let sd = temperatures
        .iter()
        .fold(0.0, |a, b| a + (b - average).powi(2))
        / temperatures.len() as f64;
    println!(
        "Average temperature: {}, sd2: {}, ESIM: {}, id: {}",
        average,
        sd,
        config.e_total,
        file_log::index(),
    );
    log!("b0/b0", "{}", itertools::join(b0_in_0, ", "));
    log!("b1/b1", "{}", itertools::join(b1_in_1, ", "));
}

#[allow(dead_code)]
#[allow(clippy::modulo_one)]
/// NVE (microcanonical) with
/// N: config.n_ppl
/// V: LxL
/// E: e_total
fn nve_simulation(system: &mut System, config: &Config) {
    let mut rechazados: usize = 0;
    let mut b0_in_0: Vec<u32> = Vec::new();
    let mut b1_in_1: Vec<u32> = Vec::new();
    let mut flipped: bool = false;

    let mut rng = MonteCarlo::<f64, _>::default();
    let mut energy0 = system.energy(config);
    let mut temperatures = Vec::with_capacity(config.n_steps - config.n_burn);

    for n in 1..config.n_steps {
        if !flipped && n >= config.n_before_flip {
            flipped = true;
            for zone in &mut system.izones {
                zone.change_opinion(config);
            }
        }
        let k = (rng.next().unwrap() * config.n_people as f64).floor() as usize;
        let (x, y, s) = system.step(&mut rng, k, config.delta2, config);

        let mut energy = system.energy(config);
        if n > config.n_before_flip && n <= config.n_flip_accomodation + config.n_before_flip {
            let proportion = (n - config.n_before_flip) as f64 / config.n_flip_accomodation as f64;
            let adjust = config.e_flip * (proportion - 1.0);
            assert!(adjust <= 0.0, "Adjust should always lower the energy");
            energy += adjust;
        }

        let pi = probability_log(energy, config.e_total, config.n_people);
        let p0 = probability_log(energy0, config.e_total, config.n_people);
        if pi > p0 || rng.next().unwrap().ln() < pi - p0 {
            energy0 = energy;
        } else {
            system.get_mut(k).pos.0 = x;
            system.get_mut(k).pos.1 = y;
            system.get_mut(k).po = s;
            rechazados += 1;
        }
        if n % config.n_print == 0 {
            log!(
                "log/log",
                "Currently on: {n: >10}, Energy: {energy0: >6.1}, rej/n: {:.4}",
                rechazados as f64 / n as f64
            );
        }

        if n % config.n_log == 0 {
            let mut b0_count = 0;
            let mut b1_count = 0;
            for p in system.ppl.iter() {
                for z in system.izones.iter() {
                    if z.inside(&p.pos) && z.opinion == p.po {
                        if z.opinion == 0 {
                            b0_count += 1;
                        } else {
                            b1_count += 1;
                        }
                    }
                }
            }
            b0_in_0.push(b0_count);
            b1_in_1.push(b1_count);
            log!(
                "system/system",
                "{}",
                serde_json::to_string(system).unwrap()
            );
        }

        if n > config.n_burn {
            temperatures.push(system.temperature(config));
        }
    }

    let average = temperatures.iter().sum::<f64>() / temperatures.len() as f64;
    let sd = temperatures
        .iter()
        .fold(0.0, |a, b| a + (b - average).powi(2))
        / temperatures.len() as f64;
    println!(
        "Average temperature: {}, sd2: {}, ESIM: {}, id: {}",
        average,
        sd,
        config.e_total,
        file_log::index(),
    );
    log!("b0/b0", "{}", itertools::join(b0_in_0, ", "));
    log!("b1/b1", "{}", itertools::join(b1_in_1, ", "));
}
