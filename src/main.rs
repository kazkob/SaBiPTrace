use example_scenes;
use getopts::Options;
use log::*;
use renderer::*;
use sabiptrace::*;
use std::sync::{Arc, Mutex};

fn main() -> Result<(), std::io::Error> {
    let env = env_logger::Env::new().default_filter_or("sabiptrace=info");
    env_logger::init_from_env(env);

    let args: Vec<String> = std::env::args().collect();
    let mut opts = Options::new();
    opts.reqopt("o", "outdir", "output directory", "DIR");
    opts.optopt("t", "time", "time limit", "SEC");
    opts.optopt("r", "report", "report frequency", "SEC");
    opts.optopt("s", "spp", "spp limit", "SEC");
    opts.optflag("h", "help", "show help");

    let matches = match opts.parse(&args[1..]) {
        Ok(m) => m,
        Err(f) => {
            eprintln!("{}", f.to_string());
            eprintln!("{}", opts.usage("sabiptrace"));
            panic!("invalid options");
        }
    };
    if matches.opt_present("h") {
        println!("{}", opts.usage("sabiptrace"));
        return Ok(());
    }

    let outdir = format!("output/{}", matches.opt_str("o").unwrap());
    std::fs::create_dir_all(&outdir)?;
    let time_limit = matches
        .opt_str("t")
        .map(|s| s.parse().expect(&format!("failed to prase time {}", s)))
        .unwrap_or(1.0);
    let report_freq: f64 = matches
        .opt_str("r")
        .map(|s| s.parse().expect(&format!("failed to prase time {}", s)))
        .unwrap_or(5.0);
    let max_spp: usize = matches
        .opt_str("s")
        .map(|s| s.parse().expect(&format!("failed to prase time {}", s)))
        .unwrap_or(1);

    let v = vec![RGB::all(0.0); 10];
    let film = {
        let s = 50;
        image::Film::new(16 * s, 9 * s, v.clone())
    };

    let (camera, scene) = example_scenes::make_black_shell();
    let film = Arc::new(Mutex::new(film));
    let scene = Arc::new(scene);

    let film_config = FilmConfig {
        film_arc: film.clone(),
        accum_init: v.clone(),
    };

    let render_config = RenderConfig {
        integrator: Integrator::PathrTraceWithNee,
        nthread: num_cpus::get(),
    };

    info!("outdir {}", outdir);
    info!("trheads      :{:?}", render_config.nthread);
    info!("integrator   :{:?}", render_config.integrator);
    info!("max spp      :{:?}", max_spp);
    info!("time limit   :{:?}", time_limit);
    info!("report freq  :{:?}", report_freq);

    let sched = {
        let start = std::time::Instant::now();
        let _film = film.clone();
        Box::new(move |next_cycle: usize, completed_samples: usize| {
            if next_cycle <= 0 {
                Some(1)
            } else {
                let elapsed = std::time::Instant::now().duration_since(start);
                let ms = { elapsed.as_secs() * 1000 + elapsed.subsec_millis() as u64 };
                let secs = (ms as f64) / 1000.0;
                let progress = completed_samples as f64 / max_spp as f64;
                let eta = secs * (1.0 - progress) / progress;
                let spd = completed_samples as f64 / secs;
                let spd_pc = spd / render_config.nthread as f64;
                info!(
                    "{} / {} ({:.2} %) elapsed {:.2} sec",
                    completed_samples,
                    max_spp,
                    progress * 100.0,
                    secs,
                );
                info!("    ETA {:.2} sec ({:?} for limit)", eta, time_limit - secs);

                info!("    Speed {:.2} spp/sec {:.2} spp/sec/core", spd, spd_pc);
                //let film = film.lock().unwrap();
                //film.to_image(RGBPixel::average).write_exr(&format!("output/{}.exr", cycle));
                if completed_samples >= max_spp {
                    None
                } else if secs >= time_limit {
                    info!("stopping due to time limit");
                    None
                } else {
                    let rest = max_spp - completed_samples;
                    let next_cycle_time = report_freq.min(time_limit - secs);
                    let next_report: usize = (next_cycle_time * spd) as usize;
                    Some(rest.min(next_report).max(1))
                }
            }
        })
    };

    let renderer = Renderer;
    renderer.render(scene, &camera, film_config, render_config, sched);
    let film = film.lock().unwrap();
    for i in 0..v.len() {
        film.to_image(|v| v.accum[i] / v.samples as f32)
            .write_exr(&format!("{}/len{:>02}.exr", outdir, i));
    }
    film.to_image(|v| {
        let mut sum = RGB::default();
        for c in v.accum.iter() {
            sum += *c;
        }
        sum / v.samples as f32
    })
    .write_exr(&format!("{}/total.exr", outdir));
    //film.lock()
    //    .unwrap()
    //    .to_image(RGBPixel::average)
    //    .write_exr("output/output.exr");
    Ok(())
}
