//! Frequency response system and other filter tests.

#![allow(
    dead_code,
    clippy::precedence,
    clippy::type_complexity,
    clippy::float_cmp,
    clippy::len_zero,
    clippy::double_neg,
    clippy::many_single_char_names,
    clippy::manual_range_contains
)]

use fundsp::hacker::*;
use funutd::Rnd;
use num_complex::Complex64;
use realfft::*;
use rustfft::algorithm::Radix4;
use rustfft::Fft;
use rustfft::FftDirection;

#[test]
fn test_filter() {
    let mut rnd = Rnd::new();

    // Test follow().
    for _ in 0..200 {
        // Bias testing toward smaller lengths to cut testing time shorter.
        let samples = round(xerp(1.0, 500_000.0, squared(rnd.f64())));
        let sample_rate = xerp(10.0, 500_000.0, rnd.f64());
        let mut x = follow(samples / sample_rate);
        x.set_sample_rate(sample_rate);
        x.filter_mono(0.0);
        let goal = lerp(-100.0, 100.0, rnd.f64());
        for _ in 0..samples as usize {
            x.filter_mono(goal);
        }
        // Promise was 0.5% accuracy between 1 and 500k samples.
        let response = x.value() / goal;
        //println!("follow samples {}, goal {}, response {}", samples, goal, response);
        assert!(response >= 0.495 && response <= 0.505);
    }

    // Test asymmetric follow().
    for _ in 0..200 {
        // Bias testing toward smaller lengths to cut testing time shorter.
        let attack_samples = round(xerp(1.0, 500_000.0, squared(rnd.f64())));
        let release_samples = round(xerp(1.0, 500_000.0, squared(rnd.f64())));
        let sample_rate = xerp(10.0, 100_000.0, rnd.f64());
        let goal = lerp(-100.0, 100.0, rnd.f64());
        let mut x = follow((attack_samples / sample_rate, release_samples / sample_rate));
        x.set_sample_rate(sample_rate);
        x.filter_mono(0.0);
        for _ in 0..(if goal > 0.0 {
            attack_samples
        } else {
            release_samples
        }) as usize
        {
            x.filter_mono(goal);
        }
        // Promise was 0.5% accuracy between 1 and 500k samples.
        let response = x.value() / goal;
        //println!("follow attack samples {}, release samples {}, goal {}, response {}", attack_samples, release_samples, goal, response);
        assert!(response >= 0.495 && response <= 0.505);
    }
}

/// Complex64 with real component `x` and imaginary component zero.
fn re<T: Float>(x: T) -> Complex64 {
    Complex64::new(x.to_f64(), 0.0)
}

fn is_equal_response(x: Complex64, y: Complex64) -> bool {
    let abs_tolerance = 1.0e-9;
    let amp_tolerance = db_amp(0.05);
    let phase_tolerance = 1.0e-4 * TAU;
    let x_norm = x.norm();
    let y_norm = y.norm();
    let x_phase = x.arg();
    let y_phase = y.arg();
    x_norm / amp_tolerance - abs_tolerance <= y_norm
        && x_norm * amp_tolerance + abs_tolerance >= y_norm
        && min(
            abs(x_phase - y_phase),
            min(abs(x_phase - y_phase + TAU), abs(x_phase - y_phase - TAU)),
        ) <= phase_tolerance
}

fn test_response<X>(mut filter: X)
where
    X: AudioUnit64,
{
    assert!(filter.inputs() == 1 && filter.outputs() == 1);

    filter.allocate();

    let length = 0x10000;
    let sample_rate = DEFAULT_SR;

    filter.reset();
    filter.set_sample_rate(sample_rate);

    let mut input = 1.0;
    let mut buffer = Vec::with_capacity(length);
    // Try to remove effect of DC by warming up the filter.
    for _i in 0..length / 4 {
        filter.filter_mono(0.0);
    }
    for _i in 0..length {
        // Apply a Hann window.
        //let window = 0.5 + 0.5 * cos(_i as f64 / length as f64 * PI);
        let x = filter.filter_mono(input);
        //buffer.push(re(x * window));
        buffer.push(re(x));
        input = 0.0;
    }

    let fft = Radix4::new(length, FftDirection::Forward);
    fft.process(&mut buffer);

    let mut f = 10.0;
    while f <= 22_000.0 {
        let i = round(f * length as f64 / sample_rate) as usize;
        let f_i = i as f64 / length as f64 * sample_rate;
        let reported = filter.response(0, f_i).unwrap();
        let response = buffer[i];
        /*
        println!(
            "{} Hz reported ({}, {}) actual ({}, {}) matches {}",
            f_i,
            reported.norm(),
            reported.arg(),
            response.norm(),
            response.arg(),
            is_equal_response(reported, response)
        );
        */
        assert!(is_equal_response(reported, response));
        if f < 1000.0 {
            f += 10.0;
        } else {
            f += 100.0;
        }
    }
}

#[test]
fn test_misc() {
    let epsilon = 1.0e-9;
    assert!((pass() & tick()).response(0, 22050.0).unwrap().norm() < epsilon);
    assert!(
        (0.5 * pass() & tick() & 0.5 * tick() >> tick())
            .response(0, 22050.0)
            .unwrap()
            .norm()
            < epsilon
    );
    assert!(
        (pass() & tick() & tick() >> tick())
            .response(0, 22050.0)
            .unwrap()
            .norm()
            > 0.1
    );
}

/// Test frequency response system.
#[test]
fn test_responses() {
    test_response(bell_hz(500.0, 1.0, 2.0) * 0.5);
    test_response(lowshelf_hz(2000.0, 10.0, 5.0));
    test_response(highshelf_hz(2000.0, 10.0, 5.0));
    test_response(peak_hz(5000.0, 1.0));
    test_response(allpass_hz(500.0, 5.0));
    test_response(notch_hz(1000.0, 1.0));
    test_response(lowpass_hz(50.0, 1.0));
    test_response(highpass_hz(5000.0, 1.0));
    test_response(bandpass_hz(100.0, 1.0));
    test_response(highpass_hz(500.0, 1.0) & bandpass_hz(500.0, 2.0));
    test_response(pinkpass());
    test_response(follow(0.0002));
    test_response(follow(0.01));
    test_response(delay(0.0001));
    test_response(delay(0.0001) >> delay(0.0002));
    test_response(dcblock());
    test_response(dcblock_hz(100.0) & follow(0.001));
    test_response(lowpole_hz(1000.0));
    test_response(split() >> (lowpole_hz(100.0) + lowpole_hz(190.0)));
    test_response(lowpole_hz(10000.0));
    test_response(resonator_hz(300.0, 20.0));
    test_response(butterpass_hz(200.0));
    test_response(butterpass_hz(1000.0));
    test_response(butterpass_hz(500.0) & bell_hz(2000.0, 10.0, 5.0));
    test_response(butterpass_hz(6000.0) >> lowpass_hz(500.0, 3.0));
    test_response(pass() & tick());
    test_response(pass() * 0.25 & tick() * 0.5 & tick() >> tick() * 0.25);
    test_response(tick() & lowshelf_hz(500.0, 2.0, 0.1));
    test_response(allpole_delay(0.5) & allpole_delay(1.3) & allpole_delay(0.1));
    test_response(highpole_hz(5000.0) & highpole_hz(500.0) & highpole_hz(2000.0));
    test_response(
        (delay(0.001) ^ delay(0.002)) >> reverse() >> (delay(0.003) | delay(0.007)) >> join(),
    );
    test_response(
        (butterpass_hz(15000.0) ^ allpass_hz(10000.0, 10.0)) >> lowpole_hz(500.0) + pass(),
    );
    test_response(
        (resonator_hz(12000.0, 500.0) ^ lowpass_hz(3000.0, 0.5))
            >> pass() + highshelf_hz(3000.0, 0.5, 4.0),
    );
    test_response(split() >> multipass::<U32>() >> join());
    test_response(
        split()
            >> stack::<U8, _, _>(|i| {
                resonator_hz(1000.0 + 1000.0 * i as f64, 100.0 + 100.0 * i as f64)
            })
            >> join(),
    );
    test_response(branchf::<U5, _, _>(|t| resonator_hz(xerp(100.0, 20000.0, t), 10.0)) >> join());
    test_response(pipe::<U4, _, _>(|i| {
        bell_hz(
            1000.0 + 1000.0 * i as f64,
            (i + 1) as f64,
            db_amp((i + 6) as f64),
        )
    }));
    test_response(
        split() >> stack::<U5, _, _>(|i| lowpole_hz(1000.0 + 1000.0 + i as f64)) >> join(),
    );
    test_response(bus::<U7, _, _>(|i| {
        lowpass_hz(1000.0 + 1000.0 * rnd(i), 1.0 + 1.0 * rnd(i << 1))
    }));
    test_response(
        split::<U3>()
            >> multisplit::<U3, U3>()
            >> sumf::<U9, _, _>(|f| highshelf_hz(f, 1.0 + f, 2.0 + f)),
    );
    test_response(pan(0.5) >> join());
    test_response(pan(0.0) >> join());
    test_response(pan(-1.0) >> multijoin::<U1, U2>());
    let tmp = shared(0.0);
    test_response(fir((0.5, 0.5)) | timer(&tmp));
    test_response(fir((0.25, 0.5, 0.25)) >> monitor(&tmp, Meter::Sample));
    test_response(fir((0.4, 0.3, 0.2, 0.1)));
    test_response(morph_hz(1000.0, 1.0, 0.5));
    test_response(morph_hz(2000.0, 2.0, -0.5));
    test_response((pass() | dc((1000.0, 0.5, 0.5))) >> morph());
    test_response((pass() | dc((500.0, 2.0, -1.0))) >> morph());
    test_response(biquad(0.0, 0.17149959, 0.29287490, 0.58574979, 0.29287490));
    test_response(biquad(
        0.03371705,
        0.17177385,
        1.05925373,
        -0.03571491,
        0.18195209,
    ));
    test_response(pass() + 1.0 >> lowpass_hz(1000.0, 1.0));
    test_response((pass() | dc(1.0)) >> rotate(0.5, 1.0) >> (pass() | sink()));
    test_response((dc(2.0) | pass()) >> rotate(-0.1, 0.5) >> (pass() | sink()));

    let mut net1 = Net64::new(1, 1);
    net1.chain(Box::new(lowpole_hz(1500.0)));
    test_response(net1);

    let mut net2 = Net64::new(1, 1);
    net2.chain(Box::new(lowpole_hz(500.0)));
    net2.chain(Box::new(lowpole_hz(2500.0)));
    test_response(net2);

    let mut net3 = Net64::new(1, 1);
    net3.chain(Box::new(highpole_hz(1500.0)));
    let mut net4 = Net64::new(1, 1);
    net4.chain(Box::new(highpole_hz(500.0)));
    test_response(net3 >> net4);

    let mut net5 = Net64::new(1, 1);
    net5.chain(Box::new(highpole_hz(1500.0)));
    let mut net6 = Net64::new(1, 1);
    net6.chain(Box::new(highpole_hz(500.0)));
    test_response(net5 & net6 & pass());

    let mut net7 = Net64::new(1, 1);
    net7.chain(Box::new(highpass_hz(1000.0, 1.0)));
    test_response(net7);

    let mut net8 = Net64::new(1, 1);
    net8.chain(Box::new(highpole_hz(1500.0)));
    test_response(Net64::wrap(Box::new(zero())) + net8);

    let mut net9 = Net64::new(1, 1);
    net9.chain(Box::new(highpole_hz(2000.0)));
    test_response(Net64::wrap(Box::new(dc(1.0))) - net9);

    let mut neta = Net64::new(1, 1);
    neta.chain(Box::new(notch_hz(2500.0, 2.0)));
    test_response(Net64::wrap(Box::new(dc(2.0))) * neta);

    let mut netb = Net64::new(1, 1);
    netb.chain(Box::new(notch_hz(2500.0, 1.0)));
    test_response(netb * 2.0 >> lowpass_hz(1500.0, 1.0));

    let mut netc = Net64::new(1, 1);
    netc.chain(Box::new(highpass_hz(5500.0, 1.0)));
    test_response(netc >> highpass_hz(2500.0, 1.0) + 1.0);

    let mut netd = Net64::new(1, 1);
    netd.chain(Box::new(lowpass_hz(5000.0, 1.0)));
    test_response((netd ^ highpass_hz(3000.0, 1.0)) >> (pass() + pass()));

    let mut nete = Net64::new(1, 1);
    nete.chain(Box::new(notch_hz(5000.0, 1.0)));
    test_response(
        (nete.clone() ^ peak_hz(3000.0, 1.0)) >> (Net64::wrap(Box::new(pass())) + pass()),
    );

    let mut netf = Net64::new(1, 1);
    netf.chain(Box::new(notch_hz(2000.0, 1.0)));
    test_response(
        (netf.clone() ^ pass() ^ peak_hz(1000.0, 1.0))
            >> (Net64::wrap(Box::new(pass())) + pass() + pass()),
    );

    let mut netg = Net64::new(1, 1);
    netg.chain(Box::new(notch_hz(2000.0, 1.0)));
    test_response(
        (netg ^ pass() ^ pass())
            >> (Net64::wrap(Box::new(pass())) | pass() | pinkpass())
            >> (Net64::wrap(Box::new(pinkpass())) + pass() + pass()),
    );
}

// Test various allpass filters for the allpass property.
#[test]
fn test_allpass() {
    let length = 0x10000;
    let mut planner = RealFftPlanner::<f64>::new();
    let r2c = planner.plan_fft_forward(length);
    let mut spectrum = r2c.make_output_vec();

    let allpasses: [Box<dyn AudioUnit64>; 12] = [
        Box::new(pass()),
        Box::new(tick()),
        Box::new(allpole_delay(0.5)),
        Box::new(allpole_delay(0.8)),
        Box::new(delay(2.0 / DEFAULT_SR)),
        Box::new(delay(0.001)),
        Box::new(allpass_hz(1000.0, 1.0)),
        Box::new(allpass_hz(2000.0, 2.0)),
        Box::new(allnest_c(0.5, pass())),
        Box::new(allnest_c(0.6, tick())),
        Box::new(allnest_c(0.7, allpole_delay(0.5))),
        Box::new(allnest_c(-0.6, allpass_hz(3000.0, 3.0))),
    ];

    let impulse = Wave64::render(DEFAULT_SR, 1.0, &mut (impulse::<U1>()));

    for mut x in allpasses {
        let response = impulse.filter(length as f64 / DEFAULT_SR, &mut *(x));
        let mut data = response.channel(0).clone();
        r2c.process(&mut data, &mut spectrum).unwrap();
        let tolerance = 1.0e-9;
        for s in &spectrum {
            let norm = s.norm();
            assert!(norm >= 1.0 - tolerance && norm <= 1.0 + tolerance);
        }
    }
}
